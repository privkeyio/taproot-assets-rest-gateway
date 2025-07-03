use crate::api::assets::{MintAsset, MintAssetRequest};
use crate::config::Config;
use crate::error::AppError;
use crate::types::{BaseUrl, MacaroonHex};
use actix_web::web;
use hex;
use reqwest::Client;
use serde_json::{json, Value};
use std::fs;
use std::sync::Once;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info, warn};
use uuid::Uuid;

// Global test initialization
static TEST_INIT: Once = Once::new();
static TRACING_INIT: Once = Once::new();

/// Initialize the test environment once
fn init_test_env() {
    TRACING_INIT.call_once(|| {
        // Set up logging for tests - RESPECT RUST_LOG
        // Default to warn for the application crate, but allow override
        let default_filter = "taproot_assets_rest_gateway=warn";
        let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| default_filter.to_string());

        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
            .with_test_writer() // Use test-friendly output
            .try_init();
    });

    TEST_INIT.call_once(|| {
        // Load test environment
        dotenv::from_filename(".env").ok();

        if std::env::var("TLS_VERIFY").is_err() {
            std::env::set_var("TLS_VERIFY", "false");
        }
    });
}

/// Ensure test assets exist before running tests
async fn ensure_test_assets_exist() -> Result<String, String> {
    let (client, base_url, macaroon_hex, lnd_macaroon_hex) = setup_internal().await;

    // Check for existing assets first
    if let Some(asset_id) =
        check_existing_assets(client.as_ref(), &base_url.0, &macaroon_hex.0).await
    {
        info!("Found existing asset: {}", asset_id);
        return Ok(asset_id);
    }

    info!("No existing assets found, creating test assets...");

    // Setup Bitcoin Core RPC
    let bitcoin_rpc_url =
        std::env::var("BITCOIN_RPC_URL").unwrap_or_else(|_| "http://127.0.0.1:18443".to_string());
    let bitcoin_rpc_user =
        std::env::var("BITCOIN_RPC_USER").map_err(|_| "BITCOIN_RPC_USER not set")?;
    let bitcoin_rpc_pass =
        std::env::var("BITCOIN_RPC_PASS").map_err(|_| "BITCOIN_RPC_PASS not set")?;
    let lnd_url = std::env::var("LND_URL").unwrap_or_else(|_| "https://127.0.0.1:8083".to_string());

    // Step 1: Ensure blockchain has mature coins
    info!("Step 1: Generating initial blocks for coinbase maturity...");
    ensure_mature_coins(
        client.as_ref(),
        &bitcoin_rpc_url,
        &bitcoin_rpc_user,
        &bitcoin_rpc_pass,
    )
    .await?;

    // Step 2: Ensure LND is funded
    info!("Step 2: Ensuring LND is funded...");
    ensure_lnd_funded(
        client.as_ref(),
        &bitcoin_rpc_url,
        &bitcoin_rpc_user,
        &bitcoin_rpc_pass,
        &lnd_url,
        &lnd_macaroon_hex,
    )
    .await?;

    // Step 3: Create and wait for asset
    info!("Step 3: Creating test asset...");
    let asset_id = create_and_wait_for_asset(
        client.as_ref(),
        &base_url.0,
        &macaroon_hex.0,
        &bitcoin_rpc_url,
        &bitcoin_rpc_user,
        &bitcoin_rpc_pass,
    )
    .await?;

    Ok(asset_id)
}

/// Ensure we have mature coins in Bitcoin Core
async fn ensure_mature_coins(
    client: &Client,
    rpc_url: &str,
    rpc_user: &str,
    rpc_pass: &str,
) -> Result<(), String> {
    // Check current balance
    let balance_request = json!({
        "jsonrpc": "1.0",
        "id": "test",
        "method": "getbalance",
        "params": []
    });

    let response = client
        .post(rpc_url)
        .basic_auth(rpc_user, Some(rpc_pass))
        .json(&balance_request)
        .send()
        .await
        .map_err(|e| format!("Failed to check balance: {e}"))?;

    let json: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse balance response: {e}"))?;

    let balance = json["result"].as_f64().unwrap_or(0.0);
    debug!("Current Bitcoin Core balance: {} BTC", balance);

    if balance < 50.0 {
        info!("Insufficient balance, mining blocks to generate coins...");
        let _address = get_new_address(client, rpc_url, rpc_user, rpc_pass)
            .await
            .map_err(|e| format!("Failed to get address: {e}"))?;

        // Mine 200 blocks to ensure maturity
        generate_blocks_with_retry(client, rpc_url, rpc_user, rpc_pass, 200)
            .await
            .map_err(|e| format!("Failed to generate blocks: {e}"))?;

        sleep(Duration::from_secs(2)).await;
    }

    Ok(())
}

