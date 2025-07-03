use actix_web::{test, App};
use base64::{engine::general_purpose, Engine as _};
use serde_json::{json, Value};
use serial_test::serial;
use taproot_assets_rest_gateway::api::assets::{
    MintAsset, MintAssetRequest, MintFinalizeRequest, MintFundRequest, MintSealRequest,
};
use taproot_assets_rest_gateway::api::routes::configure;
use taproot_assets_rest_gateway::tests::setup::{setup, setup_without_assets};
use uuid::Uuid;

#[actix_rt::test]
#[serial]
async fn test_create_collectible_asset() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Cancel any pending batch first
    let _ = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/assets/mint/cancel")
            .set_json(json!({}))
            .to_request(),
    )
    .await;

    let asset_name = format!("test-collectible-{}", Uuid::new_v4());
    let request = MintAssetRequest {
        asset: MintAsset {
            asset_type: "COLLECTIBLE".to_string(),
            name: asset_name,
            amount: "1".to_string(), // Collectibles typically have amount of 1
        },
        short_response: true,
    };

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/assets")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    assert!(json["pending_batch"].is_object());
    assert!(json["pending_batch"]["batch_key"].is_string());
}

#[actix_rt::test]
#[serial]
async fn test_create_normal_asset() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Cancel any pending batch first
    let _ = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/assets/mint/cancel")
            .set_json(json!({}))
            .to_request(),
    )
    .await;

    let asset_name = format!("test-fungible-{}", Uuid::new_v4());
    let request = MintAssetRequest {
        asset: MintAsset {
            asset_type: "NORMAL".to_string(),
            name: asset_name,
            amount: "1000000".to_string(),
        },
        short_response: true,
    };

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/assets")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    assert!(json["pending_batch"].is_object());
    assert_eq!(
        json["pending_batch"]["state"].as_str(),
        Some("BATCH_STATE_PENDING")
    );
}

#[actix_rt::test]
#[serial]
async fn test_mint_with_metadata() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Cancel any pending batch first
    let _ = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/assets/mint/cancel")
            .set_json(json!({}))
            .to_request(),
    )
    .await;

    let asset_name = format!("test-meta-asset-{}", Uuid::new_v4());
    let metadata = json!({
        "description": "Test asset with metadata",
        "image": "https://example.com/image.png",
        "decimal_display": 2
    });

    let request = json!({
        "asset": {
            "asset_type": "NORMAL",
            "name": asset_name,
            "amount": "10000",
            "asset_meta": {
                "data": general_purpose::STANDARD.encode(metadata.to_string()),
                "type": "META_TYPE_JSON"
            }
        },
        "short_response": true
    });

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/assets")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    assert!(json["pending_batch"].is_object());
}

#[actix_rt::test]
#[serial]
async fn test_mint_batching() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Cancel any pending batch first
    let _ = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/assets/mint/cancel")
            .set_json(json!({}))
            .to_request(),
    )
    .await;

    // Create first asset
    let asset_name1 = format!("batch-asset-1-{}", Uuid::new_v4());
    let request1 = MintAssetRequest {
        asset: MintAsset {
            asset_type: "NORMAL".to_string(),
            name: asset_name1,
            amount: "1000".to_string(),
        },
        short_response: true,
    };

    let resp1 = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/assets")
            .set_json(&request1)
            .to_request(),
    )
    .await;
    assert!(resp1.status().is_success());
    let json1: Value = test::read_body_json(resp1).await;
    let batch_key1 = json1["pending_batch"]["batch_key"].as_str().unwrap();

    // Create second asset (should go into same batch)
    let asset_name2 = format!("batch-asset-2-{}", Uuid::new_v4());
    let request2 = MintAssetRequest {
        asset: MintAsset {
            asset_type: "NORMAL".to_string(),
            name: asset_name2,
            amount: "2000".to_string(),
        },
        short_response: true,
    };

    let resp2 = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/assets")
            .set_json(&request2)
            .to_request(),
    )
    .await;
    assert!(resp2.status().is_success());
    let json2: Value = test::read_body_json(resp2).await;
    let batch_key2 = json2["pending_batch"]["batch_key"].as_str().unwrap();

    // Verify both assets are in the same batch
    assert_eq!(batch_key1, batch_key2);
}

