use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError};
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
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("WebSocket error: {0}")]
    #[allow(dead_code)]
    WebSocketError(String),
    #[error("WebSocket proxy error: {0}")]
    #[allow(dead_code)]
    WebSocketProxyError(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
}

impl ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        self.status_code()
    }

    fn error_response(&self) -> HttpResponse {
        let (message, error_type) = match self {
            AppError::ValidationError(msg) => (msg.clone(), "validation_error"),
            AppError::InvalidInput(msg) => (msg.clone(), "invalid_input"),
            AppError::RequestError(e) => {
                if e.is_timeout() {
                    ("Request timed out".to_string(), "timeout")
                } else if e.is_connect() {
                    (
                        "Service temporarily unavailable".to_string(),
                        "service_unavailable",
                    )
                } else {
                    (
                        "An error occurred processing your request".to_string(),
                        "request_error",
                    )
                }
            }
            AppError::JsonError(_) => ("Invalid JSON format".to_string(), "json_error"),
            AppError::IoError(_) => ("Internal server error".to_string(), "internal_error"),
            AppError::HexError(_) => ("Invalid hex encoding".to_string(), "encoding_error"),
            AppError::EnvVarError(_) => ("Server configuration error".to_string(), "config_error"),
            AppError::SerializationError(_) => (
                "Data serialization error".to_string(),
                "serialization_error",
            ),
            AppError::WebSocketError(_) => {
                ("WebSocket connection error".to_string(), "websocket_error")
            }
            AppError::WebSocketProxyError(_) => {
                ("WebSocket proxy error".to_string(), "proxy_error")
            }
            AppError::DatabaseError(_) => {
                ("Database operation failed".to_string(), "database_error")
            }
        };

        HttpResponse::build(self.status_code()).json(serde_json::json!({
            "error": message,
            "type": error_type
        }))
    }
}

impl AppError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            AppError::ValidationError(_) => StatusCode::BAD_REQUEST,
            AppError::InvalidInput(_) => StatusCode::BAD_REQUEST,
            AppError::SerializationError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::WebSocketError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::WebSocketProxyError(_) => StatusCode::BAD_GATEWAY,
            AppError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
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
