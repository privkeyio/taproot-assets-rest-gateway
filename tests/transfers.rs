use actix_web::{test, App};
use serde_json::{json, Value};
use serial_test::serial;
use taproot_assets_rest_gateway::api::addresses::NewAddrRequest;
use taproot_assets_rest_gateway::api::assets::TransferRegisterRequest;
use taproot_assets_rest_gateway::api::proofs::ExportProofRequest;
use taproot_assets_rest_gateway::api::routes::configure;
use taproot_assets_rest_gateway::api::send::SendRequest;
use taproot_assets_rest_gateway::tests::setup::{
    assert_status_matches_body, generate_blocks_with_retry, mint_test_asset, setup,
    setup_without_assets, txid_to_internal_hex,
};
use tokio::time::{sleep, Duration};

#[actix_rt::test]
#[serial]
async fn test_complete_transfer_workflow() {
    let (client, base_url, macaroon_hex, lnd_macaroon_hex) = setup().await;
    let asset_id = mint_test_asset(
        client.as_ref(),
        &base_url.0,
        &macaroon_hex.0,
        &lnd_macaroon_hex,
    )
    .await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Step 1: Create receiving address
    let addr_req = NewAddrRequest {
        asset_id: asset_id.clone(),
        amt: "100".to_string(),
        script_key: None,
        internal_key: None,
        tapscript_sibling: None,
        proof_courier_addr: None,
        asset_version: None,
        address_version: None,
    };
    let addr_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/addrs")
            .set_json(&addr_req)
            .to_request(),
    )
    .await;
    let addr_json: Value = test::read_body_json(addr_resp).await;
    let tap_addr = addr_json["encoded"].as_str().unwrap().to_string();

    // Step 2: Send assets
    let send_req = SendRequest {
        tap_addrs: vec![tap_addr.clone()],
        fee_rate: Some(300),
        label: Some("Transfer test".to_string()),
        skip_proof_courier_ping_check: Some(true),
    };
    let send_body = serde_json::to_value(&send_req).expect("serialize send request");

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/send")
            .set_json(&send_body)
            .to_request(),
    )
    .await;
    let mut send_status = resp.status();
    let mut send_json: Value = test::read_body_json(resp).await;

    // A send needs confirmed on-chain coins to anchor with. Sibling tests leave
    // change unconfirmed, so mine and retry once on a funding failure rather
    // than mining unconditionally, which would perturb their UTXO state.
    let funding_failure = |body: &Value| {
        let msg = body["message"].as_str().unwrap_or_default();
        msg.contains("un-confirmed")
            || msg.contains("not enough witness outputs")
            || msg.contains("insufficient")
    };
    if !send_status.is_success() && funding_failure(&send_json) {
        let rpc_url = std::env::var("BITCOIN_RPC_URL").unwrap_or_default();
        let rpc_user = std::env::var("BITCOIN_RPC_USER").unwrap_or_default();
        let rpc_pass = std::env::var("BITCOIN_RPC_PASS").unwrap_or_default();
        let _ =
            generate_blocks_with_retry(client.as_ref(), &rpc_url, &rpc_user, &rpc_pass, 6).await;
        sleep(Duration::from_secs(2)).await;

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/taproot-assets/send")
                .set_json(&send_body)
                .to_request(),
        )
        .await;
        send_status = resp.status();
        send_json = test::read_body_json(resp).await;
    }

    assert_status_matches_body(send_status, &send_json);
    assert!(
        send_status.is_success(),
        "send must succeed: {send_status} {send_json}"
    );

    // Step 3: List transfers
    let list_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/v1/taproot-assets/assets/transfers")
            .to_request(),
    )
    .await;
    assert!(list_resp.status().is_success());
    let transfers_json: Value = test::read_body_json(list_resp).await;
    assert!(transfers_json["transfers"].is_array());

    // Step 4: Find our transfer
    let transfers = transfers_json["transfers"].as_array().unwrap();
    let our_transfer = transfers
        .iter()
        .find(|t| t.get("label").and_then(|l| l.as_str()) == Some("Transfer test"));

    if let Some(transfer) = our_transfer {
        // Verify transfer structure
        assert!(transfer["transfer_timestamp"].is_string());
        assert!(transfer["anchor_tx_hash"].is_string() || transfer["anchor_tx"].is_string());
        assert!(transfer["inputs"].is_array());
        assert!(transfer["outputs"].is_array());
    }
}

