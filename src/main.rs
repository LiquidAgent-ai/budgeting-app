#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]

mod api;
mod config;
mod db;
mod desktop;
mod import;
mod plaid;
mod server;
mod ui;

use crate::config::Config;

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
    config::logging::init();
    let config = Config::from_env()?;
    server::block_on_serve(config)
}
