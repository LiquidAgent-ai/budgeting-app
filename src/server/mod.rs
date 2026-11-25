//! HTTP server: open the encrypted DB, mount routes, serve static pages.

use crate::api::{router, AppState};
use crate::config::logging::{error_event, info_event, short_reason};
use crate::config::Config;
use crate::db::Db;
use crate::{plaid, teller};
use axum::Router;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tower_http::services::ServeDir;

pub fn block_on_serve(config: Config) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|_| "could not start Anchored".to_string())?;
    rt.block_on(serve(config))
}

async fn serve(config: Config) -> Result<(), String> {
    // Fresh installs stay empty. Demo data is opt-in via POST /api/seed (or Accounts).
    let opened = Db::open(config.database_path.clone(), config.database_key.clone())?;
    info_event(&format!(
        "database opened existed_at_startup={}",
        if opened.existed_at_startup() {
            "yes"
        } else {
            "no"
        }
    ));
    let db = Arc::new(opened);

    spawn_auto_sync(config.clone(), Arc::clone(&db));

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

/// Periodic read-only Plaid/Teller sync in the same Rust process (desktop + container).
fn spawn_auto_sync(config: Config, db: Arc<Db>) {
    if !config.auto_sync_active() {
        return;
    }
    let hours = config.sync_interval_hours;
    info_event(&format!(
        "scheduled bank sync enabled interval_hours={hours}"
    ));
    tokio::spawn(async move {
        let interval = Duration::from_secs(hours.saturating_mul(3600));
        loop {
            run_scheduled_sync(&config, &db).await;
            tokio::time::sleep(interval).await;
        }
    });
}

async fn run_scheduled_sync(config: &Config, db: &Db) {
    info_event("scheduled bank sync started");

    let mut plaid_ok = 0i64;
    let mut plaid_failed = 0i64;
    let mut teller_ok = 0i64;
    let mut teller_failed = 0i64;

    if config.plaid_configured() {
        match plaid::sync_all(config, db).await {
            Ok(v) => {
                plaid_ok = v.get("items").and_then(|x| x.as_i64()).unwrap_or(0);
                plaid_failed = v.get("failed").and_then(|x| x.as_i64()).unwrap_or(0);
            }
            Err(e) => {
                plaid_failed = 1;
                error_event(&format!(
                    "scheduled plaid sync failed: {}",
                    short_reason(&e)
                ));
            }
        }
    }

    if config.teller_configured() {
        match teller::sync_all(config, db).await {
            Ok(v) => {
                teller_ok = v.get("enrollments").and_then(|x| x.as_i64()).unwrap_or(0);
                teller_failed = v.get("failed").and_then(|x| x.as_i64()).unwrap_or(0);
            }
            Err(e) => {
                teller_failed = 1;
                error_event(&format!(
                    "scheduled teller sync failed: {}",
                    short_reason(&e)
                ));
            }
        }
    }

    info_event(&format!(
        "scheduled bank sync finished plaid_ok={plaid_ok} plaid_failed={plaid_failed} teller_ok={teller_ok} teller_failed={teller_failed}"
    ));
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    info_event("server shutting down");
}

/// Pages live in `static/` next to the working directory (local runs and the
/// container) or next to the executable (the installed Windows app).
pub fn static_dir() -> PathBuf {
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
