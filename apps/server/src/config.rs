//! Server configuration from environment variables (with `.env` support).

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub bind: String,
    pub media_dir: PathBuf,
    pub max_upload_bytes: usize,
    pub policy_version: String,
    /// Session lifetime in days.
    pub session_days: i64,
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).ok().filter(|v| !v.is_empty()).unwrap_or_else(|| default.to_string())
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: env_or("DATABASE_URL", "postgres://postgres@localhost:5432/nook"),
            bind: env_or("NOOK_BIND", "0.0.0.0:8080"),
            media_dir: PathBuf::from(env_or("NOOK_MEDIA_DIR", "./data/media")),
            max_upload_bytes: env_or("NOOK_MAX_UPLOAD_BYTES", "10485760")
                .parse()
                .unwrap_or(10 * 1024 * 1024),
            policy_version: env_or("NOOK_POLICY_VERSION", nook_shared::policies::POLICY_VERSION),
            session_days: env_or("NOOK_SESSION_DAYS", "90").parse().unwrap_or(90),
        }
    }
}
