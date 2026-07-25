use std::path::PathBuf;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 4200;

pub struct Config {
    pub db_path: PathBuf,
    pub host: String,
    pub port: u16,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            db_path: parse_env_or("YOJANA_DB_PATH", default_db_path()),
            host: parse_env_or("YOJANA_HOST", DEFAULT_HOST.into()),
            port: parse_env_or("YOJANA_PORT", DEFAULT_PORT),
        }
    }

    /// Config for an explicit database path, with defaults for everything else.
    ///
    /// Lets callers (notably tests) get a `Config` without mutating the process
    /// environment, which is a data race when tests run in parallel threads.
    pub fn for_path(db_path: impl Into<PathBuf>) -> Self {
        Self {
            db_path: db_path.into(),
            host: DEFAULT_HOST.into(),
            port: DEFAULT_PORT,
        }
    }

    pub fn pid_path(&self) -> PathBuf {
        self.db_path
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join("yojana.pid")
    }
}

fn default_db_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".yojana").join("yojana.db")
}

fn parse_env_or<T: std::str::FromStr + std::fmt::Debug>(name: &str, default: T) -> T {
    match std::env::var(name) {
        Ok(val) => match val.parse() {
            Ok(parsed) => parsed,
            Err(_) => {
                tracing::warn!("invalid {name}={val}, using default {default:?}");
                default
            }
        },
        Err(_) => default,
    }
}
