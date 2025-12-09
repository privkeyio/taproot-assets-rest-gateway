use crate::error::AppError;
use serde::Deserialize;
use std::path::Path;

#[derive(Clone, Deserialize)]
pub struct Config {
    pub taproot_assets_host: String,
    pub macaroon_path: String,
    #[allow(dead_code)]
    pub lnd_macaroon_path: String,
    pub tls_verify: bool,
    pub cors_origins: Vec<String>,
    pub server_address: String,
    pub request_timeout_secs: u64,
    pub rate_limit_per_minute: usize,
    pub rfq_poll_interval_secs: u64,
}

impl Config {
    pub fn load() -> Result<Self, AppError> {
        // Load host configuration
        let taproot_assets_host =
            std::env::var("TAPROOT_ASSETS_HOST").unwrap_or_else(|_| "127.0.0.1:8289".to_string());

        // Load authentication paths
        let macaroon_path = std::env::var("TAPD_MACAROON_PATH").map_err(AppError::EnvVarError)?;
        let lnd_macaroon_path =
            std::env::var("LND_MACAROON_PATH").map_err(AppError::EnvVarError)?;

        // Security settings - TLS verification defaults to true for production safety
        let tls_verify = std::env::var("TLS_VERIFY")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .unwrap_or(true);

        // CORS configuration
        let cors_origins = std::env::var("CORS_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:5173,http://127.0.0.1:5173".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();

        // Server configuration
        let server_address =
            std::env::var("SERVER_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8080".to_string());

        // Request timeout configuration
        let request_timeout_secs = std::env::var("REQUEST_TIMEOUT_SECS")
            .unwrap_or_else(|_| "30".to_string())
            .parse::<u64>()
            .unwrap_or(30);

        // Rate limiting configuration
        let rate_limit_per_minute = std::env::var("RATE_LIMIT_PER_MINUTE")
            .unwrap_or_else(|_| "100".to_string())
            .parse::<usize>()
            .unwrap_or(100);

        // RFQ polling interval configuration
        let rfq_poll_interval_secs = std::env::var("RFQ_POLL_INTERVAL_SECS")
            .unwrap_or_else(|_| "5".to_string())
            .parse::<u64>()
            .unwrap_or(5);

        // Validate paths exist
        if !Path::new(&macaroon_path).exists() {
            return Err(AppError::ValidationError(format!(
                "Tapd macaroon file does not exist at path: {macaroon_path}. Please check TAPD_MACAROON_PATH in your .env file."
            )));
        }
        if !Path::new(&lnd_macaroon_path).exists() {
            return Err(AppError::ValidationError(format!(
                "LND macaroon file does not exist at path: {lnd_macaroon_path}. Please check LND_MACAROON_PATH in your .env file."
            )));
        }

        let config = Config {
            taproot_assets_host,
            macaroon_path,
            lnd_macaroon_path,
            tls_verify,
            cors_origins,
            server_address,
            request_timeout_secs,
            rate_limit_per_minute,
            rfq_poll_interval_secs,
        };

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    pub fn validate(&self) -> Result<(), AppError> {
        // Validate host configuration
        if self.taproot_assets_host.is_empty() {
            return Err(AppError::ValidationError(
                "TAPROOT_ASSETS_HOST cannot be empty".to_string(),
            ));
        }

        // Validate host format
        if !self.taproot_assets_host.contains(':') {
            return Err(AppError::ValidationError(
                "TAPROOT_ASSETS_HOST must include port (e.g., 127.0.0.1:8289)".to_string(),
            ));
        }

        // Validate server address format
        if !self.server_address.contains(':') {
            return Err(AppError::ValidationError(
                "SERVER_ADDRESS must include port (e.g., 127.0.0.1:8080)".to_string(),
            ));
        }

        if self.request_timeout_secs == 0 {
            return Err(AppError::ValidationError(
                "REQUEST_TIMEOUT_SECS must be greater than 0".to_string(),
            ));
        }
        if self.request_timeout_secs > 300 {
            return Err(AppError::ValidationError(
                "REQUEST_TIMEOUT_SECS must not exceed 300 seconds".to_string(),
            ));
        }

        if self.rate_limit_per_minute == 0 {
            return Err(AppError::ValidationError(
                "RATE_LIMIT_PER_MINUTE must be greater than 0".to_string(),
            ));
        }
        if self.rate_limit_per_minute > 10000 {
            return Err(AppError::ValidationError(
                "RATE_LIMIT_PER_MINUTE must not exceed 10000".to_string(),
            ));
        }

        if self.rfq_poll_interval_secs == 0 {
            return Err(AppError::ValidationError(
                "RFQ_POLL_INTERVAL_SECS must be greater than 0".to_string(),
            ));
        }
        if self.rfq_poll_interval_secs > 60 {
            return Err(AppError::ValidationError(
                "RFQ_POLL_INTERVAL_SECS must not exceed 60 seconds".to_string(),
            ));
        }

        // Warn about security settings in production
        if !self.tls_verify {
            eprintln!("⚠️  WARNING: TLS verification is disabled. This should only be used in development!");
        }

        // Validate CORS origins
        for origin in &self.cors_origins {
            if origin.is_empty() {
                return Err(AppError::ValidationError(
                    "CORS origins cannot contain empty strings".to_string(),
                ));
            }
            // Basic URL validation
            if !origin.starts_with("http://") && !origin.starts_with("https://") {
                return Err(AppError::ValidationError(format!(
                    "CORS origin must be a valid URL: {origin}"
                )));
            }
        }

        Ok(())
    }
}
