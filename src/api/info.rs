use crate::error::AppError;
use crate::types::{BaseUrl, MacaroonHex};
use actix_web::{web, HttpResponse};
use reqwest::Client;
use serde_json::Value;
use tracing::{info, instrument};

#[instrument(skip(client))]
pub async fn get_info(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<Value, AppError> {
    info!("Fetching getinfo");
    let url = format!("{base_url}/v1/taproot-assets/getinfo");
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

async fn get_info_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    handle_result(get_info(client.as_ref(), &base_url.0, &macaroon_hex.0).await)
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
    cfg.service(web::resource("/getinfo").route(web::get().to(get_info_handler)));
}