/// Ensure LND wallet is funded
async fn ensure_lnd_funded(
    client: &Client,
    rpc_url: &str,
    rpc_user: &str,
    rpc_pass: &str,
    lnd_url: &str,
    lnd_macaroon_hex: &str,
) -> Result<(), String> {
    // Check LND balance
    let balance_url = format!("{lnd_url}/v1/balance/blockchain");
    let balance_resp = client
        .get(&balance_url)
        .header("Grpc-Metadata-macaroon", lnd_macaroon_hex)
        .send()
        .await
        .map_err(|e| format!("Failed to check LND balance: {e}"))?;

    if balance_resp.status().is_success() {
        let balance_json: Value = balance_resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse LND balance: {e}"))?;

        let confirmed_balance = balance_json["confirmed_balance"]
            .as_str()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);

        debug!("LND confirmed balance: {} sats", confirmed_balance);

        if confirmed_balance > 1_000_000 {
            // More than 0.01 BTC
            info!("LND already has sufficient balance");
            return Ok(());
        }
    }

    // Get new LND address
    info!("Getting new LND address...");
    let lnd_address = get_lnd_address(client, lnd_url, lnd_macaroon_hex)
        .await
        .map_err(|e| format!("Failed to get LND address: {e}"))?;

    debug!("LND address: {}", lnd_address);

    // Send funds to LND
    info!("Sending funds to LND...");
    let send_request = json!({
        "jsonrpc": "1.0",
        "id": "test",
        "method": "sendtoaddress",
        "params": [lnd_address, 5.0] // Send 5 BTC
    });

    let send_resp = client
        .post(rpc_url)
        .basic_auth(rpc_user, Some(rpc_pass))
        .json(&send_request)
        .send()
        .await
        .map_err(|e| format!("Failed to send to LND: {e}"))?;

    if !send_resp.status().is_success() {
        let error = send_resp.text().await.unwrap_or_default();
        return Err(format!("Failed to send funds to LND: {error}"));
    }

    // Mine blocks to confirm
    info!("Mining blocks to confirm LND funding...");
    generate_blocks_with_retry(client, rpc_url, rpc_user, rpc_pass, 6)
        .await
        .map_err(|e| format!("Failed to mine blocks: {e}"))?;

    // Wait for LND to see the funds
    sleep(Duration::from_secs(10)).await;

    Ok(())
}

