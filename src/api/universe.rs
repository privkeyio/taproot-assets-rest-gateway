use crate::error::AppError;
use crate::types::{BaseUrl, MacaroonHex};
use actix_web::{web, HttpResponse};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{info, instrument};

#[derive(Debug, Serialize, Deserialize)]
pub struct FederationRequest {
    pub servers: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MultiverseRequest {
    pub proof_type: String,
    pub specific_ids: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PushProofRequest {
    pub key: serde_json::Value,
    pub server: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncRequest {
    pub universe_host: String,
    pub sync_mode: String,
    pub sync_targets: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncConfigRequest {
    pub global_sync_configs: Vec<serde_json::Value>,
    pub asset_sync_configs: Vec<serde_json::Value>,
}

#[instrument(skip(client, macaroon_hex))]
pub async fn delete_universe(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<Value, AppError> {
    info!("Deleting universe");
    let url = format!("{base_url}/v1/taproot-assets/universe/delete");
    let response = client
        .delete(&url)
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
pub async fn delete_federation(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<Value, AppError> {
    info!("Deleting federation");
    let url = format!("{base_url}/v1/taproot-assets/universe/federation");
    let response = client
        .delete(&url)
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
pub async fn add_federation(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: FederationRequest,
) -> Result<Value, AppError> {
    info!("Adding federation");
    let url = format!("{base_url}/v1/taproot-assets/universe/federation");
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
pub async fn get_federation(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<Value, AppError> {
    info!("Fetching federation info");
    let url = format!("{base_url}/v1/taproot-assets/universe/federation");
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
pub async fn get_universe_info(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<Value, AppError> {
    info!("Fetching universe info");
    let url = format!("{base_url}/v1/taproot-assets/universe/info");
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
pub async fn get_keys(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    asset_id: &str,
) -> Result<Value, AppError> {
    info!("Fetching keys for asset ID: {}", asset_id);
    let url = format!("{base_url}/v1/taproot-assets/universe/keys/asset-id/{asset_id}");
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
pub async fn get_leaves(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    asset_id: &str,
) -> Result<Value, AppError> {
    info!("Fetching leaves for asset ID: {}", asset_id);
    let url = format!("{base_url}/v1/taproot-assets/universe/leaves/asset-id/{asset_id}");
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
pub async fn get_multiverse(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: MultiverseRequest,
) -> Result<Value, AppError> {
    info!("Fetching multiverse data");
    let url = format!("{base_url}/v1/taproot-assets/universe/multiverse");
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
pub async fn get_proofs(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    asset_id: &str,
    hash_str: &str,
    index: &str,
    script_key: &str,
) -> Result<Value, AppError> {
    info!("Fetching proofs for asset ID: {}", asset_id);
    let url = format!(
        "{base_url}/v1/taproot-assets/universe/proofs/asset-id/{asset_id}/{hash_str}/{index}/{script_key}"
    );
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
#[allow(clippy::too_many_arguments)]
pub async fn push_proof(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: PushProofRequest,
    asset_id: &str,
    hash_str: &str,
    index: &str,
    script_key: &str,
) -> Result<Value, AppError> {
    info!("Pushing proof for asset ID: {}", asset_id);
    let url = format!(
        "{base_url}/v1/taproot-assets/universe/proofs/push/asset-id/{asset_id}/{hash_str}/{index}/{script_key}"
    );
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
pub async fn get_roots(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<Value, AppError> {
    info!("Fetching universe roots");
    let url = format!("{base_url}/v1/taproot-assets/universe/roots");
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
pub async fn get_asset_roots(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    asset_id: &str,
) -> Result<Value, AppError> {
    info!("Fetching asset roots for asset ID: {}", asset_id);
    let url = format!("{base_url}/v1/taproot-assets/universe/roots/asset-id/{asset_id}");
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
pub async fn get_stats(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<Value, AppError> {
    info!("Fetching universe stats");
    let url = format!("{base_url}/v1/taproot-assets/universe/stats");
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
pub async fn get_asset_stats(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<Value, AppError> {
    info!("Fetching asset stats");
    let url = format!("{base_url}/v1/taproot-assets/universe/stats/assets");
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
pub async fn get_event_stats(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<Value, AppError> {
    info!("Fetching event stats");
    let url = format!("{base_url}/v1/taproot-assets/universe/stats/events");
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
pub async fn sync_universe(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: SyncRequest,
) -> Result<Value, AppError> {
    info!("Syncing universe with host: {}", request.universe_host);
    let url = format!("{base_url}/v1/taproot-assets/universe/sync");
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
pub async fn set_sync_config(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: SyncConfigRequest,
) -> Result<Value, AppError> {
    info!("Setting sync configuration");
    let url = format!("{base_url}/v1/taproot-assets/universe/sync/config");
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
pub async fn get_sync_config(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<Value, AppError> {
    info!("Fetching sync configuration");
    let url = format!("{base_url}/v1/taproot-assets/universe/sync/config");
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

async fn delete_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    handle_result(delete_universe(client.as_ref(), &base_url.0, &macaroon_hex.0).await)
}

async fn delete_federation_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    handle_result(delete_federation(client.as_ref(), &base_url.0, &macaroon_hex.0).await)
}

async fn add_federation_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<FederationRequest>,
) -> HttpResponse {
    handle_result(
        add_federation(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            req.into_inner(),
        )
        .await,
    )
}

async fn get_federation_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    handle_result(get_federation(client.as_ref(), &base_url.0, &macaroon_hex.0).await)
}

async fn info_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    handle_result(get_universe_info(client.as_ref(), &base_url.0, &macaroon_hex.0).await)
}

async fn keys_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    path: web::Path<String>,
) -> HttpResponse {
    let asset_id = path.into_inner();
    handle_result(get_keys(client.as_ref(), &base_url.0, &macaroon_hex.0, &asset_id).await)
}

async fn leaves_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    path: web::Path<String>,
) -> HttpResponse {
    let asset_id = path.into_inner();
    handle_result(get_leaves(client.as_ref(), &base_url.0, &macaroon_hex.0, &asset_id).await)
}

async fn multiverse_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<MultiverseRequest>,
) -> HttpResponse {
    handle_result(
        get_multiverse(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            req.into_inner(),
        )
        .await,
    )
}

async fn proofs_handler(
    path: web::Path<(String, String, String, String)>,
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    let (asset_id, hash_str, index, script_key) = path.into_inner();
    handle_result(
        get_proofs(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            &asset_id,
            &hash_str,
            &index,
            &script_key,
        )
        .await,
    )
}

async fn push_proof_handler(
    path: web::Path<(String, String, String, String)>,
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<PushProofRequest>,
) -> HttpResponse {
    let (asset_id, hash_str, index, script_key) = path.into_inner();
    handle_result(
        push_proof(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            req.into_inner(),
            &asset_id,
            &hash_str,
            &index,
            &script_key,
        )
        .await,
    )
}

async fn roots_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    handle_result(get_roots(client.as_ref(), &base_url.0, &macaroon_hex.0).await)
}

async fn asset_roots_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    path: web::Path<String>,
) -> HttpResponse {
    let asset_id = path.into_inner();
    handle_result(get_asset_roots(client.as_ref(), &base_url.0, &macaroon_hex.0, &asset_id).await)
}

async fn stats_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    handle_result(get_stats(client.as_ref(), &base_url.0, &macaroon_hex.0).await)
}

async fn asset_stats_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    handle_result(get_asset_stats(client.as_ref(), &base_url.0, &macaroon_hex.0).await)
}

async fn event_stats_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    handle_result(get_event_stats(client.as_ref(), &base_url.0, &macaroon_hex.0).await)
}

async fn sync_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<SyncRequest>,
) -> HttpResponse {
    handle_result(
        sync_universe(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            req.into_inner(),
        )
        .await,
    )
}

async fn set_sync_config_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<SyncConfigRequest>,
) -> HttpResponse {
    handle_result(
        set_sync_config(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            req.into_inner(),
        )
        .await,
    )
}

async fn get_sync_config_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    handle_result(get_sync_config(client.as_ref(), &base_url.0, &macaroon_hex.0).await)
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
    cfg.service(web::resource("/universe/delete").route(web::delete().to(delete_handler)))
        .service(
            web::resource("/universe/federation")
                .route(web::delete().to(delete_federation_handler))
                .route(web::post().to(add_federation_handler))
                .route(web::get().to(get_federation_handler)),
        )
        .service(web::resource("/universe/info").route(web::get().to(info_handler)))
        .service(
            web::resource("/universe/keys/asset-id/{asset_id}").route(web::get().to(keys_handler)),
        )
        .service(
            web::resource("/universe/leaves/asset-id/{asset_id}")
                .route(web::get().to(leaves_handler)),
        )
        .service(web::resource("/universe/multiverse").route(web::post().to(multiverse_handler)))
        .service(
            web::resource("/universe/proofs/asset-id/{asset_id}/{hash_str}/{index}/{script_key}")
                .route(web::get().to(proofs_handler)),
        )
        .service(
            web::resource(
                "/universe/proofs/push/asset-id/{asset_id}/{hash_str}/{index}/{script_key}",
            )
            .route(web::post().to(push_proof_handler)),
        )
        .service(web::resource("/universe/roots").route(web::get().to(roots_handler)))
        .service(
            web::resource("/universe/roots/asset-id/{asset_id}")
                .route(web::get().to(asset_roots_handler)),
        )
        .service(web::resource("/universe/stats").route(web::get().to(stats_handler)))
        .service(web::resource("/universe/stats/assets").route(web::get().to(asset_stats_handler)))
        .service(web::resource("/universe/stats/events").route(web::get().to(event_stats_handler)))
        .service(web::resource("/universe/sync").route(web::post().to(sync_handler)))
        .service(
            web::resource("/universe/sync/config")
                .route(web::post().to(set_sync_config_handler))
                .route(web::get().to(get_sync_config_handler)),
        );
}
