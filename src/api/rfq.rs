use crate::error::AppError;
use crate::types::{BaseUrl, MacaroonHex};
use actix_web::{web, HttpResponse};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{info, instrument};

#[derive(Debug, Serialize, Deserialize)]
pub struct BuyOfferRequest {
    pub asset_specifier: serde_json::Value,
    pub max_units: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuyOrderRequest {
    pub asset_specifier: serde_json::Value,
    pub asset_max_amt: String,
    pub expiry: String,
    pub peer_pub_key: String,
    pub timeout_seconds: u32,
    pub skip_asset_channel_check: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SellOfferRequest {
    pub asset_specifier: serde_json::Value,
    pub max_units: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SellOrderRequest {
    pub asset_specifier: serde_json::Value,
    pub payment_max_amt: String,
    pub expiry: String,
    pub peer_pub_key: String,
    pub timeout_seconds: u32,
    pub skip_asset_channel_check: bool,
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn buy_offer(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: BuyOfferRequest,
    asset_id: &str,
) -> Result<Value, AppError> {
    info!("Creating buy offer for asset ID: {}", asset_id);
    let url = format!("{base_url}/v1/taproot-assets/rfq/buyoffer/asset-id/{asset_id}");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response
        .json::<Value>()
        .await
        .map_err(AppError::RequestError)
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn buy_order(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: BuyOrderRequest,
    asset_id: &str,
) -> Result<Value, AppError> {
    info!("Creating buy order for asset ID: {}", asset_id);
    let url = format!("{base_url}/v1/taproot-assets/rfq/buyorder/asset-id/{asset_id}");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response
        .json::<Value>()
        .await
        .map_err(AppError::RequestError)
}

#[instrument(skip(client, macaroon_hex))]
pub async fn get_notifications(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<Value, AppError> {
    info!("Fetching RFQ notifications");
    let url = format!("{base_url}/v1/taproot-assets/rfq/ntfs");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response
        .json::<Value>()
        .await
        .map_err(AppError::RequestError)
}

#[instrument(skip(client, macaroon_hex))]
pub async fn get_asset_rates(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<Value, AppError> {
    info!("Fetching asset rates");
    let url = format!("{base_url}/v1/taproot-assets/rfq/priceoracle/assetrates");
    let response = client
        .get(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response
        .json::<Value>()
        .await
        .map_err(AppError::RequestError)
}

#[instrument(skip(client, macaroon_hex))]
pub async fn get_peer_quotes(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<Value, AppError> {
    info!("Fetching peer-accepted quotes");
    let url = format!("{base_url}/v1/taproot-assets/rfq/quotes/peeraccepted");
    let response = client
        .get(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response
        .json::<Value>()
        .await
        .map_err(AppError::RequestError)
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn sell_offer(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: SellOfferRequest,
    asset_id: &str,
) -> Result<Value, AppError> {
    info!("Creating sell offer for asset ID: {}", asset_id);
    let url = format!("{base_url}/v1/taproot-assets/rfq/selloffer/asset-id/{asset_id}");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response
        .json::<Value>()
        .await
        .map_err(AppError::RequestError)
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn sell_order(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: SellOrderRequest,
    asset_id: &str,
) -> Result<Value, AppError> {
    info!("Creating sell order for asset ID: {}", asset_id);
    let url = format!("{base_url}/v1/taproot-assets/rfq/sellorder/asset-id/{asset_id}");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response
        .json::<Value>()
        .await
        .map_err(AppError::RequestError)
}

async fn buy_offer_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    path: web::Path<String>,
    req: web::Json<BuyOfferRequest>,
) -> HttpResponse {
    let asset_id = path.into_inner();
    handle_result(
        buy_offer(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
            asset_id.as_str(),
        )
        .await,
    )
}

async fn buy_order_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    path: web::Path<String>,
    req: web::Json<BuyOrderRequest>,
) -> HttpResponse {
    let asset_id = path.into_inner();
    handle_result(
        buy_order(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
            asset_id.as_str(),
        )
        .await,
    )
}

async fn notifications_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    handle_result(
        get_notifications(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
        )
        .await,
    )
}

async fn asset_rates_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    handle_result(
        get_asset_rates(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
        )
        .await,
    )
}

async fn peer_quotes_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    handle_result(
        get_peer_quotes(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
        )
        .await,
    )
}

async fn sell_offer_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    path: web::Path<String>,
    req: web::Json<SellOfferRequest>,
) -> HttpResponse {
    let asset_id = path.into_inner();
    handle_result(
        sell_offer(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
            asset_id.as_str(),
        )
        .await,
    )
}

async fn sell_order_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    path: web::Path<String>,
    req: web::Json<SellOrderRequest>,
) -> HttpResponse {
    let asset_id = path.into_inner();
    handle_result(
        sell_order(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
            asset_id.as_str(),
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
    cfg.service(
        web::resource("/rfq/buyoffer/asset-id/{asset_id}").route(web::post().to(buy_offer_handler)),
    )
    .service(
        web::resource("/rfq/buyorder/asset-id/{asset_id}").route(web::post().to(buy_order_handler)),
    )
    .service(web::resource("/rfq/ntfs").route(web::post().to(notifications_handler)))
    .service(web::resource("/rfq/priceoracle/assetrates").route(web::get().to(asset_rates_handler)))
    .service(web::resource("/rfq/quotes/peeraccepted").route(web::get().to(peer_quotes_handler)))
    .service(
        web::resource("/rfq/selloffer/asset-id/{asset_id}")
            .route(web::post().to(sell_offer_handler)),
    )
    .service(
        web::resource("/rfq/sellorder/asset-id/{asset_id}")
            .route(web::post().to(sell_order_handler)),
    );
}
