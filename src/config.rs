use std::env;
use std::path::PathBuf;

const KEY_PLACEHOLDER: &str = "your_database_key";

#[derive(Clone, Debug)]
pub struct Config {
    pub database_path: PathBuf,
    pub database_key: String,
    pub bind_host: String,
    pub port: u16,
    #[allow(dead_code)]
    pub app_url: Option<String>,
    pub plaid_client_id: Option<String>,
    pub plaid_secret: Option<String>,
    pub plaid_env: String,
    pub in_container: bool,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        load_dotenv_keys();

        let key = env::var("DATABASE_KEY").unwrap_or_default().trim().to_string();
        if key.len() < 16 || key == KEY_PLACEHOLDER {
            return Err(
                "DATABASE_KEY is missing. Anchored will not open or create the database. Set a long random DATABASE_KEY in the environment or in .env.local."
                    .into(),
            );
        }

        let database_path = env::var("DATABASE_PATH")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("data/anchored.db"));

        let in_container = env::var("ANCHORED_CONTAINER").ok().as_deref() == Some("1");
        let bind_host = if in_container {
            "0.0.0.0".into()
        } else {
            env::var("HOST")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "127.0.0.1".into())
        };

        let port: u16 = env::var("PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3000);

        let plaid_client_id = env::var("PLAID_CLIENT_ID")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s != "your_client_id");
        let plaid_secret = env::var("PLAID_SECRET")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s != "your_sandbox_secret");

        let plaid_env = match env::var("PLAID_ENV").unwrap_or_else(|_| "sandbox".into()).as_str() {
            "development" => "development".into(),
            "production" => "production".into(),
            _ => "sandbox".into(),
        };

        let app_url = env::var("APP_URL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        Ok(Self {
            database_path,
            database_key: key,
            bind_host,
            port,
            app_url,
            plaid_client_id,
            plaid_secret,
            plaid_env,
            in_container,
        })
    }

    pub fn plaid_configured(&self) -> bool {
        self.plaid_client_id.is_some() && self.plaid_secret.is_some()
    }

    pub fn plaid_base_url(&self) -> &'static str {
        match self.plaid_env.as_str() {
            "development" => "https://development.plaid.com",
            "production" => "https://production.plaid.com",
            _ => "https://sandbox.plaid.com",
        }
    }
}

/// Fill DATABASE_KEY / DATABASE_PATH from .env.local or .env when unset.
fn load_dotenv_keys() {
    for name in [".env.local", ".env"] {
        let Ok(text) = std::fs::read_to_string(name) else {
            continue;
        };
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let Some((key, value)) = trimmed.split_once('=') else {
                continue;
            };
            let key = key.trim();
            if key != "DATABASE_KEY" && key != "DATABASE_PATH" {
                continue;
            }
            if env::var_os(key).is_some_and(|v| !v.is_empty()) {
                continue;
            }
            let mut value = value.trim().to_string();
            if (value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\''))
            {
                value = value[1..value.len() - 1].to_string();
            }
            if !value.is_empty() {
                // SAFETY: single-threaded at startup before the server starts.
                env::set_var(key, value);
            }
        }
    }
}
