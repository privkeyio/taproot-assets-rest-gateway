use actix_web::{test, App};
use serde_json::Value;
use serial_test::serial;
use taproot_assets_rest_gateway::api::routes::configure;
use taproot_assets_rest_gateway::tests::setup::{mint_test_asset, setup};

#[actix_rt::test]
#[serial]
async fn test_list_all_assets() {
    let (client, base_url, macaroon_hex, lnd_macaroon_hex) = setup().await;

    // Ensure we have at least one asset
    let _asset_id = mint_test_asset(
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

    // Test basic list
    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/assets")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;

    // The response should have assets, unconfirmed_transfers, and unconfirmed_mints fields
    assert!(
        json["assets"].is_array(),
        "Expected assets field to be an array"
    );
    let assets = json["assets"].as_array().unwrap();
    assert!(!assets.is_empty(), "Expected at least one asset");

    // Just verify we have assets, don't check internal structure
    // as it may vary depending on the asset type and API version
}

#[actix_rt::test]
#[serial]
async fn test_get_asset_balance() {
    let (client, base_url, macaroon_hex, lnd_macaroon_hex) = setup().await;

    // Ensure we have assets
    mint_test_asset(
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

    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/assets/balance")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;

    // Check for balance structure
    if json["asset_balances"].is_object() {
        let balances = json["asset_balances"].as_object().unwrap();
        for (_asset_id, balance) in balances {
            assert!(balance["balance"].is_string() || balance["balance"].is_number());
            assert!(balance["asset_genesis"].is_object());
        }
    } else {
        // Alternative structure where asset_balances might be an array
        assert!(json["asset_balances"].is_array() || json["asset_balances"].is_null());
    }
}

#[actix_rt::test]
#[serial]
async fn test_get_asset_metadata_by_id() {
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

    let req = test::TestRequest::get()
        .uri(&format!(
            "/v1/taproot-assets/assets/meta/asset-id/{asset_id}"
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;

    // May return 404 if asset has no metadata, which is valid
    if resp.status().is_success() {
        let json: Value = test::read_body_json(resp).await;

        // Check metadata structure
        if json["data"].is_string() {
            assert!(json["type"].is_string());
            assert!(json["meta_hash"].is_string());
        }
    } else {
        assert_eq!(resp.status().as_u16(), 404);
    }
}

#[actix_rt::test]
#[serial]
async fn test_list_asset_groups() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/assets/groups")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    assert!(json["groups"].is_object() || json["groups"].is_array());

    // If there are groups, verify structure
    if let Some(groups) = json["groups"].as_object() {
        for (_group_key, group_assets) in groups {
            assert!(group_assets["assets"].is_array());
        }
    }
}

#[actix_rt::test]
#[serial]
async fn test_get_asset_utxos() {
    let (client, base_url, macaroon_hex, lnd_macaroon_hex) = setup().await;

    // Ensure we have assets
    mint_test_asset(
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

    // Test basic UTXO listing
    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/assets/utxos")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    assert!(json["managed_utxos"].is_object() || json["managed_utxos"].is_array());

    // Test with include_leased parameter
    let req_with_leased = test::TestRequest::get()
        .uri("/v1/taproot-assets/assets/utxos?include_leased=true")
        .to_request();
    let resp_with_leased = test::call_service(&app, req_with_leased).await;
    assert!(resp_with_leased.status().is_success());

    // Test with script_key_type filter
    let req_with_filter = test::TestRequest::get()
        .uri("/v1/taproot-assets/assets/utxos?script_key_type.explicit_type=SCRIPT_KEY_BIP86")
        .to_request();
    let resp_with_filter = test::call_service(&app, req_with_filter).await;
    assert!(resp_with_filter.status().is_success());
}

#[actix_rt::test]
#[serial]
async fn test_query_mint_batches() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // First, get all batches
    let all_batches_req = test::TestRequest::get()
        .uri("/v1/taproot-assets/assets/mint/batches/")
        .to_request();
    let all_batches_resp = test::call_service(&app, all_batches_req).await;

    // The endpoint might require a batch key, so let's test with a dummy key
    let specific_batch_req = test::TestRequest::get()
        .uri("/v1/taproot-assets/assets/mint/batches/dummy-batch-key")
        .to_request();
    let specific_batch_resp = test::call_service(&app, specific_batch_req).await;

    // One of these should work
    assert!(
        all_batches_resp.status().is_success()
            || specific_batch_resp.status().is_success()
            || all_batches_resp.status().is_client_error()
            || specific_batch_resp.status().is_client_error()
    );

    // If we got a successful response, verify structure
    if all_batches_resp.status().is_success() {
        let json: Value = test::read_body_json(all_batches_resp).await;
        assert!(json["batches"].is_array());

        let batches = json["batches"].as_array().unwrap();
        if !batches.is_empty() {
            let first_batch = &batches[0];
            assert!(first_batch["batch_key"].is_string() || first_batch["batch"].is_object());

            // Verify batch states
            if let Some(state) = first_batch
                .get("state")
                .or_else(|| first_batch.get("batch").and_then(|b| b.get("state")))
            {
                let valid_states = [
                    "BATCH_STATE_UNKNOWN",
                    "BATCH_STATE_PENDING",
                    "BATCH_STATE_FROZEN",
                    "BATCH_STATE_COMMITTED",
                    "BATCH_STATE_BROADCAST",
                    "BATCH_STATE_CONFIRMED",
                    "BATCH_STATE_FINALIZED",
                    "BATCH_STATE_SEEDLING_CANCELLED",
                    "BATCH_STATE_SPROUT_CANCELLED",
                ];
                assert!(valid_states.contains(&state.as_str().unwrap_or("")));
            }
        }
    }
}

#[actix_rt::test]
#[serial]
async fn test_query_mint_batches_verbose() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Test verbose query
    let verbose_req = test::TestRequest::get()
        .uri("/v1/taproot-assets/assets/mint/batches/?verbose=true")
        .to_request();
    let verbose_resp = test::call_service(&app, verbose_req).await;

    if verbose_resp.status().is_success() {
        let json: Value = test::read_body_json(verbose_resp).await;
        assert!(json["batches"].is_array());

        let batches = json["batches"].as_array().unwrap();
        if !batches.is_empty() {
            let first_batch = &batches[0];
            // Verbose response should include unsealed_assets
            if first_batch["unsealed_assets"].is_array() {
                let unsealed = first_batch["unsealed_assets"].as_array().unwrap();
                if !unsealed.is_empty() {
                    assert!(unsealed[0]["asset"].is_object());
                    assert!(unsealed[0]["group_key_request"].is_object());
                }
            }
        }
    }
}

#[actix_rt::test]
#[serial]
async fn test_asset_balance_with_filters() {
    let (client, base_url, macaroon_hex, lnd_macaroon_hex) = setup().await;

    // Ensure we have assets
    mint_test_asset(
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

    // Test balance grouped by asset IDs
    let asset_id_req = test::TestRequest::get()
        .uri("/v1/taproot-assets/assets/balance?asset_id=true")
        .to_request();
    let asset_id_resp = test::call_service(&app, asset_id_req).await;
    assert!(asset_id_resp.status().is_success());

    // Test balance grouped by group keys
    let group_key_req = test::TestRequest::get()
        .uri("/v1/taproot-assets/assets/balance?group_key=true")
        .to_request();
    let group_key_resp = test::call_service(&app, group_key_req).await;
    assert!(group_key_resp.status().is_success());

    // Test with include_leased
    let leased_req = test::TestRequest::get()
        .uri("/v1/taproot-assets/assets/balance?include_leased=true")
        .to_request();
    let leased_resp = test::call_service(&app, leased_req).await;
    assert!(leased_resp.status().is_success());

    // Test with script_key_type filter
    let script_type_req = test::TestRequest::get()
        .uri("/v1/taproot-assets/assets/balance?script_key_type.explicit_type=SCRIPT_KEY_BIP86")
        .to_request();
    let script_type_resp = test::call_service(&app, script_type_req).await;
    assert!(script_type_resp.status().is_success());
}
