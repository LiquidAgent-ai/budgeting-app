//! Sparse troubleshooting logs. Real events only — no amounts, payees, tokens, or account numbers.

use tracing::{error, info};

pub fn init() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .with_level(true)
        .compact()
        .init();
}

pub fn info_event(msg: &str) {
    info!("{msg}");
}

pub fn error_event(msg: &str) {
    error!("{msg}");
}