#[actix_rt::test]
async fn test_list_transfers_with_filters() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Test with anchor_txid filter
    let test_txid = "0000000000000000000000000000000000000000000000000000000000000000";
    let req = test::TestRequest::get()
        .uri(&format!(
            "/v1/taproot-assets/assets/transfers?anchor_txid={test_txid}"
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let json: Value = test::read_body_json(resp).await;
    assert!(json["transfers"].is_array());
}

#[actix_rt::test]
#[serial]
async fn test_register_transfer() {
    let (client, base_url, macaroon_hex, lnd_macaroon_hex) = setup().await;
    let asset_id = mint_test_asset(
        client.as_ref(),
        &base_url.0,
        &macaroon_hex.0,
        &lnd_macaroon_hex,
    )
    .await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Get asset details first
    let assets_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/v1/taproot-assets/assets")
            .to_request(),
    )
    .await;
    let assets_json: Value = test::read_body_json(assets_resp).await;
    let assets = assets_json["assets"].as_array().unwrap();

    let our_asset = assets.iter().find(|a| {
        a.get("asset_genesis")
            .and_then(|g| g.get("asset_id"))
            .and_then(|id| id.as_str())
            .map(|id| id == asset_id)
            .unwrap_or(false)
    });

    if let Some(asset) = our_asset {
        let script_key = asset["script_key"].as_str().unwrap_or("dummy_key");

        let request = TransferRegisterRequest {
            asset_id: asset_id.clone(),
            group_key: None,
            script_key: script_key.to_string(),
            outpoint: json!({
                "txid": hex::encode(vec![0u8; 32]),
                "output_index": 0
            }),
        };

        let req = test::TestRequest::post()
            .uri("/v1/taproot-assets/assets/transfers/register")
            .set_json(&request)
            .to_request();
        let resp = test::call_service(&app, req).await;
        let status = resp.status();
        let register_json: Value = test::read_body_json(resp).await;
        assert_status_matches_body(status, &register_json);

        if status.is_success() {
            assert!(register_json["registered_asset"].is_object());
        }
    }
}

#[actix_rt::test]
#[serial]
async fn test_export_and_verify_transfer_proof() {
    let (client, base_url, macaroon_hex, lnd_macaroon_hex) = setup().await;
    let asset_id = mint_test_asset(
        client.as_ref(),
        &base_url.0,
        &macaroon_hex.0,
        &lnd_macaroon_hex,
    )
    .await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Get asset details
    let assets_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/v1/taproot-assets/assets")
            .to_request(),
    )
    .await;
    let assets_json: Value = test::read_body_json(assets_resp).await;
    let assets = assets_json["assets"].as_array().unwrap();

    let our_asset = assets.iter().find(|a| {
        a.get("asset_genesis")
            .and_then(|g| g.get("asset_id"))
            .and_then(|id| id.as_str())
            .map(|id| id == asset_id)
            .unwrap_or(false)
    });

    if let Some(asset) = our_asset {
        let script_key = asset["script_key"].as_str().unwrap_or("dummy_key");
        let genesis_point = asset
            .get("asset_genesis")
            .and_then(|g| g.get("genesis_point"))
            .and_then(|p| p.as_str())
            .unwrap_or("0000000000000000000000000000000000000000000000000000000000000000:0");

        // Export proof
        let export_req = ExportProofRequest {
            asset_id: asset_id.clone(),
            script_key: script_key.to_string(),
            outpoint: json!({
                "txid": hex::encode(vec![0u8; 32]),
                "output_index": 0
            }),
        };

        let export_resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/taproot-assets/proofs/export")
                .set_json(&export_req)
                .to_request(),
        )
        .await;

        if export_resp.status().is_success() {
            let export_json: Value = test::read_body_json(export_resp).await;
            let raw_proof = export_json["raw_proof_file"].as_str().unwrap();

            // Verify the exported proof
            let verify_req = json!({
                "raw_proof_file": raw_proof,
                "genesis_point": genesis_point
            });

            let verify_resp = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/taproot-assets/proofs/verify")
                    .set_json(&verify_req)
                    .to_request(),
            )
            .await;

            if verify_resp.status().is_success() {
                let verify_json: Value = test::read_body_json(verify_resp).await;
                assert!(verify_json["valid"].is_boolean());
            }
        }
    }
}

