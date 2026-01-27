use super::handle_result;
use crate::error::AppError;
use crate::types::{BaseUrl, MacaroonHex};
use actix_web::{web, HttpResponse};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

#[derive(Debug, Serialize, Deserialize)]
pub struct DecodeProofRequest {
    pub raw_proof: String,
    pub proof_at_depth: Option<u32>,
    pub with_prev_witnesses: bool,
    pub with_meta_reveal: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportProofRequest {
    pub asset_id: String,
    pub script_key: String,
    pub outpoint: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnpackFileRequest {
    pub raw_proof_file: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyProofRequest {
    pub raw_proof_file: String,
    pub genesis_point: String,
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn decode_proof(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: DecodeProofRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Decoding proof");
    let url = format!("{base_url}/v1/taproot-assets/proofs/decode");
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
pub async fn export_proof(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: ExportProofRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Exporting proof for asset ID: {}", request.asset_id);
    let url = format!("{base_url}/v1/taproot-assets/proofs/export");
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
pub async fn unpack_proof_file(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: UnpackFileRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Unpacking proof file");
    let url = format!("{base_url}/v1/taproot-assets/proofs/unpack-file");
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
pub async fn verify_proof(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: VerifyProofRequest,
) -> Result<serde_json::Value, AppError> {
    info!(
        "Verifying proof with genesis point: {}",
        request.genesis_point
    );
    let url = format!("{base_url}/v1/taproot-assets/proofs/verify");
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

async fn decode(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<DecodeProofRequest>,
) -> HttpResponse {
    handle_result(
        decode_proof(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            req.into_inner(),
        )
        .await,
    )
}

async fn export(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<ExportProofRequest>,
) -> HttpResponse {
    handle_result(
        export_proof(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            req.into_inner(),
        )
        .await,
    )
}

async fn unpack_file(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<UnpackFileRequest>,
) -> HttpResponse {
    handle_result(
        unpack_proof_file(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            req.into_inner(),
        )
        .await,
    )
}

async fn verify(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<VerifyProofRequest>,
) -> HttpResponse {
    handle_result(
        verify_proof(
            client.as_ref(),
            &base_url.0,
            &macaroon_hex.0,
            req.into_inner(),
        )
        .await,
    )
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/proofs/decode").route(web::post().to(decode)))
        .service(web::resource("/proofs/export").route(web::post().to(export)))
        .service(web::resource("/proofs/unpack-file").route(web::post().to(unpack_file)))
        .service(web::resource("/proofs/verify").route(web::post().to(verify)));
}
