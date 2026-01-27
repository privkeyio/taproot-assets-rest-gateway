use crate::error::AppError;
use crate::types::{BaseUrl, MacaroonHex};
use actix_web::{web, HttpResponse};
use reqwest::Client;
use tracing::{info, instrument};

#[instrument(skip(client))]
pub async fn stop_daemon(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<serde_json::Value, AppError> {
    info!("Stopping daemon");
    let url = format!("{base_url}/v1/taproot-assets/stop");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(AppError::RequestError)?;

    // Check the status and properly propagate errors
    if response.status().is_success() {
        Ok(serde_json::json!({}))
    } else {
        // For authentication errors, we should propagate them as actual errors
        let status = response.status();
        let text = response.text().await.map_err(AppError::RequestError)?;

        // Return an error that will be handled by the error handler
        Err(AppError::ValidationError(format!(
            "Stop daemon failed with status {status}: {text}"
        )))
    }
}

async fn stop_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    match stop_daemon(client.as_ref(), &base_url.0, &macaroon_hex.0).await {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(e) => {
            let status = e.status_code();
            HttpResponse::build(status).json(serde_json::json!({
                "error": e.to_string()
            }))
        }
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/stop").route(web::post().to(stop_handler)));
}