#[actix_rt::test]
#[serial]
async fn test_transfer_output_types() {
    let (client, base_url, macaroon_hex, lnd_macaroon_hex) = setup().await;
    let asset_id = mint_test_asset(
        client.as_ref(),
        &base_url.0,
        &macaroon_hex.0,
        &lnd_macaroon_hex,
    )
    .await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Create address for partial amount (should create split)
    let addr_req = NewAddrRequest {
        asset_id: asset_id.clone(),
        amt: "300".to_string(), // Partial amount
        script_key: None,
        internal_key: None,
        tapscript_sibling: None,
        proof_courier_addr: None,
        asset_version: None,
        address_version: None,
    };
    let addr_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/addrs")
            .set_json(&addr_req)
            .to_request(),
    )
    .await;
    let addr_json: Value = test::read_body_json(addr_resp).await;

    // Check if address creation was successful
    if addr_json.get("error").is_some() || addr_json.get("code").is_some() {
        println!("Address creation failed: {addr_json:?}");
        return;
    }

    let addr = addr_json
        .get("encoded")
        .and_then(|v| v.as_str())
        .expect("Address should have encoded field")
        .to_string();

    // Send partial amount
    let send_req = SendRequest {
        tap_addrs: vec![addr],
        fee_rate: Some(300),
        label: Some("Split test".to_string()),
        skip_proof_courier_ping_check: Some(true),
    };
    let send_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/send")
            .set_json(&send_req)
            .to_request(),
    )
    .await;

    if send_resp.status().is_success() {
        let send_json: Value = test::read_body_json(send_resp).await;

        // Check for error in response body
        if send_json.get("error").is_some() || send_json.get("code").is_some() {
            println!("Send failed with error: {send_json:?}");
            return; // Exit gracefully
        }

        // Now safe to access outputs
        let outputs = send_json["transfer"]["outputs"].as_array().unwrap();

        for output in outputs {
            let output_type = output["output_type"].as_str().unwrap_or("");
            assert!(output_type == "OUTPUT_TYPE_SIMPLE" || output_type == "OUTPUT_TYPE_SPLIT_ROOT");
            if output_type == "OUTPUT_TYPE_SPLIT_ROOT" {
                assert!(output["split_commit_root_hash"].is_string());
            }
        }
    }
}

#[actix_rt::test]
#[serial]
async fn test_transfer_proof_delivery_status() {
    let (client, base_url, macaroon_hex, _lnd_macaroon_hex) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // List existing transfers
    let list_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/v1/taproot-assets/assets/transfers")
            .to_request(),
    )
    .await;
    assert!(list_resp.status().is_success());
    let transfers_json: Value = test::read_body_json(list_resp).await;
    let transfers = transfers_json["transfers"].as_array().unwrap();

    // Check proof delivery status on outputs
    for transfer in transfers {
        if let Some(outputs) = transfer["outputs"].as_array() {
            for output in outputs {
                if let Some(status) = output["proof_delivery_status"].as_str() {
                    let valid_statuses = [
                        "PROOF_DELIVERY_STATUS_NOT_APPLICABLE",
                        "PROOF_DELIVERY_STATUS_COMPLETE",
                        "PROOF_DELIVERY_STATUS_PENDING",
                    ];
                    assert!(valid_statuses.contains(&status));
                }
            }
        }
    }
}

