use super::handle_result;
use crate::error::AppError;
use crate::types::{BaseUrl, MacaroonHex};
use actix_web::{web, HttpResponse};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

#[derive(Debug, Serialize, Deserialize)]
pub struct SendRequest {
    pub tap_addrs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_rate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_proof_courier_ping_check: Option<bool>,
}

#[instrument(skip(client))]
pub async fn send_assets(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    req: SendRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Sending assets");
    let url = format!("{base_url}/v1/taproot-assets/send");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&req)
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response.json().await.map_err(AppError::RequestError)
}

async fn send_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<SendRequest>,
) -> HttpResponse {
    handle_result(
        send_assets(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            req.into_inner(),
        )
        .await,
    )
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/send").route(web::post().to(send_handler)));
}
