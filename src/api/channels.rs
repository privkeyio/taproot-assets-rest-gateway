use crate::error::AppError;
use crate::types::{BaseUrl, MacaroonHex};
use actix_web::{web, HttpResponse};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

#[derive(Debug, Serialize, Deserialize)]
pub struct EncodeCustomDataRequest {
    pub router_send_payment: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FundChannelRequest {
    pub asset_amount: String,
    pub asset_id: String,
    pub peer_pubkey: String,
    pub fee_rate_sat_per_vbyte: u32,
    pub push_sat: Option<String>,
    pub group_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InvoiceRequest {
    pub asset_id: String,
    pub asset_amount: String,
    pub peer_pubkey: String,
    pub invoice_request: Option<serde_json::Value>,
    pub hodl_invoice: Option<serde_json::Value>,
    pub group_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DecodeInvoiceRequest {
    pub asset_id: String,
    pub pay_req_string: String,
    pub group_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendPaymentRequest {
    pub asset_id: String,
    pub asset_amount: String,
    pub peer_pubkey: String,
    pub payment_request: Option<serde_json::Value>,
    pub rfq_id: Option<String>,
    pub allow_overpay: bool,
    pub group_key: Option<String>,
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn encode_custom_data(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: EncodeCustomDataRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Encoding custom data");
    let url = format!("{base_url}/v1/taproot-assets/channels/encode-custom-data");
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

#[instrument(skip(client, macaroon_hex, request))]
pub async fn fund_channel(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: FundChannelRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Funding channel for asset ID: {}", request.asset_id);
    let url = format!("{base_url}/v1/taproot-assets/channels/fund");
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

#[instrument(skip(client, macaroon_hex, request))]
pub async fn create_invoice(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: InvoiceRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Creating invoice for asset ID: {}", request.asset_id);
    let url = format!("{base_url}/v1/taproot-assets/channels/invoice");
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

#[instrument(skip(client, macaroon_hex, request))]
pub async fn decode_invoice(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: DecodeInvoiceRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Decoding invoice for asset ID: {}", request.asset_id);
    let url = format!("{base_url}/v1/taproot-assets/channels/invoice/decode");
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

#[instrument(skip(client, macaroon_hex, request))]
pub async fn send_payment(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: SendPaymentRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Sending payment for asset ID: {}", request.asset_id);
    let url = format!("{base_url}/v1/taproot-assets/channels/send-payment");
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

async fn encode_custom_data_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<EncodeCustomDataRequest>,
) -> HttpResponse {
    handle_result(
        encode_custom_data(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
        )
        .await,
    )
}

async fn fund_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<FundChannelRequest>,
) -> HttpResponse {
    handle_result(
        fund_channel(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
        )
        .await,
    )
}

async fn create_invoice_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<InvoiceRequest>,
) -> HttpResponse {
    handle_result(
        create_invoice(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
        )
        .await,
    )
}

async fn decode_invoice_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<DecodeInvoiceRequest>,
) -> HttpResponse {
    handle_result(
        decode_invoice(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
        )
        .await,
    )
}

async fn send_payment_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<SendPaymentRequest>,
) -> HttpResponse {
    handle_result(
        send_payment(
            client.as_ref(),
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
    cfg.service(
        web::resource("/channels/encode-custom-data")
            .route(web::post().to(encode_custom_data_handler)),
    )
    .service(web::resource("/channels/fund").route(web::post().to(fund_handler)))
    .service(web::resource("/channels/invoice").route(web::post().to(create_invoice_handler)))
    .service(
        web::resource("/channels/invoice/decode").route(web::post().to(decode_invoice_handler)),
    )
    .service(web::resource("/channels/send-payment").route(web::post().to(send_payment_handler)));
}
