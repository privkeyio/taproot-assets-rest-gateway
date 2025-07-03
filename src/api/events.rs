use crate::error::AppError;
use crate::types::{BaseUrl, MacaroonHex};
use actix_web::{web, HttpResponse};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{info, instrument, warn};

#[derive(Debug, Serialize, Deserialize)]
pub struct DebugLevelRequest {
    pub show: bool,
    pub level_spec: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssetMintRequest {
    pub short_response: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssetReceiveRequest {
    pub filter_addr: Option<String>,
    pub start_timestamp: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssetSendRequest {
    pub filter_script_key: Option<String>,
    pub filter_label: Option<String>,
}

// Create a separate client for event subscriptions with longer timeout
fn create_event_client() -> Result<Client, AppError> {
    Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(300)) // 5 minute timeout for event subscriptions
        .build()
        .map_err(|e| AppError::ValidationError(format!("Failed to create event client: {e}")))
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn set_debug_level(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: DebugLevelRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Setting debug level: {}", request.level_spec);
    let url = format!("{base_url}/v1/taproot-assets/debuglevel");
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

#[instrument(skip(macaroon_hex, request))]
pub async fn asset_mint_events(
    base_url: &str,
    macaroon_hex: &str,
    request: AssetMintRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Subscribing to asset mint events");
    let event_client = create_event_client()?;
    let url = format!("{base_url}/v1/taproot-assets/events/asset-mint");

    let response = event_client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await;

    match response {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                resp.json::<serde_json::Value>()
                    .await
                    .map_err(AppError::RequestError)
            } else {
                let error_text = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(AppError::ValidationError(format!(
                    "Event subscription failed with status {status}: {error_text}"
                )))
            }
        }
        Err(e) if e.is_timeout() => {
            warn!("Asset mint event subscription timed out");
            Ok(serde_json::json!({
                "events": [],
                "timeout": true,
                "message": "No events received within timeout period"
            }))
        }
        Err(e) => Err(AppError::RequestError(e)),
    }
}

#[instrument(skip(macaroon_hex, request))]
pub async fn asset_receive_events(
    base_url: &str,
    macaroon_hex: &str,
    request: AssetReceiveRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Subscribing to asset receive events");
    let event_client = create_event_client()?;
    let url = format!("{base_url}/v1/taproot-assets/events/asset-receive");

    let response = event_client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await;

    match response {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                resp.json::<serde_json::Value>()
                    .await
                    .map_err(AppError::RequestError)
            } else {
                let error_text = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(AppError::ValidationError(format!(
                    "Event subscription failed with status {status}: {error_text}"
                )))
            }
        }
        Err(e) if e.is_timeout() => {
            warn!("Asset receive event subscription timed out");
            Ok(serde_json::json!({
                "events": [],
                "timeout": true,
                "message": "No events received within timeout period"
            }))
        }
        Err(e) => Err(AppError::RequestError(e)),
    }
}

#[instrument(skip(macaroon_hex, request))]
pub async fn asset_send_events(
    base_url: &str,
    macaroon_hex: &str,
    request: AssetSendRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Subscribing to asset send events");
    let event_client = create_event_client()?;
    let url = format!("{base_url}/v1/taproot-assets/events/asset-send");

    let response = event_client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await;

    match response {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                resp.json::<serde_json::Value>()
                    .await
                    .map_err(AppError::RequestError)
            } else {
                let error_text = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(AppError::ValidationError(format!(
                    "Event subscription failed with status {status}: {error_text}"
                )))
            }
        }
        Err(e) if e.is_timeout() => {
            warn!("Asset send event subscription timed out");
            Ok(serde_json::json!({
                "events": [],
                "timeout": true,
                "message": "No events received within timeout period"
            }))
        }
        Err(e) => Err(AppError::RequestError(e)),
    }
}

async fn set_debug_level_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<DebugLevelRequest>,
) -> HttpResponse {
    handle_result(
        set_debug_level(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
        )
        .await,
    )
}

async fn asset_mint_handler(
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<AssetMintRequest>,
) -> HttpResponse {
    handle_result(
        asset_mint_events(
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
        )
        .await,
    )
}

async fn asset_receive_handler(
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<AssetReceiveRequest>,
) -> HttpResponse {
    handle_result(
        asset_receive_events(
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
        )
        .await,
    )
}

async fn asset_send_handler(
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<AssetSendRequest>,
) -> HttpResponse {
    handle_result(
        asset_send_events(
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
        )
        .await,
    )
}

fn handle_result<T: serde::Serialize>(result: Result<T, AppError>) -> HttpResponse {
    match result {
        Ok(value) => HttpResponse::Ok().json(value),
        Err(e) => {
            let status = e.status_code();
            HttpResponse::build(status).json(serde_json::json!({
                "error": e.to_string(),
                "type": format!("{:?}", e)
            }))
        }
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/debuglevel").route(web::post().to(set_debug_level_handler)))
        .service(web::resource("/events/asset-mint").route(web::post().to(asset_mint_handler)))
        .service(
            web::resource("/events/asset-receive").route(web::post().to(asset_receive_handler)),
        )
        .service(web::resource("/events/asset-send").route(web::post().to(asset_send_handler)));
}
