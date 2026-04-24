//! Runtime configuration loaded from environment variables.
//!
//! Call [`Config::from_env`] once at startup and share via `Arc<Config>`.

use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub es_url: String,
    pub jwt_secret: String,
    pub session_ttl_min: i64,
    pub database_url: String,
    pub bind_addr: String,
    pub max_upload_bytes: usize,
    pub max_docs_per_index: usize,
    pub frontend_origin: String,
    pub admin_token: Option<String>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let _ = dotenvy::dotenv();

        let jwt_secret = env::var("JWT_SECRET")
            .map_err(|_| anyhow::anyhow!("JWT_SECRET is required"))?;
        if jwt_secret.len() < 16 {
            anyhow::bail!("JWT_SECRET must be at least 16 characters");
        }

        Ok(Self {
            es_url: env::var("ES_URL").unwrap_or_else(|_| "http://localhost:9200".into()),
            jwt_secret,
            session_ttl_min: env::var("SESSION_TTL_MIN")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite://data.db?mode=rwc".into()),
            bind_addr: env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            max_upload_bytes: env::var("MAX_UPLOAD_BYTES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(50 * 1024 * 1024),
            max_docs_per_index: env::var("MAX_DOCS_PER_INDEX")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100_000),
            frontend_origin: env::var("FRONTEND_ORIGIN")
                .unwrap_or_else(|_| "http://localhost:5173".into()),
            admin_token: env::var("ADMIN_TOKEN").ok().filter(|s| !s.is_empty()),
        })
    }
}
