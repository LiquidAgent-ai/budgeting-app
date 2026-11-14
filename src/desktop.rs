//! Windows desktop shell. Opens a window, serves the app on 127.0.0.1, and
//! seals the database key with DPAPI. The plaintext key is not written to disk.

use crate::config::Config;
use crate::key_material::{blob_contains_secret, encode_key_material, key_action, KeyAction};
use crate::log::{error_event, info_event};
use std::path::{Path, PathBuf};

const CREDENTIAL_ERROR: &str = "Anchored could not read the database key from the OS credential store. The database was not opened.";
const SEAL_ERROR: &str = "Anchored could not seal the database key. The database was not created.";
const MISSING_KEY_ERROR: &str = "Anchored found an encrypted database but not the sealed key for this Windows account. The database was not opened and was not replaced.";

pub fn should_open_window() -> bool {
    std::env::var("ANCHORED_SERVER").ok().as_deref() != Some("1")
        && std::env::var("ANCHORED_CONTAINER").ok().as_deref() != Some("1")
}

pub fn attach_parent_console() {
    const ATTACH_PARENT_PROCESS: u32 = 0xFFFFFFFF;
    unsafe {
        windows_sys::Win32::System::Console::AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

pub fn run() -> Result<(), String> {
    acquire_single_instance()?;
    let root = data_root()?;
    std::fs::create_dir_all(&root).map_err(|_| "could not create the Anchored data folder")?;
    if !crate::static_dir().join("app.css").is_file() {
        return Err("This install is missing its pages. Reinstall Anchored.".into());
    }
    crate::log::init_file(&root.join("server.log"));
    info_event("desktop session starting");

    let port = free_loopback_port()?;
    let key = prepare_database_key(&root)?;
    apply_desktop_env(&root, &key, port);
    drop(key);
    load_plaid_file(&root.join("plaid.env"));

    let config = Config::from_env()?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = crate::block_on_serve(config);
        let _ = tx.send(result);
    });
    wait_until_ready(port, &rx)?;
    info_event("desktop window open");
    open_window(port)
}

pub fn show_error(title: &str, message: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONERROR, MB_OK, MB_SETFOREGROUND,
    };
    let mut clean = String::new();
    for ch in message.chars().take(1200) {
        if ch == '\n' || ch == '\t' || !ch.is_control() {
            clean.push(ch);
        }
    }
    if clean.is_empty() {
        clean.push_str("Anchored could not start.");
    }
    let title_w = wide(title);
    let message_w = wide(&clean);
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            message_w.as_ptr(),
            title_w.as_ptr(),
            MB_OK | MB_ICONERROR | MB_SETFOREGROUND,
        );
    }
}

fn data_root() -> Result<PathBuf, String> {
    let base = std::env::var("APPDATA")
        .map_err(|_| "Anchored could not find the Windows profile folder.".to_string())?;
    if base.trim().is_empty() {
        return Err("Anchored could not find the Windows profile folder.".into());
    }
    Ok(PathBuf::from(base).join("Anchored"))
}

fn acquire_single_instance() -> Result<(), String> {
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, SetLastError, ERROR_ALREADY_EXISTS,
    };
    use windows_sys::Win32::System::Threading::CreateMutexW;
    let name = wide("Local\\AnchoredDesktop");
    unsafe { SetLastError(0) };
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    let err = unsafe { GetLastError() };
    if handle.is_null() {
        return Err("Anchored could not start.".into());
    }
    if err == ERROR_ALREADY_EXISTS {
        unsafe { CloseHandle(handle) };
        focus_existing();
        std::process::exit(0);
    }
    Ok(())
}

fn focus_existing() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE,
    };
    let title = wide("Anchored");
    unsafe {
        let hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_RESTORE);
            SetForegroundWindow(hwnd);
        }
    }
}

fn prepare_database_key(root: &Path) -> Result<String, String> {
    let key_path = root.join("db-key.bin");
    let db_path = root.join("anchored.db");
    match key_action(key_path.exists(), db_path.exists()) {
        KeyAction::Unseal => {
            let key = read_sealed(&key_path)?;
            info_event("database key unsealed");
            Ok(key)
        }
        KeyAction::Create => {
            let key = generate_key()?;
            write_sealed(&key_path, &key)?;
            info_event("database key sealed");
            Ok(key)
        }
        KeyAction::RefuseMissingKey => {
            error_event("database key missing for existing database");
            Err(MISSING_KEY_ERROR.into())
        }
    }
}

