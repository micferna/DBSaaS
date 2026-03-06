use std::env;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub redis_url: String,
    pub host: String,
    pub port: u16,
    pub jwt_secret: String,
    pub encryption_key: String,
    pub docker_host: Option<String>,
    pub port_range_start: u16,
    pub port_range_end: u16,
    pub max_databases_per_user: i32,
    pub registration_enabled: bool,
    pub tls_ca_dir: String,
    pub traefik_dynamic_dir: String,
    pub platform_domain: String,
    pub backup_dir: String,
    pub stripe_secret_key: Option<String>,
    pub stripe_webhook_secret: Option<String>,
    pub platform_ip: String,
    pub cors_origins: Option<Vec<String>>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<u16>,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    pub smtp_from: Option<String>,
}

/// Read a Docker secret from /run/secrets/<name>, falling back to env var.
/// Returns None if neither exists.
fn read_secret(name: &str) -> Option<String> {
    let secret_path = format!("/run/secrets/{}", name);
    if Path::new(&secret_path).exists() {
        if let Ok(val) = fs::read_to_string(&secret_path) {
            let trimmed = val.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }
    env::var(name.to_uppercase()).ok()
}

/// Read a Docker secret, panic if not found anywhere.
fn require_secret(name: &str) -> String {
    read_secret(name).unwrap_or_else(|| {
        panic!(
            "{} must be set via Docker secret (/run/secrets/{}) or environment variable ({})",
            name.to_uppercase(),
            name,
            name.to_uppercase()
        )
    })
}

impl Config {
    pub fn from_env() -> Self {
        // Secrets (prefer /run/secrets/ files, fallback to env vars)
        let database_url = read_secret("database_url")
            .unwrap_or_else(|| {
                // Build from components if individual secrets exist
                if let (Some(user), Some(pass), Some(db)) = (
                    read_secret("postgres_user"),
                    read_secret("postgres_password"),
                    read_secret("postgres_db"),
                ) {
                    let host = env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".to_string());
                    format!("postgres://{}:{}@{}:5432/{}", user, pass, host, db)
                } else {
                    "postgres://myuser:password@localhost:5432/mydb".to_string()
                }
            });

        let redis_url = read_secret("redis_url")
            .unwrap_or_else(|| {
                if let Some(pass) = read_secret("redis_password") {
                    let host = env::var("REDIS_HOST").unwrap_or_else(|_| "localhost".to_string());
                    format!("redis://:{}@{}:6379", pass, host)
                } else {
                    "redis://:password@localhost:6379".to_string()
                }
            });

        let jwt_secret = require_secret("jwt_secret");
        let encryption_key = require_secret("encryption_key");

        let stripe_secret_key = read_secret("stripe_secret_key")
            .filter(|s| !s.is_empty());
        let stripe_webhook_secret = read_secret("stripe_webhook_secret")
            .filter(|s| !s.is_empty());

        Self {
            database_url,
            redis_url,
            host: env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("API_PORT")
                .unwrap_or_else(|_| "3001".to_string())
                .parse()
                .expect("API_PORT must be a number"),
            jwt_secret,
            encryption_key,
            docker_host: env::var("DOCKER_HOST").ok(),
            port_range_start: env::var("PORT_RANGE_START")
                .unwrap_or_else(|_| "10000".to_string())
                .parse()
                .expect("PORT_RANGE_START must be a number"),
            port_range_end: env::var("PORT_RANGE_END")
                .unwrap_or_else(|_| "60000".to_string())
                .parse()
                .expect("PORT_RANGE_END must be a number"),
            max_databases_per_user: env::var("MAX_DATABASES_PER_USER")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .expect("MAX_DATABASES_PER_USER must be a number"),
            registration_enabled: env::var("REGISTRATION_ENABLED")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            tls_ca_dir: env::var("TLS_CA_DIR").unwrap_or_else(|_| "./tls/ca".to_string()),
            traefik_dynamic_dir: env::var("TRAEFIK_DYNAMIC_DIR")
                .unwrap_or_else(|_| "./traefik/dynamic".to_string()),
            platform_domain: env::var("PLATFORM_DOMAIN")
                .unwrap_or_else(|_| "localhost".to_string()),
            backup_dir: env::var("BACKUP_DIR")
                .unwrap_or_else(|_| "./backups".to_string()),
            platform_ip: env::var("PLATFORM_IP")
                .unwrap_or_else(|_| "127.0.0.1".to_string()),
            stripe_secret_key,
            stripe_webhook_secret,
            cors_origins: env::var("CORS_ORIGINS").ok().map(|s| {
                s.split(',').map(|o| o.trim().to_string()).collect()
            }),
            smtp_host: env::var("SMTP_HOST").ok().filter(|s| !s.is_empty()),
            smtp_port: env::var("SMTP_PORT").ok().and_then(|s| s.parse().ok()),
            smtp_username: env::var("SMTP_USERNAME").ok().filter(|s| !s.is_empty()),
            smtp_password: read_secret("smtp_password"),
            smtp_from: env::var("SMTP_FROM").ok().filter(|s| !s.is_empty()),
        }
    }

    /// DNS zone for database subdomains (e.g. "db.example.com")
    pub fn dns_zone(&self) -> String {
        format!("db.{}", self.platform_domain)
    }
}
