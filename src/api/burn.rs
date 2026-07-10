use super::{handle_result, parse_upstream, validate_asset_id, validate_group_key};
use crate::error::AppError;
use crate::types::{BaseUrl, MacaroonHex};
use actix_web::{web, HttpResponse};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

#[derive(Debug, Serialize, Deserialize)]
pub struct AssetSpecifier {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_id_str: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_key_str: Option<String>,
}

impl AssetSpecifier {
    fn validate(&self) -> Result<(), AppError> {
        match (self.asset_id_str.as_deref(), self.group_key_str.as_deref()) {
            (Some(_), Some(_)) => Err(AppError::InvalidInput(
                "asset_specifier must set exactly one of asset_id_str or group_key_str".to_string(),
            )),
            (None, None) => Err(AppError::InvalidInput(
                "asset_specifier must set either asset_id_str or group_key_str".to_string(),
            )),
            (Some(asset_id), None) => validate_asset_id(asset_id),
            (None, Some(group_key)) => validate_group_key(group_key),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BurnRequest {
    pub asset_specifier: AssetSpecifier,
    pub amount_to_burn: String,
    pub confirmation_text: String,
    pub note: Option<String>,
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn burn_assets(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: BurnRequest,
) -> Result<serde_json::Value, AppError> {
    request.asset_specifier.validate()?;
    info!(
        asset_id_str = ?request.asset_specifier.asset_id_str,
        group_key_str = ?request.asset_specifier.group_key_str,
        amount_to_burn = %request.amount_to_burn,
        "Burning assets"
    );
    let url = format!("{base_url}/v1/taproot-assets/burn");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await
        .map_err(AppError::RequestError)?;
    parse_upstream::<serde_json::Value>(response).await
}

#[instrument(skip(client, macaroon_hex))]
pub async fn list_burns(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<serde_json::Value, AppError> {
    info!("Listing burns");
    let url = format!("{base_url}/v1/taproot-assets/burns");
    let response = client
        .get(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .send()
        .await
        .map_err(AppError::RequestError)?;
    parse_upstream::<serde_json::Value>(response).await
}

async fn burn(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<BurnRequest>,
) -> HttpResponse {
    handle_result(
        burn_assets(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            req.into_inner(),
        )
        .await,
    )
}

async fn list(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    handle_result(list_burns(client.as_ref(), &base_url.0, &macaroon_hex.0).await)
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/burn").route(web::post().to(burn)))
        .service(web::resource("/burns").route(web::get().to(list)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn specifier(asset_id: Option<&str>, group_key: Option<&str>) -> AssetSpecifier {
        AssetSpecifier {
            asset_id_str: asset_id.map(str::to_string),
            group_key_str: group_key.map(str::to_string),
        }
    }

    #[test]
    fn test_rejects_empty_specifier() {
        assert!(specifier(None, None).validate().is_err());
    }

    #[test]
    fn test_rejects_both_fields_set() {
        assert!(specifier(Some(&"a".repeat(64)), Some(&"a".repeat(66)))
            .validate()
            .is_err());
    }

    #[test]
    fn test_accepts_exactly_one_valid_field() {
        assert!(specifier(Some(&"a".repeat(64)), None).validate().is_ok());
        assert!(specifier(None, Some(&"a".repeat(66))).validate().is_ok());
    }

    #[test]
    fn test_rejects_malformed_asset_id() {
        assert!(specifier(Some("deadbeef"), None).validate().is_err());
        assert!(specifier(Some(&"z".repeat(64)), None).validate().is_err());
    }
}
