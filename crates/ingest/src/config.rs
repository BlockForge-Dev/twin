/// Application configuration loaded from environment variables with sensible defaults.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// PostgreSQL connection string.
    pub database_url: String,
    /// Host address to bind the HTTP server to.
    pub host: String,
    /// Port to bind the HTTP server to.
    pub port: u16,
    /// Tracing filter level (e.g. "info", "debug").
    pub log_level: String,
    /// Log format ("text" or "json").
    pub log_format: String,
}

impl AppConfig {
    /// Load configuration from environment variables.
    ///
    /// Optional with sensible defaults:
    /// - `DATABASE_URL` (default: "postgres://cs_dev:cs_dev_pass@localhost:5432/cylindersense")
    /// - `HOST` (default: "0.0.0.0")
    /// - `PORT` (default: 3000)
    /// - `LOG_LEVEL` (default: "info")
    /// - `LOG_FORMAT` (default: "text")
    pub fn from_env() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://cs_dev:cs_dev_pass@localhost:5432/cylindersense".to_string()
            }),
            host: std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "3001".to_string())
                .parse()
                .unwrap_or(3001),
            log_level: std::env::var("LOG_LEVEL")
                .or_else(|_| std::env::var("RUST_LOG"))
                .unwrap_or_else(|_| "info".to_string()),
            log_format: std::env::var("LOG_FORMAT").unwrap_or_else(|_| "text".to_string()),
        }
    }
}
