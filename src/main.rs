#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]

mod categories;
mod config;
mod db;
#[cfg(windows)]
mod desktop;
mod finance;
mod key_material;
mod log;
mod models;
mod plaid;
mod routes;
mod seed;
mod ui;

use crate::config::Config;
use crate::db::Db;
use crate::log::info_event;
use crate::routes::{router, AppState};
use axum::Router;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::services::ServeDir;

fn main() {
    #[cfg(windows)]
    {
        if desktop::should_open_window() {
            if let Err(msg) = desktop::run() {
                desktop::show_error("Anchored could not start", &msg);
                std::process::exit(1);
            }
            return;
        }
        desktop::attach_parent_console();
    }

    let code = match server_main() {
        Ok(()) => 0,
        Err(msg) => {
            eprintln!("{msg}");
            1
        }
    };
    std::process::exit(code);
}

fn server_main() -> Result<(), String> {
    log::init();
    let config = Config::from_env()?;
    block_on_serve(config)
}

pub(crate) fn block_on_serve(config: Config) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|_| "could not start Anchored".to_string())?;
    rt.block_on(serve(config))
}

async fn serve(config: Config) -> Result<(), String> {
    let db = Db::open(config.database_path.clone(), config.database_key.clone())?;

    // Load demo household on first run when the file did not already exist.
    if !db.existed_at_startup() {
        match seed::seed_database(&db, false) {
            Ok((true, _)) => info_event("demo household seeded"),
            Ok((false, _)) => {}
            Err(_) => crate::log::error_event("demo seed failed"),
        }
    }
    let db = Arc::new(db);

    let state = AppState {
        db,
        config: config.clone(),
    };

    let app = Router::new()
        .merge(router(state))
        .nest_service("/static", ServeDir::new(static_dir()));

    let addr: SocketAddr = format!("{}:{}", config.bind_host, config.port)
        .parse()
        .map_err(|_| "bind address".to_string())?;
    info_event(&format!(
        "server starting host={} port={} container={}",
        config.bind_host,
        config.port,
        if config.in_container { "yes" } else { "no" }
    ));

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("could not bind {addr}: {e}"))?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| format!("server error: {e}"))?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    info_event("server shutting down");
}

/// Pages live in `static/` next to the working directory (local runs and the
/// container) or next to the executable (the installed Windows app).
pub(crate) fn static_dir() -> PathBuf {
    let cwd = PathBuf::from("static");
    if cwd.join("app.css").is_file() {
        return cwd;
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let beside = dir.join("static");
            if beside.join("app.css").is_file() {
                return beside;
            }
        }
    }
    cwd
}

#[cfg(test)]
mod tests {
    use super::static_dir;

    #[test]
    fn static_dir_finds_app_css() {
        assert!(static_dir().join("app.css").is_file());
    }
}