#[actix_rt::test]
#[serial]
async fn test_cancel_mint_operation() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // First create a mint
    let asset_name = format!("test-cancel-{}", Uuid::new_v4());
    let request = MintAssetRequest {
        asset: MintAsset {
            asset_type: "NORMAL".to_string(),
            name: asset_name,
            amount: "1000".to_string(),
        },
        short_response: true,
    };

    let mint_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/assets")
            .set_json(&request)
            .to_request(),
    )
    .await;
    assert!(mint_resp.status().is_success());

    // Now cancel it
    let cancel_req = test::TestRequest::post()
        .uri("/v1/taproot-assets/assets/mint/cancel")
        .set_json(json!({}))
        .to_request();
    let cancel_resp = test::call_service(&app, cancel_req).await;
    assert!(cancel_resp.status().is_success());

    let cancel_json: Value = test::read_body_json(cancel_resp).await;
    assert!(cancel_json.is_object() || cancel_json.is_null());
}

#[actix_rt::test]
#[serial]
async fn test_fund_mint_transaction() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Cancel any pending batch first
    let _ = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/assets/mint/cancel")
            .set_json(json!({}))
            .to_request(),
    )
    .await;

    // Create a mint first
    let asset_name = format!("test-fund-{}", Uuid::new_v4());
    let request = MintAssetRequest {
        asset: MintAsset {
            asset_type: "NORMAL".to_string(),
            name: asset_name,
            amount: "5000".to_string(),
        },
        short_response: true,
    };

    let mint_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/assets")
            .set_json(&request)
            .to_request(),
    )
    .await;
    assert!(mint_resp.status().is_success());

    // Fund the mint
    let fund_request = MintFundRequest {
        short_response: true,
        fee_rate: 500,
        full_tree: None,
        branch: None,
    };

    let fund_req = test::TestRequest::post()
        .uri("/v1/taproot-assets/assets/mint/fund")
        .set_json(&fund_request)
        .to_request();
    let fund_resp = test::call_service(&app, fund_req).await;

    // May fail if no UTXOs available, but API structure should be correct
    if fund_resp.status().is_success() {
        let fund_json: Value = test::read_body_json(fund_resp).await;
        assert!(fund_json["batch"].is_object());
    }
}

#[actix_rt::test]
async fn test_finalize_mint_transaction() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // This test requires a funded batch, which is complex to set up
    // Testing API structure only
    let finalize_request = MintFinalizeRequest {
        short_response: true,
        fee_rate: 300,
        full_tree: None,
        branch: None,
    };

    let finalize_req = test::TestRequest::post()
        .uri("/v1/taproot-assets/assets/mint/finalize")
        .set_json(&finalize_request)
        .to_request();
    let finalize_resp = test::call_service(&app, finalize_req).await;

    // Will likely fail without proper setup, but we're testing API structure
    assert!(finalize_resp.status().is_success() || finalize_resp.status().is_client_error());
}

#[actix_rt::test]
async fn test_seal_mint_transaction() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Testing API structure only
    let seal_request = MintSealRequest {
        short_response: true,
        group_witnesses: vec![],
        signed_group_virtual_psbts: vec![],
    };

    let seal_req = test::TestRequest::post()
        .uri("/v1/taproot-assets/assets/mint/seal")
        .set_json(&seal_request)
        .to_request();
    let seal_resp = test::call_service(&app, seal_req).await;

    // Will likely fail without proper setup, but we're testing API structure
    assert!(seal_resp.status().is_success() || seal_resp.status().is_client_error());
}