#[actix_rt::test]
#[serial]
async fn test_transfer_timestamps_and_fees() {
    let (client, base_url, macaroon_hex, _lnd_macaroon_hex) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // List transfers
    let list_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/v1/taproot-assets/assets/transfers")
            .to_request(),
    )
    .await;
    assert!(list_resp.status().is_success());
    let transfers_json: Value = test::read_body_json(list_resp).await;
    let transfers = transfers_json["transfers"].as_array().unwrap();

    // Verify timestamp and fee fields
    for transfer in transfers {
        // Check timestamp
        assert!(transfer["transfer_timestamp"].is_string());
        let timestamp = transfer["transfer_timestamp"].as_str().unwrap();
        assert!(timestamp.parse::<i64>().is_ok());

        // Check fees
        if transfer["anchor_tx_chain_fees"].is_string() {
            let fees = transfer["anchor_tx_chain_fees"].as_str().unwrap();
            assert!(fees.parse::<i64>().is_ok());
        }

        // Check block info if confirmed
        if transfer["anchor_tx_block_hash"].is_object() {
            let block_hash = &transfer["anchor_tx_block_hash"];
            assert!(block_hash["hash"].is_string() || block_hash["hash_str"].is_string());
        }

        if transfer["anchor_tx_block_height"].is_number() {
            let height = transfer["anchor_tx_block_height"].as_u64().unwrap();
            // Height of 0 indicates an unconfirmed transaction, which is valid
            // Only confirmed transactions have height > 0
            if height > 0 {
                // For confirmed transactions, verify block hash is present
                assert!(transfer["anchor_tx_block_hash"].is_object());
            }
        }
    }
}

/// Regression test: the gateway used to drop query strings, so filtered
/// requests silently returned the unfiltered result set.
///
/// Note the upstream inconsistency: `/burns` takes `asset_id` as base64, while
/// `/assets/transfers` takes `anchor_txid` as hex in internal (reversed) order.
#[actix_rt::test]
#[serial]
async fn test_transfers_filter_is_forwarded() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/assets/transfers")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let status = resp.status();
    let all: Value = test::read_body_json(resp).await;
    assert_status_matches_body(status, &all);
    assert!(status.is_success());

    let Some(transfers) = all["transfers"].as_array().filter(|t| !t.is_empty()) else {
        return;
    };
    let total = transfers.len();
    let anchor = transfers[0]["anchor_tx_hash"]
        .as_str()
        .expect("anchor_tx_hash")
        .to_string();

    // No transfer is anchored to an all-zero txid, so a forwarded filter
    // returns nothing. If the query were dropped we would get all of them.
    let req = test::TestRequest::get()
        .uri(&format!(
            "/v1/taproot-assets/assets/transfers?anchor_txid={}",
            "0".repeat(64)
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let status = resp.status();
    let none: Value = test::read_body_json(resp).await;
    assert_status_matches_body(status, &none);
    assert!(status.is_success());
    let matched = none["transfers"].as_array().map(|t| t.len()).unwrap_or(0);
    assert_eq!(
        matched, 0,
        "filter was dropped: got {matched} transfers, unfiltered total is {total}"
    );

    // Filtering by a real anchor txid returns only that transfer.
    let req = test::TestRequest::get()
        .uri(&format!(
            "/v1/taproot-assets/assets/transfers?anchor_txid={}",
            txid_to_internal_hex(&anchor)
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let status = resp.status();
    let filtered: Value = test::read_body_json(resp).await;
    assert_status_matches_body(status, &filtered);
    assert!(status.is_success());

    let matched = filtered["transfers"].as_array().expect("transfers");
    assert!(
        !matched.is_empty(),
        "the anchoring transfer must be returned"
    );
    for t in matched {
        assert_eq!(t["anchor_tx_hash"].as_str(), Some(anchor.as_str()));
    }
}
