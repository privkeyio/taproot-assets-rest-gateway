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
use actix_web::http::StatusCode;
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

/// Deserializes a tapd response, surfacing non-2xx statuses as errors instead
/// of relaying the upstream error body with a 200.
pub async fn parse_upstream<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T, AppError> {
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::UpstreamError {
            status: status.as_u16(),
            body,
        });
    }
    response.json::<T>().await.map_err(AppError::RequestError)
}

pub fn handle_result<T: serde::Serialize>(result: Result<T, AppError>) -> HttpResponse {
    match result {
        Ok(value) => HttpResponse::Ok().json(value),
        // Relay tapd's own error document unchanged so callers keep reading the
        // `code`/`message` fields it defines; only the status is corrected.
        Err(AppError::UpstreamError { status, body }) => {
            let status = StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
            match serde_json::from_str::<serde_json::Value>(&body) {
                Ok(json) => HttpResponse::build(status).json(json),
                Err(_) => HttpResponse::build(status).json(serde_json::json!({ "error": body })),
            }
        }
        Err(e) => {
            let status = e.status_code();
            HttpResponse::build(status).json(serde_json::json!({
                "error": e.to_string()
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_of(len: usize) -> String {
        "a".repeat(len)
    }

    #[test]
    fn test_validate_asset_id_requires_64_hex_chars() {
        assert!(validate_asset_id(&hex_of(64)).is_ok());
        assert!(validate_asset_id(&hex_of(63)).is_err());
        assert!(validate_asset_id(&hex_of(66)).is_err());
        assert!(validate_asset_id("").is_err());
        assert!(validate_asset_id(&"z".repeat(64)).is_err());
    }

    #[test]
    fn test_validate_group_key_accepts_x_only_and_compressed() {
        assert!(validate_group_key(&hex_of(64)).is_ok());
        assert!(validate_group_key(&hex_of(66)).is_ok());
        assert!(validate_group_key(&hex_of(65)).is_err());
        assert!(validate_group_key("not-hex").is_err());
    }

    #[test]
    fn test_validate_group_key_rejects_traversal() {
        assert!(validate_group_key("../../etc/passwd").is_err());
        assert!(validate_group_key("%2e%2e%2fadmin").is_err());
    }

    async fn body_of(resp: HttpResponse) -> serde_json::Value {
        let bytes = actix_web::body::to_bytes(resp.into_body()).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[actix_rt::test]
    async fn test_handle_result_relays_upstream_status_and_body() {
        let resp = handle_result::<serde_json::Value>(Err(AppError::UpstreamError {
            status: 400,
            body: r#"{"code":3,"message":"invalid confirmation text"}"#.to_string(),
        }));
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body = body_of(resp).await;
        assert_eq!(body["code"], 3);
        assert_eq!(body["message"], "invalid confirmation text");
    }

    #[actix_rt::test]
    async fn test_handle_result_wraps_non_json_upstream_body() {
        let resp = handle_result::<serde_json::Value>(Err(AppError::UpstreamError {
            status: 503,
            body: "upstream down".to_string(),
        }));
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body_of(resp).await["error"], "upstream down");
    }

    #[actix_rt::test]
    async fn test_handle_result_never_returns_ok_for_errors() {
        let resp = handle_result::<serde_json::Value>(Err(AppError::InvalidInput("bad".into())));
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[actix_rt::test]
    async fn test_handle_result_passes_success_through() {
        let resp = handle_result(Ok(serde_json::json!({"ok": true})));
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(body_of(resp).await["ok"], true);
    }
}