/// Create an asset and wait for it to appear
async fn create_and_wait_for_asset(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    rpc_url: &str,
    rpc_user: &str,
    rpc_pass: &str,
) -> Result<String, String> {
    // Clean up any existing batches
    cancel_pending_batch(client, base_url, macaroon_hex).await;
    sleep(Duration::from_secs(2)).await;

    // Create mint request
    let asset_name = format!("test-asset-{}", Uuid::new_v4());
    let request = MintAssetRequest {
        asset: MintAsset {
            asset_type: "NORMAL".to_string(),
            name: asset_name.clone(),
            amount: "100000".to_string(),
        },
        short_response: true,
    };

    info!("Creating mint batch for asset: {}", asset_name);

    // Step 1: Create the batch
    let mint_url = format!("{base_url}/v1/taproot-assets/assets");
    let response = client
        .post(&mint_url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Failed to create mint batch: {e}"))?;

    if !response.status().is_success() {
        let error = response.text().await.unwrap_or_default();
        return Err(format!("Mint request failed: {error}"));
    }

    let mint_json: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse mint response: {e}"))?;

    let batch_key = mint_json["pending_batch"]["batch_key"]
        .as_str()
        .ok_or("Batch key not found in response")?
        .to_string();

    info!("Batch created with key: {}", batch_key);

    // Step 2: Fund the batch
    sleep(Duration::from_secs(3)).await;

    info!("Funding the batch...");
    let fund_url = format!("{base_url}/v1/taproot-assets/assets/mint/fund");

    let fee_rate = std::env::var("TAPD_TEST_FEE_RATE")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(300);

    debug!("Using fee rate: {} sat/kw", fee_rate);

    let fund_request = json!({
        "short_response": true,
        "fee_rate": fee_rate
    });

    let fund_resp = client
        .post(&fund_url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&fund_request)
        .send()
        .await
        .map_err(|e| format!("Failed to fund batch: {e}"))?;

    if !fund_resp.status().is_success() {
        let error = fund_resp.text().await.unwrap_or_default();
        if !error.contains("already funded") {
            warn!("Fund request failed with fee_rate={}: {}", fee_rate, error);
            return Err(format!("Fund request failed: {error}"));
        }
    }

    info!("Batch funded successfully");

    // Step 3: Finalize the batch
    sleep(Duration::from_secs(3)).await;

    info!("Finalizing the batch...");
    let finalize_url = format!("{base_url}/v1/taproot-assets/assets/mint/finalize");
    let finalize_request = json!({
        "short_response": true
    });

    let finalize_resp = client
        .post(&finalize_url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&finalize_request)
        .send()
        .await
        .map_err(|e| format!("Failed to finalize batch: {e}"))?;

    if !finalize_resp.status().is_success() {
        let error = finalize_resp.text().await.unwrap_or_default();
        warn!("Finalize request returned error: {}", error);
        info!("Continuing with seal despite finalize error...");
    } else {
        info!("Batch finalized successfully");
    }

    // Step 4: Seal the batch (broadcast the transaction)
    sleep(Duration::from_secs(2)).await;

    info!("Sealing the batch...");
    let seal_url = format!("{base_url}/v1/taproot-assets/assets/mint/seal");
    let seal_request = json!({
        "short_response": true,
        "group_witnesses": [],
        "signed_group_virtual_psbts": []
    });

    let seal_resp = client
        .post(&seal_url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&seal_request)
        .send()
        .await
        .map_err(|e| format!("Failed to seal batch: {e}"))?;

    if !seal_resp.status().is_success() {
        let error = seal_resp.text().await.unwrap_or_default();
        warn!("Seal request failed (might be auto-sealed): {}", error);
    } else {
        info!("Batch sealed successfully");
    }

    // Step 5: Mine blocks and wait for the asset to appear
    info!("Mining blocks and waiting for asset to appear...");
    let start_time = std::time::Instant::now();
    let timeout = Duration::from_secs(300); // 5 minute timeout
    let mut last_block_time = std::time::Instant::now();

    while start_time.elapsed() < timeout {
        // Mine blocks periodically
        if last_block_time.elapsed() > Duration::from_secs(15) {
            debug!("Mining more blocks...");
            generate_blocks_with_retry(client, rpc_url, rpc_user, rpc_pass, 5)
                .await
                .ok();
            last_block_time = std::time::Instant::now();
        }

        // Check for assets
        let assets_url = format!("{base_url}/v1/taproot-assets/assets");
        if let Ok(resp) = client
            .get(&assets_url)
            .header("Grpc-Metadata-macaroon", macaroon_hex)
            .send()
            .await
        {
            if let Ok(json) = resp.json::<Value>().await {
                if let Some(assets) = json["assets"].as_array() {
                    debug!("Found {} assets", assets.len());

                    for asset in assets {
                        if let Some(genesis) = asset.get("asset_genesis") {
                            if let Some(asset_id) =
                                genesis.get("asset_id").and_then(|id| id.as_str())
                            {
                                info!("Asset created successfully with ID: {}", asset_id);

                                // Mine extra blocks for deeper confirmation
                                info!("Mining extra blocks for confirmation...");
                                generate_blocks_with_retry(client, rpc_url, rpc_user, rpc_pass, 10)
                                    .await
                                    .ok();
                                sleep(Duration::from_secs(5)).await;

                                return Ok(asset_id.to_string());
                            }
                        }
                    }
                }
            }
        }

        sleep(Duration::from_secs(3)).await;
    }

    Err(format!(
        "Asset did not appear within {} seconds",
        timeout.as_secs()
    ))
}

/// Internal setup function
async fn setup_internal() -> (
    web::Data<Client>,
    web::Data<BaseUrl>,
    web::Data<MacaroonHex>,
    String,
) {
    init_test_env();

    let config = Config::load().expect("Failed to load test configuration");

    let macaroon_bytes = fs::read(&config.macaroon_path).expect("Failed to read tapd macaroon");
    let macaroon_hex = hex::encode(macaroon_bytes);

    let lnd_macaroon_bytes =
        fs::read(&config.lnd_macaroon_path).expect("Failed to read LND macaroon");
    let lnd_macaroon_hex = hex::encode(lnd_macaroon_bytes);

    let base_url = format!("https://{}", config.taproot_assets_host);

    let mut client_builder = Client::builder().timeout(Duration::from_secs(60));

    if !config.tls_verify {
        client_builder = client_builder.danger_accept_invalid_certs(true);
    }

    let client = client_builder.build().unwrap();

    (
        web::Data::new(client),
        web::Data::new(BaseUrl(base_url)),
        web::Data::new(MacaroonHex(macaroon_hex)),
        lnd_macaroon_hex,
    )
}

/// Main setup function that ensures assets exist
pub async fn setup() -> (
    web::Data<Client>,
    web::Data<BaseUrl>,
    web::Data<MacaroonHex>,
    String,
) {
    let setup = setup_internal().await;

    // Ensure assets exist for tests that need them
    static ASSET_ID: tokio::sync::OnceCell<Result<String, String>> =
        tokio::sync::OnceCell::const_new();

    let result = ASSET_ID
        .get_or_init(|| async { ensure_test_assets_exist().await })
        .await;

    match result {
        Ok(asset_id) => {
            debug!("Using asset ID for tests: {}", asset_id);
        }
        Err(e) => {
            warn!("Failed to ensure test assets: {}", e);
            warn!("Tests requiring assets may fail!");
        }
    }

    setup
}

/// Setup for tests that don't require minted assets
pub async fn setup_without_assets() -> (
    web::Data<Client>,
    web::Data<BaseUrl>,
    web::Data<MacaroonHex>,
) {
    let (client, base_url, macaroon_hex, _) = setup_internal().await;
    (client, base_url, macaroon_hex)
}

async fn get_new_address(
    client: &Client,
    rpc_url: &str,
    rpc_user: &str,
    rpc_pass: &str,
) -> Result<String, AppError> {
    let request_body = json!({
        "jsonrpc": "1.0",
        "id": "test",
        "method": "getnewaddress",
        "params": []
    });

    let response = client
        .post(rpc_url)
        .basic_auth(rpc_user, Some(rpc_pass))
        .json(&request_body)
        .send()
        .await
        .map_err(AppError::RequestError)?;

    let json: Value = response.json().await.map_err(AppError::RequestError)?;

    json["result"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| AppError::ValidationError("Failed to get new address".to_string()))
}

async fn generate_blocks(
    client: &Client,
    rpc_url: &str,
    rpc_user: &str,
    rpc_pass: &str,
    num_blocks: u32,
) -> Result<(), AppError> {
    let address = get_new_address(client, rpc_url, rpc_user, rpc_pass).await?;

    let request_body = json!({
        "jsonrpc": "1.0",
        "id": "test",
        "method": "generatetoaddress",
        "params": [num_blocks, address]
    });

    let response = client
        .post(rpc_url)
        .basic_auth(rpc_user, Some(rpc_pass))
        .json(&request_body)
        .send()
        .await
        .map_err(AppError::RequestError)?;

    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_default();
        return Err(AppError::ValidationError(format!(
            "Failed to generate blocks with status {status}: {error_body}"
        )));
    }

    debug!("Generated {} blocks", num_blocks);
    Ok(())
}

