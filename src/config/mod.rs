pub mod logging;

use std::env;
use std::path::PathBuf;

const KEY_PLACEHOLDER: &str = "your_database_key";

#[derive(Clone)]
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
    pub teller_application_id: Option<String>,
    pub teller_env: String,
    pub teller_cert_path: Option<PathBuf>,
    pub teller_key_path: Option<PathBuf>,
    pub teller_cert_pem: Option<String>,
    pub teller_key_pem: Option<String>,
    pub in_container: bool,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        load_dotenv_keys();

        let key = env::var("DATABASE_KEY")
            .unwrap_or_default()
            .trim()
            .to_string();
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

        let plaid_env = match env::var("PLAID_ENV")
            .unwrap_or_else(|_| "sandbox".into())
            .as_str()
        {
            "development" => "development".into(),
            "production" => "production".into(),
            _ => "sandbox".into(),
        };

        let app_url = env::var("APP_URL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let teller_application_id = env::var("TELLER_APPLICATION_ID")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s != "your_application_id");
        let teller_env = match env::var("TELLER_ENV")
            .unwrap_or_else(|_| "sandbox".into())
            .as_str()
        {
            "development" => "development".into(),
            "production" => "production".into(),
            _ => "sandbox".into(),
        };
        let teller_cert_path = env::var("TELLER_CERT_PATH")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from);
        let teller_key_path = env::var("TELLER_KEY_PATH")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from);
        let teller_cert_pem = env::var("TELLER_CERT_PEM")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let teller_key_pem = env::var("TELLER_KEY_PEM")
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
            teller_application_id,
            teller_env,
            teller_cert_path,
            teller_key_path,
            teller_cert_pem,
            teller_key_pem,
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

    pub fn teller_configured(&self) -> bool {
        self.teller_application_id.is_some()
    }

    pub fn bank_linking_configured(&self) -> bool {
        self.plaid_configured() || self.teller_configured()
    }

    pub fn teller_certificate_ready(&self) -> bool {
        if self.teller_env == "sandbox" {
            return true;
        }
        let has_paths = self.teller_cert_path.is_some() && self.teller_key_path.is_some();
        let has_pem = self.teller_cert_pem.is_some() && self.teller_key_pem.is_some();
        has_paths || has_pem
    }

    /// Client certificate identity for Teller mTLS (development/production).
    pub fn teller_identity(&self) -> Result<Option<reqwest::Identity>, String> {
        let cert = if let Some(pem) = &self.teller_cert_pem {
            pem.clone()
        } else if let Some(path) = &self.teller_cert_path {
            std::fs::read_to_string(path).map_err(|_| {
                "Could not read TELLER_CERT_PATH".to_string()
            })?
        } else {
            return Ok(None);
        };
        let key = if let Some(pem) = &self.teller_key_pem {
            pem.clone()
        } else if let Some(path) = &self.teller_key_path {
            std::fs::read_to_string(path).map_err(|_| {
                "Could not read TELLER_KEY_PATH".to_string()
            })?
        } else {
            return Ok(None);
        };
        let mut combined = cert;
        if !combined.ends_with('\n') {
            combined.push('\n');
        }
        combined.push_str(&key);
        let identity = reqwest::Identity::from_pem(combined.as_bytes())
            .map_err(|_| "Teller certificate or key is invalid".to_string())?;
        Ok(Some(identity))
    }
}

/// Fill selected keys from .env.local or .env when unset.
fn load_dotenv_keys() {
    const KEYS: &[&str] = &[
        "DATABASE_KEY",
        "DATABASE_PATH",
        "PLAID_CLIENT_ID",
        "PLAID_SECRET",
        "PLAID_ENV",
        "TELLER_APPLICATION_ID",
        "TELLER_ENV",
        "TELLER_CERT_PATH",
        "TELLER_KEY_PATH",
        "TELLER_CERT_PEM",
        "TELLER_KEY_PEM",
        "APP_URL",
        "HOST",
        "PORT",
    ];
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
            if !KEYS.contains(&key) {
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
                // SAFETY: single-threaded at startup, before any other thread
                // reads the environment.
                unsafe { env::set_var(key, value) };
            }
        }
    }
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("database_path", &self.database_path)
            .field("database_key", &"[redacted]")
            .field("bind_host", &self.bind_host)
            .field("port", &self.port)
            .field("app_url", &self.app_url)
            .field(
                "plaid_client_id",
                &self.plaid_client_id.as_ref().map(|_| "[redacted]"),
            )
            .field(
                "plaid_secret",
                &self.plaid_secret.as_ref().map(|_| "[redacted]"),
            )
            .field("plaid_env", &self.plaid_env)
            .field(
                "teller_application_id",
                &self.teller_application_id.as_ref().map(|_| "[redacted]"),
            )
            .field("teller_env", &self.teller_env)
            .field("teller_cert_path", &self.teller_cert_path)
            .field("teller_key_path", &self.teller_key_path)
            .field(
                "teller_cert_pem",
                &self.teller_cert_pem.as_ref().map(|_| "[redacted]"),
            )
            .field(
                "teller_key_pem",
                &self.teller_key_pem.as_ref().map(|_| "[redacted]"),
            )
            .field("in_container", &self.in_container)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_does_not_include_secrets() {
        let secret_key = "super-secret-database-key-value";
        let plaid = "plaid-secret-value-not-for-logs";
        let client = "plaid-client-id-value";
        let teller_app = "app_teller_application_id_secret";
        let teller_pem = "-----BEGIN CERTIFICATE-----\nteller-cert\n-----END CERTIFICATE-----";
        let cfg = Config {
            database_path: PathBuf::from("data/anchored.db"),
            database_key: secret_key.into(),
            bind_host: "127.0.0.1".into(),
            port: 3000,
            app_url: None,
            plaid_client_id: Some(client.into()),
            plaid_secret: Some(plaid.into()),
            plaid_env: "sandbox".into(),
            teller_application_id: Some(teller_app.into()),
            teller_env: "sandbox".into(),
            teller_cert_path: None,
            teller_key_path: None,
            teller_cert_pem: Some(teller_pem.into()),
            teller_key_pem: Some("teller-key-pem-value".into()),
            in_container: false,
        };
        let text = format!("{cfg:?}");
        assert!(!text.contains(secret_key));
        assert!(!text.contains(plaid));
        assert!(!text.contains(client));
        assert!(!text.contains(teller_app));
        assert!(!text.contains("teller-cert"));
        assert!(!text.contains("teller-key-pem-value"));
        assert!(text.contains("redacted"));
    }
}
