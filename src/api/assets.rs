use crate::error::AppError;
use crate::types::{BaseUrl, MacaroonHex};
use actix_web::{web, HttpResponse};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Asset {
    #[serde(default)]
    pub asset_id: Option<String>,
    #[serde(default)]
    pub asset_type: Option<String>,
    #[serde(default)]
    pub amount: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum AssetResponse {
    Wrapped {
        assets: Vec<Asset>,
        #[serde(default)]
        #[allow(dead_code)] // Used for deserialization
        unconfirmed_transfers: Option<String>,
        #[serde(default)]
        #[allow(dead_code)] // Used for deserialization
        unconfirmed_mints: Option<String>,
    },
    Direct(Vec<Asset>),
}

impl AssetResponse {
    fn into_assets(self) -> Vec<Asset> {
        match self {
            AssetResponse::Wrapped { assets, .. } => assets,
            AssetResponse::Direct(assets) => assets,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MintAssetRequest {
    pub asset: MintAsset,
    pub short_response: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MintAsset {
    pub asset_type: String,
    pub name: String,
    pub amount: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MintFundRequest {
    pub short_response: bool,
    pub fee_rate: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_tree: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MintFinalizeRequest {
    pub short_response: bool,
    pub fee_rate: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_tree: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MintSealRequest {
    pub short_response: bool,
    pub group_witnesses: Vec<String>,
    pub signed_group_virtual_psbts: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransferRegisterRequest {
    pub asset_id: String,
    pub group_key: Option<String>,
    pub script_key: String,
    pub outpoint: serde_json::Value,
}

#[instrument(skip(client, macaroon_hex))]
pub async fn list_assets(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<Vec<Asset>, AppError> {
    info!("Listing assets");
    let url = format!("{base_url}/v1/taproot-assets/assets");
    let response = client
        .get(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .send()
        .await
        .map_err(AppError::RequestError)?;

    let asset_response: AssetResponse = response.json().await.map_err(AppError::RequestError)?;

    Ok(asset_response.into_assets())
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn mint_asset(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: MintAssetRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Minting asset: {}", request.asset.name);
    let url = format!("{base_url}/v1/taproot-assets/assets");
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

#[instrument(skip(client, macaroon_hex))]
pub async fn get_balance(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<serde_json::Value, AppError> {
    info!("Fetching asset balance");
    let url = format!("{base_url}/v1/taproot-assets/assets/balance");
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

#[instrument(skip(client, macaroon_hex))]
pub async fn get_groups(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<serde_json::Value, AppError> {
    info!("Fetching asset groups");
    let url = format!("{base_url}/v1/taproot-assets/assets/groups");
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

#[instrument(skip(client, macaroon_hex))]
pub async fn get_meta(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    asset_id: &str,
) -> Result<serde_json::Value, AppError> {
    info!("Fetching meta for asset ID: {}", asset_id);
    let url = format!("{base_url}/v1/taproot-assets/assets/meta/asset-id/{asset_id}");
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

#[instrument(skip(client, macaroon_hex))]
pub async fn get_mint_batches(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    batch_key: &str,
) -> Result<serde_json::Value, AppError> {
    info!("Fetching mint batches for batch key: {}", batch_key);
    let url = format!("{base_url}/v1/taproot-assets/assets/mint/batches/{batch_key}");
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

#[instrument(skip(client, macaroon_hex))]
pub async fn list_all_mint_batches(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<serde_json::Value, AppError> {
    info!("Fetching all mint batches");
    let url = format!("{base_url}/v1/taproot-assets/assets/mint/batches");
    let response = client
        .get(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .send()
        .await
        .map_err(AppError::RequestError)?;

    // Handle empty response or return empty batches array
    if response.status() == 404 {
        return Ok(serde_json::json!({ "batches": [] }));
    }

    response
        .json::<serde_json::Value>()
        .await
        .map_err(AppError::RequestError)
}

#[instrument(skip(client, macaroon_hex))]
pub async fn cancel_mint(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<serde_json::Value, AppError> {
    info!("Canceling mint");
    let url = format!("{base_url}/v1/taproot-assets/assets/mint/cancel");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response
        .json::<serde_json::Value>()
        .await
        .map_err(AppError::RequestError)
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn fund_mint(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: MintFundRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Funding mint with fee rate: {}", request.fee_rate);
    let url = format!("{base_url}/v1/taproot-assets/assets/mint/fund");
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
pub async fn finalize_mint(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: MintFinalizeRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Finalizing mint with fee rate: {}", request.fee_rate);
    let url = format!("{base_url}/v1/taproot-assets/assets/mint/finalize");
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
pub async fn seal_mint(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: MintSealRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Sealing mint");
    let url = format!("{base_url}/v1/taproot-assets/assets/mint/seal");
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

#[instrument(skip(client, macaroon_hex))]
pub async fn get_transfers(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<serde_json::Value, AppError> {
    info!("Fetching asset transfers");
    let url = format!("{base_url}/v1/taproot-assets/assets/transfers");
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
pub async fn register_transfer(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: TransferRegisterRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Registering transfer for asset ID: {}", request.asset_id);
    let url = format!("{base_url}/v1/taproot-assets/assets/transfers/register");
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

#[instrument(skip(client, macaroon_hex))]
pub async fn get_utxos(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<serde_json::Value, AppError> {
    info!("Fetching asset UTXOs");
    let url = format!("{base_url}/v1/taproot-assets/assets/utxos");
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

async fn list_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    match list_assets(
        client.as_ref(),
        base_url.0.as_str(),
        macaroon_hex.0.as_str(),
    )
    .await
    {
        Ok(assets) => {
            // The API expects a response with assets, unconfirmed_transfers, and unconfirmed_mints
            let response = serde_json::json!({
                "assets": assets,
                "unconfirmed_transfers": "0",
                "unconfirmed_mints": "0"
            });
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let status = e.status_code();
            HttpResponse::build(status)
                .json(serde_json::json!({"error": e.to_string(), "type": format!("{:?}", e)}))
        }
    }
}

async fn mint_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<MintAssetRequest>,
) -> HttpResponse {
    handle_result(
        mint_asset(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
        )
        .await,
    )
}

async fn balance_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    handle_result(
        get_balance(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
        )
        .await,
    )
}

async fn groups_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    handle_result(
        get_groups(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
        )
        .await,
    )
}

async fn meta_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    path: web::Path<String>,
) -> HttpResponse {
    let asset_id = path.into_inner();
    handle_result(
        get_meta(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            asset_id.as_str(),
        )
        .await,
    )
}

async fn mint_batches_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    path: web::Path<String>,
) -> HttpResponse {
    let batch_key = path.into_inner();
    handle_result(
        get_mint_batches(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            batch_key.as_str(),
        )
        .await,
    )
}

async fn list_mint_batches_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    handle_result(
        list_all_mint_batches(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
        )
        .await,
    )
}

async fn cancel_mint_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    handle_result(
        cancel_mint(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
        )
        .await,
    )
}

async fn fund_mint_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<MintFundRequest>,
) -> HttpResponse {
    handle_result(
        fund_mint(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
        )
        .await,
    )
}

async fn finalize_mint_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<MintFinalizeRequest>,
) -> HttpResponse {
    handle_result(
        finalize_mint(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
        )
        .await,
    )
}

async fn seal_mint_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<MintSealRequest>,
) -> HttpResponse {
    handle_result(
        seal_mint(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
        )
        .await,
    )
}

async fn transfers_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    handle_result(
        get_transfers(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
        )
        .await,
    )
}

async fn register_transfer_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<TransferRegisterRequest>,
) -> HttpResponse {
    handle_result(
        register_transfer(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
        )
        .await,
    )
}

async fn utxos_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    handle_result(
        get_utxos(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
        )
        .await,
    )
}

fn handle_result<T: serde::Serialize>(result: Result<T, AppError>) -> HttpResponse {
    match result {
        Ok(value) => HttpResponse::Ok().json(value),
        Err(e) => {
            let status = e.status_code();
            HttpResponse::build(status)
                .json(serde_json::json!({"error": e.to_string(), "type": format!("{:?}", e)}))
        }
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/assets")
            .route(web::get().to(list_handler))
            .route(web::post().to(mint_handler)),
    )
    .service(web::resource("/assets/balance").route(web::get().to(balance_handler)))
    .service(web::resource("/assets/groups").route(web::get().to(groups_handler)))
    .service(web::resource("/assets/meta/asset-id/{asset_id}").route(web::get().to(meta_handler)))
    .service(web::resource("/assets/mint/batches/").route(web::get().to(list_mint_batches_handler)))
    .service(
        web::resource("/assets/mint/batches/{batch_key}")
            .route(web::get().to(mint_batches_handler)),
    )
    .service(web::resource("/assets/mint/cancel").route(web::post().to(cancel_mint_handler)))
    .service(web::resource("/assets/mint/fund").route(web::post().to(fund_mint_handler)))
    .service(web::resource("/assets/mint/finalize").route(web::post().to(finalize_mint_handler)))
    .service(web::resource("/assets/mint/seal").route(web::post().to(seal_mint_handler)))
    .service(web::resource("/assets/transfers").route(web::get().to(transfers_handler)))
    .service(
        web::resource("/assets/transfers/register")
            .route(web::post().to(register_transfer_handler)),
    )
    .service(web::resource("/assets/utxos").route(web::get().to(utxos_handler)));
}
