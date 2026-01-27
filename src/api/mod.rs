pub mod addresses;
pub mod assets;
pub mod burn;
pub mod channels;
pub mod events;
pub mod health;
pub mod info;
pub mod mailbox;
pub mod mailbox_auth;
pub mod proofs;
pub mod rfq;
pub mod routes;
pub mod send;
pub mod stop;
pub mod universe;
pub mod wallet;

use crate::error::AppError;
use actix_web::HttpResponse;

pub fn handle_result<T: serde::Serialize>(result: Result<T, AppError>) -> HttpResponse {
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