fn generate_key() -> Result<String, String> {
    use windows_sys::Win32::Security::Cryptography::{
        BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
    };
    let mut bytes = [0u8; 32];
    let status = unsafe {
        BCryptGenRandom(
            std::ptr::null_mut(),
            bytes.as_mut_ptr(),
            bytes.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status != 0 {
        error_event("database key generation failed");
        return Err(SEAL_ERROR.into());
    }
    let key = encode_key_material(&bytes);
    bytes.fill(0);
    if key.len() < 16 {
        return Err(SEAL_ERROR.into());
    }
    Ok(key)
}

/// DPAPI blob for the current user. Empty description, no extra entropy, flags 0.
/// This is the same shape Electron's safeStorage used, so an existing
/// `db-key.bin` still unseals on this Windows account.
fn protect(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Foundation::{LocalFree, HLOCAL};
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_PROMPTSTRUCT, CRYPT_INTEGER_BLOB,
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: plaintext.len() as u32,
        pbData: plaintext.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let desc = [0u16];
    let ok = unsafe {
        CryptProtectData(
            &input,
            desc.as_ptr(),
            std::ptr::null::<CRYPT_INTEGER_BLOB>(),
            std::ptr::null::<std::ffi::c_void>(),
            std::ptr::null::<CRYPTPROTECT_PROMPTSTRUCT>(),
            0,
            &mut output,
        )
    };
    if ok == 0 || output.pbData.is_null() || output.cbData == 0 {
        error_event("database key seal failed");
        return Err(SEAL_ERROR.into());
    }
    let blob =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe { LocalFree(output.pbData as HLOCAL) };
    Ok(blob)
}

fn unprotect(blob: &[u8]) -> Result<String, String> {
    use windows_sys::Win32::Foundation::{LocalFree, HLOCAL};
    use windows_sys::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_PROMPTSTRUCT, CRYPT_INTEGER_BLOB,
    };
    if blob.is_empty() {
        return Err(CREDENTIAL_ERROR.into());
    }
    let input = CRYPT_INTEGER_BLOB {
        cbData: blob.len() as u32,
        pbData: blob.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            std::ptr::null_mut(),
            std::ptr::null::<CRYPT_INTEGER_BLOB>(),
            std::ptr::null::<std::ffi::c_void>(),
            std::ptr::null::<CRYPTPROTECT_PROMPTSTRUCT>(),
            0,
            &mut output,
        )
    };
    if ok == 0 || output.pbData.is_null() || output.cbData == 0 {
        error_event("database key unseal failed");
        return Err(CREDENTIAL_ERROR.into());
    }
    let bytes =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe { LocalFree(output.pbData as HLOCAL) };
    String::from_utf8(bytes).map_err(|_| {
        error_event("database key unseal failed");
        CREDENTIAL_ERROR.to_string()
    })
}

fn read_sealed(path: &Path) -> Result<String, String> {
    let blob = std::fs::read(path).map_err(|_| {
        error_event("database key file unreadable");
        CREDENTIAL_ERROR.to_string()
    })?;
    let key = unprotect(&blob)?.trim().to_string();
    if key.len() < 16 {
        error_event("database key unseal failed");
        return Err(CREDENTIAL_ERROR.into());
    }
    Ok(key)
}

fn write_sealed(path: &Path, key: &str) -> Result<(), String> {
    let blob = protect(key.as_bytes())?;
    if blob_contains_secret(&blob, key) {
        error_event("database key seal failed");
        return Err(SEAL_ERROR.into());
    }
    let round_trip = unprotect(&blob)?;
    if round_trip != key {
        error_event("database key seal failed");
        return Err(SEAL_ERROR.into());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| "could not create the Anchored data folder")?;
    }
    let tmp = path.with_extension("creating");
    std::fs::write(&tmp, &blob).map_err(|_| {
        let _ = std::fs::remove_file(&tmp);
        SEAL_ERROR.to_string()
    })?;
    if let Err(err) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        let _ = err;
        return Err(SEAL_ERROR.into());
    }
    Ok(())
}

