use actix_web::http::StatusCode;
use thiserror::Error;

#[derive(Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum AppError {
    #[error("Request error: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Hex error: {0}")]
    HexError(#[from] hex::FromHexError),
    #[error("Environment variable error: {0}")]
    EnvVarError(#[from] std::env::VarError),
    #[error("Validation error: {0}")]
    ValidationError(String),
    #[error("WebSocket error: {0}")]
    WebSocketError(String),
    #[error("WebSocket proxy error: {0}")]
    WebSocketProxyError(String),
}

impl AppError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            AppError::ValidationError(_) => StatusCode::BAD_REQUEST,
            AppError::WebSocketError(_) => StatusCode::BAD_REQUEST,
            AppError::WebSocketProxyError(_) => StatusCode::BAD_GATEWAY,
            AppError::RequestError(e) => {
                if e.is_timeout() {
                    StatusCode::GATEWAY_TIMEOUT
                } else if e.is_connect() {
                    StatusCode::BAD_GATEWAY
                } else {
                    StatusCode::INTERNAL_SERVER_ERROR
                }
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
