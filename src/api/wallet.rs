use crate::error::AppError;
use crate::types::{BaseUrl, MacaroonHex};
use actix_web::{web, HttpResponse};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{info, instrument};

#[derive(Debug, Serialize, Deserialize)]
pub struct InternalKeyRequest {
    pub key_family: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OwnershipProveRequest {
    pub asset_id: String,
    pub script_key: String,
    pub outpoint: serde_json::Value,
    pub challenge: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OwnershipVerifyRequest {
    pub proof_with_witness: String,
    pub challenge: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScriptKeyRequest {
    pub script_key: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UtxoLeaseDeleteRequest {
    pub outpoint: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VirtualPsbtAnchorRequest {
    pub virtual_psbts: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VirtualPsbtCommitRequest {
    pub virtual_psbts: Vec<String>,
    pub passive_asset_psbts: Vec<String>,
    pub anchor_psbt: String,
    pub existing_output_index: i32,
    pub add: bool,
    pub target_conf: u32,
    pub sat_per_vbyte: String,
    pub custom_lock_id: Option<String>,
    pub lock_expiration_seconds: Option<String>,
    pub skip_funding: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VirtualPsbtFundRequest {
    pub psbt: String,
    pub raw: serde_json::Value,
    pub coin_select_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VirtualPsbtLogTransferRequest {
    pub anchor_psbt: String,
    pub virtual_psbts: Vec<String>,
    pub passive_asset_psbts: Vec<String>,
    pub change_output_index: i32,
    pub lnd_locked_utxos: Vec<serde_json::Value>,
    pub skip_anchor_tx_broadcast: bool,
    pub label: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VirtualPsbtSignRequest {
    pub funded_psbt: String,
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn next_internal_key(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: InternalKeyRequest,
) -> Result<Value, AppError> {
    info!(
        "Fetching next internal key for family: {}",
        request.key_family
    );
    let url = format!("{base_url}/v1/taproot-assets/wallet/internal-key/next");
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
pub async fn get_internal_key(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    internal_key: &str,
) -> Result<Value, AppError> {
    info!("Fetching internal key: {}", internal_key);
    let url = format!("{base_url}/v1/taproot-assets/wallet/internal-key/{internal_key}");
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
pub async fn prove_ownership(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: OwnershipProveRequest,
) -> Result<Value, AppError> {
    info!("Proving ownership for asset ID: {}", request.asset_id);
    let url = format!("{base_url}/v1/taproot-assets/wallet/ownership/prove");
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
pub async fn verify_ownership(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: OwnershipVerifyRequest,
) -> Result<Value, AppError> {
    info!("Verifying ownership");
    let url = format!("{base_url}/v1/taproot-assets/wallet/ownership/verify");
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
pub async fn declare_script_key(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: ScriptKeyRequest,
) -> Result<Value, AppError> {
    info!("Declaring script key");
    let url = format!("{base_url}/v1/taproot-assets/wallet/script-key/declare");
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
pub async fn next_script_key(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: InternalKeyRequest,
) -> Result<Value, AppError> {
    info!(
        "Fetching next script key for family: {}",
        request.key_family
    );
    let url = format!("{base_url}/v1/taproot-assets/wallet/script-key/next");
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
pub async fn get_script_key(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    tweaked_script_key: &str,
) -> Result<Value, AppError> {
    info!("Fetching script key: {}", tweaked_script_key);
    let url = format!("{base_url}/v1/taproot-assets/wallet/script-key/{tweaked_script_key}");
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
pub async fn delete_utxo_lease(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: UtxoLeaseDeleteRequest,
) -> Result<Value, AppError> {
    info!("Deleting UTXO lease");
    let url = format!("{base_url}/v1/taproot-assets/wallet/utxo-lease/delete");
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
pub async fn anchor_virtual_psbt(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: VirtualPsbtAnchorRequest,
) -> Result<Value, AppError> {
    info!("Anchoring virtual PSBT");
    let url = format!("{base_url}/v1/taproot-assets/wallet/virtual-psbt/anchor");
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
pub async fn commit_virtual_psbt(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: VirtualPsbtCommitRequest,
) -> Result<Value, AppError> {
    info!("Committing virtual PSBT");
    let url = format!("{base_url}/v1/taproot-assets/wallet/virtual-psbt/commit");
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
pub async fn fund_virtual_psbt(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: VirtualPsbtFundRequest,
) -> Result<Value, AppError> {
    info!("Funding virtual PSBT");
    let url = format!("{base_url}/v1/taproot-assets/wallet/virtual-psbt/fund");
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
pub async fn log_virtual_psbt_transfer(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: VirtualPsbtLogTransferRequest,
) -> Result<Value, AppError> {
    info!("Logging virtual PSBT transfer");
    let url = format!("{base_url}/v1/taproot-assets/wallet/virtual-psbt/log-transfer");
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
pub async fn sign_virtual_psbt(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: VirtualPsbtSignRequest,
) -> Result<Value, AppError> {
    info!("Signing virtual PSBT");
    let url = format!("{base_url}/v1/taproot-assets/wallet/virtual-psbt/sign");
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

async fn next_internal_key_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<InternalKeyRequest>,
) -> HttpResponse {
    handle_result(
        next_internal_key(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            req.into_inner(),
        )
        .await,
    )
}

async fn get_internal_key_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    path: web::Path<String>,
) -> HttpResponse {
    let internal_key = path.into_inner();
    handle_result(
        get_internal_key(client.as_ref(), &base_url.0, &macaroon_hex.0, &internal_key).await,
    )
}

async fn prove_ownership_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<OwnershipProveRequest>,
) -> HttpResponse {
    handle_result(
        prove_ownership(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            req.into_inner(),
        )
        .await,
    )
}

async fn verify_ownership_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<OwnershipVerifyRequest>,
) -> HttpResponse {
    handle_result(
        verify_ownership(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            req.into_inner(),
        )
        .await,
    )
}

async fn declare_script_key_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<ScriptKeyRequest>,
) -> HttpResponse {
    handle_result(
        declare_script_key(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            req.into_inner(),
        )
        .await,
    )
}

async fn next_script_key_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<InternalKeyRequest>,
) -> HttpResponse {
    handle_result(
        next_script_key(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            req.into_inner(),
        )
        .await,
    )
}

async fn get_script_key_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    path: web::Path<String>,
) -> HttpResponse {
    let tweaked_script_key = path.into_inner();
    handle_result(
        get_script_key(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            &tweaked_script_key,
        )
        .await,
    )
}

async fn delete_utxo_lease_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<UtxoLeaseDeleteRequest>,
) -> HttpResponse {
    handle_result(
        delete_utxo_lease(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            req.into_inner(),
        )
        .await,
    )
}

async fn anchor_virtual_psbt_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<VirtualPsbtAnchorRequest>,
) -> HttpResponse {
    handle_result(
        anchor_virtual_psbt(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            req.into_inner(),
        )
        .await,
    )
}

async fn commit_virtual_psbt_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<VirtualPsbtCommitRequest>,
) -> HttpResponse {
    handle_result(
        commit_virtual_psbt(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            req.into_inner(),
        )
        .await,
    )
}

async fn fund_virtual_psbt_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<VirtualPsbtFundRequest>,
) -> HttpResponse {
    handle_result(
        fund_virtual_psbt(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            req.into_inner(),
        )
        .await,
    )
}

async fn log_virtual_psbt_transfer_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<VirtualPsbtLogTransferRequest>,
) -> HttpResponse {
    handle_result(
        log_virtual_psbt_transfer(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            req.into_inner(),
        )
        .await,
    )
}

async fn sign_virtual_psbt_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<VirtualPsbtSignRequest>,
) -> HttpResponse {
    handle_result(
        sign_virtual_psbt(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
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
        web::resource("/wallet/internal-key/next").route(web::post().to(next_internal_key_handler)),
    )
    .service(
        web::resource("/wallet/internal-key/{internal_key}")
            .route(web::get().to(get_internal_key_handler)),
    )
    .service(
        web::resource("/wallet/ownership/prove").route(web::post().to(prove_ownership_handler)),
    )
    .service(
        web::resource("/wallet/ownership/verify").route(web::post().to(verify_ownership_handler)),
    )
    .service(
        web::resource("/wallet/script-key/declare")
            .route(web::post().to(declare_script_key_handler)),
    )
    .service(
        web::resource("/wallet/script-key/next").route(web::post().to(next_script_key_handler)),
    )
    .service(
        web::resource("/wallet/script-key/{tweaked_script_key}")
            .route(web::get().to(get_script_key_handler)),
    )
    .service(
        web::resource("/wallet/utxo-lease/delete").route(web::post().to(delete_utxo_lease_handler)),
    )
    .service(
        web::resource("/wallet/virtual-psbt/anchor")
            .route(web::post().to(anchor_virtual_psbt_handler)),
    )
    .service(
        web::resource("/wallet/virtual-psbt/commit")
            .route(web::post().to(commit_virtual_psbt_handler)),
    )
    .service(
        web::resource("/wallet/virtual-psbt/fund").route(web::post().to(fund_virtual_psbt_handler)),
    )
    .service(
        web::resource("/wallet/virtual-psbt/log-transfer")
            .route(web::post().to(log_virtual_psbt_transfer_handler)),
    )
    .service(
        web::resource("/wallet/virtual-psbt/sign").route(web::post().to(sign_virtual_psbt_handler)),
    );
}
