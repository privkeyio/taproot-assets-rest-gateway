use crate::api::info;
use crate::types::{BaseUrl, MacaroonHex};
use actix_web::{web, HttpResponse};
use reqwest::Client;

pub async fn health() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

pub async fn readiness(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    match info::get_info(client.as_ref(), &base_url.0, &macaroon_hex.0).await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({
            "status": "ready",
            "services": {"taproot_assets": "up"}
        })),
        Err(_) => HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "not_ready",
            "services": {"taproot_assets": "down"}
        })),
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/health").route(web::get().to(health)))
        .service(web::resource("/readiness").route(web::get().to(readiness)));
}
