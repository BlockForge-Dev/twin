/// Application configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// PostgreSQL connection string.
    pub database_url: String,
    /// Host address to bind the HTTP server to.
    pub host: String,
    /// Port to bind the HTTP server to.
    pub port: u16,
}

impl AppConfig {
    /// Load configuration from environment variables.
    ///
    /// Required: `DATABASE_URL`
    /// Optional: `HOST` (default "0.0.0.0"), `PORT` (default 3000)
    pub fn from_env() -> Result<Self, std::env::VarError> {
        Ok(Self {
            database_url: std::env::var("DATABASE_URL")?,
            host: std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .expect("PORT must be a valid u16"),
        })
    }
}
