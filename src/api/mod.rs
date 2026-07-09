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

const ASSET_ID_HEX_LEN: usize = 64;
const GROUP_KEY_HEX_LENS: [usize; 2] = [64, 66];

pub fn validate_asset_id(value: &str) -> Result<(), AppError> {
    validate_hex_param(value)?;
    if value.len() != ASSET_ID_HEX_LEN {
        return Err(AppError::InvalidInput(format!(
            "Invalid asset ID: expected {ASSET_ID_HEX_LEN} hex characters, got {}",
            value.len()
        )));
    }
    Ok(())
}

/// tapd accepts a group key as either a 32-byte x-only or a 33-byte
/// compressed public key, so both hex lengths are valid.
pub fn validate_group_key(value: &str) -> Result<(), AppError> {
    validate_hex_param(value)?;
    if !GROUP_KEY_HEX_LENS.contains(&value.len()) {
        return Err(AppError::InvalidInput(format!(
            "Invalid group key: expected 64 or 66 hex characters, got {}",
            value.len()
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
