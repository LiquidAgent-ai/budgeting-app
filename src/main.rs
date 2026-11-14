mod categories;
mod config;
mod db;
mod finance;
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
use std::sync::Arc;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    log::init();

    let config = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    let db = match Db::open(config.database_path.clone(), config.database_key.clone()) {
        Ok(db) => Arc::new(db),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    // Load demo household on first run when the file did not already exist.
    if !db.existed_at_startup() {
        match seed::seed_database(&db, false) {
            Ok((true, _)) => info_event("demo household seeded"),
            Ok((false, _)) => {}
            Err(_) => crate::log::error_event("demo seed failed"),
        }
    }

    let state = AppState {
        db,
        config: config.clone(),
    };

    let app = Router::new()
        .merge(router(state))
        .nest_service("/static", ServeDir::new("static"));

    let addr: SocketAddr = format!("{}:{}", config.bind_host, config.port)
        .parse()
        .expect("bind address");
    info_event(&format!(
        "server starting host={} port={} container={}",
        config.bind_host,
        config.port,
        if config.in_container { "yes" } else { "no" }
    ));

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| {
            eprintln!("could not bind {addr}: {e}");
            std::process::exit(1);
        });

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap_or_else(|e| {
            eprintln!("server error: {e}");
            std::process::exit(1);
        });
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    info_event("server shutting down");
}