fn apply_desktop_env(root: &Path, key: &str, port: u16) {
    let db_path = root.join("anchored.db");
    // SAFETY: this runs on the desktop thread before the server thread starts.
    // On Windows, set_var is sound even after other threads exist.
    unsafe {
        std::env::set_var("HOST", "127.0.0.1");
        std::env::set_var("PORT", port.to_string());
        std::env::set_var("DATABASE_PATH", &db_path);
        std::env::set_var("DATABASE_KEY", key);
    }
}

fn load_plaid_file(path: &Path) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let mut loaded = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key != "PLAID_CLIENT_ID" && key != "PLAID_SECRET" && key != "PLAID_ENV" {
            continue;
        }
        let mut value = value.trim().to_string();
        if (value.starts_with('"') && value.ends_with('"') && value.len() >= 2)
            || (value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2)
        {
            value = value[1..value.len() - 1].to_string();
        }
        if value.is_empty() || value == "your_client_id" || value == "your_sandbox_secret" {
            continue;
        }
        // SAFETY: still before the server thread starts.
        unsafe { std::env::set_var(key, value) };
        loaded = true;
    }
    if loaded {
        info_event("plaid configuration file loaded");
    }
}

fn free_loopback_port() -> Result<u16, String> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))
        .map_err(|_| "Anchored could not choose a local port.".to_string())?;
    let port = listener
        .local_addr()
        .map_err(|_| "Anchored could not choose a local port.".to_string())?
        .port();
    drop(listener);
    Ok(port)
}

fn wait_until_ready(
    port: u16,
    rx: &std::sync::mpsc::Receiver<Result<(), String>>,
) -> Result<(), String> {
    let started = std::time::Instant::now();
    while started.elapsed() < std::time::Duration::from_secs(60) {
        match rx.try_recv() {
            Ok(Err(msg)) => return Err(msg),
            Ok(Ok(())) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                return Err("Anchored stopped before the window could open.".into());
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
        if health_ok(port) {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    match rx.try_recv() {
        Ok(Err(msg)) => Err(msg),
        _ => Err("Anchored took too long to start.".into()),
    }
}

fn health_ok(port: u16) -> bool {
    use std::io::{Read, Write};
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut stream) =
        std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(400))
    else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(800)));
    let _ = stream.set_write_timeout(Some(std::time::Duration::from_millis(800)));
    let req =
        format!("GET /api/health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    if stream.write_all(req.as_bytes()).is_err() {
        return false;
    }
    let mut buf = [0u8; 128];
    let Ok(n) = stream.read(&mut buf) else {
        return false;
    };
    let text = String::from_utf8_lossy(&buf[..n]);
    text.starts_with("HTTP/1.1 200") || text.starts_with("HTTP/1.0 200")
}

fn open_window(port: u16) -> Result<(), String> {
    use tao::dpi::LogicalSize;
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoop};
    use tao::window::WindowBuilder;
    use wry::WebViewBuilder;

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Anchored")
        .with_inner_size(LogicalSize::new(1280.0, 840.0))
        .with_min_inner_size(LogicalSize::new(880.0, 640.0))
        .build(&event_loop)
        .map_err(|_| "Anchored could not open its window.".to_string())?;

    let start = format!("http://127.0.0.1:{port}/");
    let allowed = format!("http://127.0.0.1:{port}");
    let webview = WebViewBuilder::new()
        .with_url(start)
        .with_devtools(false)
        .with_incognito(true)
        .with_clipboard(true)
        .with_download_started_handler(|_url, _path| false)
        .with_new_window_req_handler(|_url, _features| wry::NewWindowResponse::Deny)
        .with_navigation_handler(move |url| {
            url == "about:blank" || url == allowed || url.starts_with(&format!("{allowed}/"))
        })
        .build(&window)
        .map_err(|err| window_error(&err.to_string()))?;

    event_loop.run(move |event, _target, control_flow| {
        let _keepalive = (&window, &webview);
        *control_flow = ControlFlow::Wait;
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            info_event("desktop window closed");
            *control_flow = ControlFlow::ExitWithCode(0);
        }
    });
}

fn window_error(raw: &str) -> String {
    error_event("desktop window failed");
    let lower = raw.to_ascii_lowercase();
    if lower.contains("webview2") || lower.contains("0x80070002") {
        "Anchored could not open its window because the Microsoft Edge WebView2 Runtime is missing. Install it from Microsoft, then start Anchored again.".into()
    } else {
        "Anchored could not open its window.".into()
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