pub async fn generate_blocks_with_retry(
    client: &Client,
    rpc_url: &str,
    rpc_user: &str,
    rpc_pass: &str,
    num_blocks: u32,
) -> Result<(), AppError> {
    let max_retries = 3;
    let mut attempts = 0;

    while attempts < max_retries {
        match generate_blocks(client, rpc_url, rpc_user, rpc_pass, num_blocks).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                attempts += 1;
                if attempts < max_retries {
                    warn!(
                        "Block generation attempt {} failed: {}, retrying...",
                        attempts, e
                    );
                    sleep(Duration::from_secs(2)).await;
                } else {
                    return Err(e);
                }
            }
        }
    }

    Err(AppError::ValidationError(
        "Failed to generate blocks after retries".to_string(),
    ))
}

async fn get_lnd_address(
    client: &Client,
    lnd_url: &str,
    macaroon_hex: &str,
) -> Result<String, AppError> {
    let url = format!("{lnd_url}/v1/newaddress?type=0");

    let response = client
        .get(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .send()
        .await
        .map_err(AppError::RequestError)?;

    let json: Value = response.json().await.map_err(AppError::RequestError)?;

    json["address"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| AppError::ValidationError("Failed to get LND address".to_string()))
}

async fn cancel_pending_batch(client: &Client, base_url: &str, macaroon_hex: &str) {
    debug!("Attempting to cancel any pending batch...");

    let cancel_url = format!("{base_url}/v1/taproot-assets/assets/mint/cancel");
    let _ = client
        .post(&cancel_url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&json!({}))
        .send()
        .await;
}

async fn check_existing_assets(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Option<String> {
    let url = format!("{base_url}/v1/taproot-assets/assets");

    if let Ok(response) = client
        .get(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .send()
        .await
    {
        if let Ok(json) = response.json::<Value>().await {
            if let Some(assets) = json["assets"].as_array() {
                if !assets.is_empty() {
                    if let Some(first_asset) = assets.first() {
                        if let Some(genesis) = first_asset.get("asset_genesis") {
                            if let Some(asset_id) =
                                genesis.get("asset_id").and_then(|id| id.as_str())
                            {
                                return Some(asset_id.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

pub async fn mint_test_asset(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    _lnd_macaroon_hex: &str,
) -> String {
    // Always try to reuse existing assets first
    if let Some(asset_id) = check_existing_assets(client, base_url, macaroon_hex).await {
        debug!("Reusing existing asset: {}", asset_id);
        return asset_id;
    }

    // Wait for any ongoing asset creation to complete
    static ASSET_CREATION_IN_PROGRESS: tokio::sync::Mutex<bool> =
        tokio::sync::Mutex::const_new(false);

    let mut in_progress = ASSET_CREATION_IN_PROGRESS.lock().await;

    // Check again after acquiring the lock
    if let Some(asset_id) = check_existing_assets(client, base_url, macaroon_hex).await {
        debug!("Found asset after acquiring lock: {}", asset_id);
        return asset_id;
    }

    // If no other test is creating assets, we'll create them
    if !*in_progress {
        *in_progress = true;
        drop(in_progress); // Release lock during asset creation

        info!("No assets found, creating new test assets for fresh setup...");

        // Get the required environment variables
        let bitcoin_rpc_url = std::env::var("BITCOIN_RPC_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:18443".to_string());
        let bitcoin_rpc_user =
            std::env::var("BITCOIN_RPC_USER").expect("BITCOIN_RPC_USER must be set for tests");
        let bitcoin_rpc_pass =
            std::env::var("BITCOIN_RPC_PASS").expect("BITCOIN_RPC_PASS must be set for tests");

        match create_and_wait_for_asset(
            client,
            base_url,
            macaroon_hex,
            &bitcoin_rpc_url,
            &bitcoin_rpc_user,
            &bitcoin_rpc_pass,
        )
        .await
        {
            Ok(asset_id) => {
                let mut in_progress = ASSET_CREATION_IN_PROGRESS.lock().await;
                *in_progress = false;
                return asset_id;
            }
            Err(e) => {
                let mut in_progress = ASSET_CREATION_IN_PROGRESS.lock().await;
                *in_progress = false;
                panic!("Failed to create test assets: {e}");
            }
        }
    }

    // Another test is creating assets, wait for it to complete
    drop(in_progress);
    info!("Waiting for another test to create assets...");

    let start_time = std::time::Instant::now();
    let timeout = Duration::from_secs(60);

    while start_time.elapsed() < timeout {
        if let Some(asset_id) = check_existing_assets(client, base_url, macaroon_hex).await {
            debug!("Found asset created by another test: {}", asset_id);
            return asset_id;
        }
        sleep(Duration::from_secs(2)).await;
    }

    panic!("No assets available after waiting. Run the test setup script or ensure tapd is properly configured.");
}
