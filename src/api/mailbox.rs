use crate::error::AppError;
use crate::types::{BaseUrl, MacaroonHex};
use actix_web::{web, HttpResponse};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

#[derive(Debug, Serialize, Deserialize)]
pub struct ReceiveRequest {
    pub init: serde_json::Value,
    pub auth_sig: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendRequest {
    pub receiver_id: String,
    pub encrypted_payload: String,
    pub tx_proof: Option<serde_json::Value>,
    pub expiry_block_height: Option<u32>,
}

#[instrument(skip(client, macaroon_hex))]
pub async fn get_mailbox_info(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<serde_json::Value, AppError> {
    info!("Fetching mailbox info");
    let url = format!("{base_url}/v1/taproot-assets/mailbox/info");
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

#[instrument(skip(client, macaroon_hex, request))]
pub async fn receive_mail(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: ReceiveRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Receiving mail");
    let url = format!("{base_url}/v1/taproot-assets/mailbox/receive");
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
pub async fn send_mail(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: SendRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Sending mail to receiver ID: {}", request.receiver_id);
    let url = format!("{base_url}/v1/taproot-assets/mailbox/send");
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

async fn info(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    handle_result(get_mailbox_info(&client, &base_url.0, &macaroon_hex.0).await)
}

async fn receive(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<ReceiveRequest>,
) -> HttpResponse {
    handle_result(receive_mail(&client, &base_url.0, &macaroon_hex.0, req.into_inner()).await)
}

async fn send(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<SendRequest>,
) -> HttpResponse {
    handle_result(send_mail(&client, &base_url.0, &macaroon_hex.0, req.into_inner()).await)
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
    cfg.service(web::resource("/mailbox/info").route(web::get().to(info)))
        .service(web::resource("/mailbox/receive").route(web::post().to(receive)))
        .service(web::resource("/mailbox/send").route(web::post().to(send)));
}
