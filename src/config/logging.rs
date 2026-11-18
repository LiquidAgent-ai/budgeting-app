//! Sparse troubleshooting logs. Real events only — no amounts, payees, tokens, or account numbers.

#[cfg(windows)]
use std::io::{self, Write};
#[cfg(windows)]
use std::path::Path;
#[cfg(windows)]
use std::sync::{Arc, Mutex};
use tracing::{error, info};

pub fn init() {
    tracing_subscriber::fmt()
        .with_env_filter(env_filter())
        .with_target(false)
        .with_level(true)
        .compact()
        .init();
}

/// Append-only log for the Windows app, where there is no console.
#[cfg(windows)]
pub fn init_file(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    rotate_log(path);
    let mut file = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        Ok(file) => file,
        Err(_) => {
            init();
            return;
        }
    };
    let stamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    let _ = writeln!(file, "\n--- {stamp} ---");
    let _ = file.flush();
    let file = Arc::new(Mutex::new(file));
    let writer = file;
    let _ = tracing_subscriber::fmt()
        .with_env_filter(env_filter())
        .with_target(false)
        .with_level(true)
        .with_ansi(false)
        .compact()
        .with_writer(move || LogFile(writer.clone()))
        .init();
}

fn env_filter() -> tracing_subscriber::EnvFilter {
    tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
}

#[cfg(windows)]
fn rotate_log(path: &Path) {
    let Ok(meta) = std::fs::metadata(path) else {
        return;
    };
    if meta.len() <= 1024 * 1024 {
        return;
    }
    let mut previous = path.to_path_buf();
    previous.as_mut_os_string().push(".1");
    let _ = std::fs::remove_file(&previous);
    let _ = std::fs::rename(path, previous);
}

#[cfg(windows)]
struct LogFile(Arc<Mutex<std::fs::File>>);

#[cfg(windows)]
impl Write for LogFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.0.lock() {
            Ok(mut file) => file.write(buf),
            Err(_) => Ok(buf.len()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.0.lock() {
            Ok(mut file) => file.flush(),
            Err(_) => Ok(()),
        }
    }
}

pub fn info_event(msg: &str) {
    info!("{msg}");
}

pub fn error_event(msg: &str) {
    error!("{msg}");
}
