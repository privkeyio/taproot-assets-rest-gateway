use super::handle_result;
use crate::error::AppError;
use crate::types::{BaseUrl, MacaroonHex};
use actix_web::{web, HttpResponse};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

#[derive(Debug, Serialize, Deserialize)]
pub struct BurnRequest {
    pub asset_id: String,
    pub asset_id_str: Option<String>,
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
    info!("Burning assets for asset ID: {}", request.asset_id);
    let url = format!("{base_url}/v1/taproot-assets/burn");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response
        .json::<serde_json::Value>()
        .await
        .map_err(AppError::RequestError)
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
    response
        .json::<serde_json::Value>()
        .await
        .map_err(AppError::RequestError)
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
