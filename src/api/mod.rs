pub mod addresses;
pub mod assets;
pub mod burn;
pub mod channels;
pub mod events;
pub mod health;
pub mod info;
pub mod mailbox;
pub mod mailbox_auth;
pub mod proofs;
pub mod rfq;
pub mod routes;
pub mod send;
pub mod stop;
pub mod universe;
pub mod wallet;

use crate::error::AppError;
use actix_web::HttpResponse;

pub fn validate_hex_param(value: &str) -> Result<(), AppError> {
    if value.is_empty()
        || value.contains('/')
        || value.contains("..")
        || value.contains("%2F")
        || value.contains("%2f")
        || value.contains("%2E")
        || value.contains("%2e")
        || !value.chars().all(|c| c.is_ascii_hexdigit())
    {
        return Err(AppError::InvalidInput(format!(
            "Invalid path parameter: {}",
            value
        )));
    }
    Ok(())
}

#[allow(dead_code)]
pub fn validate_path_param(value: &str) -> Result<(), AppError> {
    if value.is_empty()
        || value.contains('/')
        || value.contains("..")
        || value.contains("%2F")
        || value.contains("%2f")
        || value.contains("%2E")
        || value.contains("%2e")
    {
        return Err(AppError::InvalidInput(format!(
            "Invalid path parameter: {}",
            value
        )));
    }
    Ok(())
}

pub fn validate_integer_param(value: &str) -> Result<(), AppError> {
    if value.parse::<u64>().is_err() {
        return Err(AppError::InvalidInput(format!(
            "Invalid integer parameter: {}",
            value
        )));
    }
    Ok(())
}

pub fn handle_result<T: serde::Serialize>(result: Result<T, AppError>) -> HttpResponse {
    match result {
        Ok(value) => HttpResponse::Ok().json(value),
        Err(e) => {
            let status = e.status_code();
            HttpResponse::build(status).json(serde_json::json!({
                "error": e.to_string()
            }))
        }
    }
}
