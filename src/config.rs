use std::path::PathBuf;

pub struct Config {
    pub db_path: PathBuf,
    pub host: String,
    pub port: u16,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            db_path: parse_env_or("YOJANA_DB_PATH", default_db_path()),
            host: parse_env_or("YOJANA_HOST", "127.0.0.1".into()),
            port: parse_env_or("YOJANA_PORT", 4200),
        }
    }

    pub fn pid_path(&self) -> PathBuf {
        self.db_path.parent().unwrap_or(std::path::Path::new(".")).join("yojana.pid")
    }
}

fn default_db_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".yojana").join("yojana.db")
}

fn parse_env_or<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
