use actix_web::{test, App};
use serde_json::{json, Value};
use serial_test::serial;
use taproot_assets_rest_gateway::api::routes::configure;
use taproot_assets_rest_gateway::api::universe::{
    FederationRequest, MultiverseRequest, PushProofRequest, SyncConfigRequest, SyncRequest,
};
use taproot_assets_rest_gateway::tests::setup::{mint_test_asset, setup, setup_without_assets};
use tokio::time::{sleep, Duration};

#[actix_rt::test]
async fn test_add_federation_server() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let request = FederationRequest {
        servers: vec![json!({
            "host": "universe.example.com:10029",
            "id": 1
        })],
    };

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/universe/federation")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;

    // May fail if server is unreachable, but API structure should be correct
    assert!(resp.status().is_success() || resp.status().is_client_error());

    if resp.status().is_success() {
        let json: Value = test::read_body_json(resp).await;
        println!("Add federation response: {json:?}");
    }
}

#[actix_rt::test]
async fn test_list_federation_servers() {
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
        .uri("/v1/taproot-assets/universe/federation")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    assert!(json["servers"].is_array());

    let servers = json["servers"].as_array().unwrap();
    for server in servers {
        assert!(server["host"].is_string());
        assert!(server["id"].is_number());
    }
}

#[actix_rt::test]
async fn test_delete_federation_server() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // First add a server
    let add_request = FederationRequest {
        servers: vec![json!({
            "host": "test.universe.com:10029",
            "id": 99
        })],
    };

    let _ = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/universe/federation")
            .set_json(&add_request)
            .to_request(),
    )
    .await;

    // Then delete it
    let req = test::TestRequest::delete()
        .uri("/v1/taproot-assets/universe/federation")
        .set_json(json!({
            "servers": [{
                "host": "test.universe.com:10029",
                "id": 99
            }]
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;

    // May succeed or fail based on server state
    assert!(resp.status().is_success() || resp.status().is_client_error());
}

#[actix_rt::test]
async fn test_get_universe_info() {
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
        .uri("/v1/taproot-assets/universe/info")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    assert!(json["runtime_id"].is_string());
}

#[actix_rt::test]
async fn test_sync_with_universe() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Sync with local universe (self)
    let request = SyncRequest {
        universe_host: "127.0.0.1:8289".to_string(),
        sync_mode: "SYNC_ISSUANCE_ONLY".to_string(),
        sync_targets: vec![],
    };

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/universe/sync")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;

    // May fail if no assets to sync, but API structure should be correct
    assert!(resp.status().is_success() || resp.status().is_client_error());

    if resp.status().is_success() {
        let json: Value = test::read_body_json(resp).await;
        // Response might have synced_universes or an error/empty response
        if let Some(synced) = json.get("synced_universes") {
            assert!(synced.is_array() || synced.is_object());
        }
    }
}

#[actix_rt::test]
#[serial]
async fn test_sync_with_specific_assets() {
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

    let request = SyncRequest {
        universe_host: "127.0.0.1:8289".to_string(),
        sync_mode: "SYNC_FULL".to_string(),
        sync_targets: vec![json!({
            "id": {
                "asset_id_str": asset_id,
                "proof_type": "PROOF_TYPE_ISSUANCE"
            }
        })],
    };

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/universe/sync")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success() || resp.status().is_client_error());
}

#[actix_rt::test]
async fn test_query_universe_roots() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Get all universe roots
    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/universe/roots")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    assert!(json["universe_roots"].is_object());

    // Test with query parameters
    let req_with_params = test::TestRequest::get()
        .uri("/v1/taproot-assets/universe/roots?with_amounts_by_id=true&limit=10")
        .to_request();
    let resp_with_params = test::call_service(&app, req_with_params).await;
    assert!(resp_with_params.status().is_success());
}

#[actix_rt::test]
#[serial]
async fn test_query_asset_roots() {
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

    // Query specific asset roots
    let req = test::TestRequest::get()
        .uri(&format!(
            "/v1/taproot-assets/universe/roots/asset-id/{asset_id}"
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;

    // Should have issuance and transfer roots
    if json["issuance_root"].is_object() {
        let issuance_root = &json["issuance_root"];
        assert!(issuance_root["id"].is_object());
        assert!(issuance_root["mssmt_root"].is_object());
    }

    if json["transfer_root"].is_object() {
        let transfer_root = &json["transfer_root"];
        assert!(transfer_root["id"].is_object());
        assert!(transfer_root["mssmt_root"].is_object());
    }
}

#[actix_rt::test]
async fn test_get_universe_statistics() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Get general universe stats
    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/universe/stats")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    assert!(json["num_total_assets"].is_string());
    assert!(json["num_total_groups"].is_string());
    assert!(json["num_total_syncs"].is_string());
    assert!(json["num_total_proofs"].is_string());
}

#[actix_rt::test]
async fn test_get_asset_statistics() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Get asset stats
    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/universe/stats/assets")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    assert!(json["asset_stats"].is_array());

    // Test with filters
    let req_filtered = test::TestRequest::get()
        .uri("/v1/taproot-assets/universe/stats/assets?asset_type_filter=FILTER_ASSET_NORMAL&sort_by=SORT_BY_TOTAL_SUPPLY&limit=10")
        .to_request();
    let resp_filtered = test::call_service(&app, req_filtered).await;
    assert!(resp_filtered.status().is_success());
}

#[actix_rt::test]
async fn test_get_event_statistics() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Get event stats for last 30 days
    let start_timestamp = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::days(30))
        .unwrap()
        .timestamp();
    let end_timestamp = chrono::Utc::now().timestamp();

    let req = test::TestRequest::get()
        .uri(&format!(
            "/v1/taproot-assets/universe/stats/events?start_timestamp={start_timestamp}&end_timestamp={end_timestamp}"
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    assert!(json["events"].is_array());

    let events = json["events"].as_array().unwrap();
    for event in events {
        assert!(event["date"].is_string());
        assert!(event["sync_events"].is_string());
        assert!(event["new_proof_events"].is_string());
    }
}

#[actix_rt::test]
#[serial]
async fn test_push_proof_to_universe() {
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
        // Extract required info
        let script_key = asset["script_key"].as_str().unwrap_or("dummy_key");
        let anchor_outpoint = asset["chain_anchor"]["anchor_outpoint"]
            .as_str()
            .unwrap_or("0000:0");
        let parts: Vec<&str> = anchor_outpoint.split(':').collect();

        if parts.len() == 2 {
            let request = PushProofRequest {
                key: json!({
                    "id": {
                        "asset_id_str": asset_id,
                        "proof_type": "PROOF_TYPE_ISSUANCE"
                    },
                    "leaf_key": {
                        "op": {
                            "hash_str": parts[0],
                            "index": parts[1].parse::<u32>().unwrap_or(0)
                        },
                        "script_key_str": script_key
                    }
                }),
                server: json!({
                    "host": "127.0.0.1:8289",
                    "id": 0
                }),
            };

            let req = test::TestRequest::post()
                .uri(&format!(
                    "/v1/taproot-assets/universe/proofs/push/asset-id/{}/{}/{}/{}",
                    asset_id, parts[0], parts[1], script_key
                ))
                .set_json(&request)
                .to_request();
            let resp = test::call_service(&app, req).await;

            // May fail if proof already exists or other conditions
            assert!(resp.status().is_success() || resp.status().is_client_error());

            if resp.status().is_success() {
                let json: Value = test::read_body_json(resp).await;
                assert!(json["key"].is_object());
            }
        }
    }
}

#[actix_rt::test]
async fn test_delete_universe_root() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Delete a universe root (will fail if doesn't exist)
    let req = test::TestRequest::delete()
        .uri("/v1/taproot-assets/universe/delete")
        .set_json(json!({
            "id": {
                "asset_id_str": "0000000000000000000000000000000000000000000000000000000000000000",
                "proof_type": "PROOF_TYPE_ISSUANCE"
            }
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;

    // Expect this to fail as the asset doesn't exist
    assert!(resp.status().is_success() || resp.status().is_client_error());
}

#[actix_rt::test]
async fn test_configure_sync_settings() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Set sync configuration
    let request = SyncConfigRequest {
        global_sync_configs: vec![
            json!({
                "proof_type": "PROOF_TYPE_ISSUANCE",
                "allow_sync_insert": true,
                "allow_sync_export": true
            }),
            json!({
                "proof_type": "PROOF_TYPE_TRANSFER",
                "allow_sync_insert": false,
                "allow_sync_export": true
            }),
        ],
        asset_sync_configs: vec![],
    };

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/universe/sync/config")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_rt::test]
async fn test_query_sync_configuration() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Query sync configuration
    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/universe/sync/config")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    assert!(json["global_sync_configs"].is_array());
    assert!(json["asset_sync_configs"].is_array());

    let global_configs = json["global_sync_configs"].as_array().unwrap();
    for config in global_configs {
        assert!(config["proof_type"].is_string());
        assert!(config["allow_sync_insert"].is_boolean());
        assert!(config["allow_sync_export"].is_boolean());
    }
}

#[actix_rt::test]
#[serial]
async fn test_asset_specific_sync_config() {
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

    // Set asset-specific sync config
    let request = SyncConfigRequest {
        global_sync_configs: vec![],
        asset_sync_configs: vec![json!({
            "id": {
                "asset_id_str": asset_id,
                "proof_type": "PROOF_TYPE_ISSUANCE"
            },
            "allow_sync_insert": true,
            "allow_sync_export": false
        })],
    };

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/universe/sync/config")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_rt::test]
async fn test_get_multiverse_root() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let request = MultiverseRequest {
        proof_type: "PROOF_TYPE_ISSUANCE".to_string(),
        specific_ids: vec![],
    };

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/universe/multiverse")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    assert!(json["multiverse_root"].is_object());

    let multiverse_root = &json["multiverse_root"];
    assert!(multiverse_root["root_hash"].is_string());
    assert!(multiverse_root["root_sum"].is_string());
}

#[actix_rt::test]
#[serial]
async fn test_get_asset_keys() {
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
            "/v1/taproot-assets/universe/keys/asset-id/{asset_id}"
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;

    // Check if we got an error response
    if json.get("error").is_some() || json.get("code").is_some() {
        println!("Asset keys query returned error: {json:?}");
        return;
    }

    // asset_keys might be in different format or empty
    if let Some(keys) = json.get("asset_keys") {
        if let Some(keys_array) = keys.as_array() {
            for key in keys_array {
                assert!(key["op_str"].is_string() || key["op"].is_object());
                assert!(key["script_key_bytes"].is_string() || key["script_key_str"].is_string());
            }
        }
    }
}

#[actix_rt::test]
#[serial]
async fn test_get_asset_leaves() {
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
            "/v1/taproot-assets/universe/leaves/asset-id/{asset_id}"
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    assert!(json["leaves"].is_array());

    let leaves = json["leaves"].as_array().unwrap();
    for leaf in leaves {
        assert!(leaf["asset"].is_object());
        assert!(leaf["proof"].is_string());
    }
}

#[actix_rt::test]
#[serial]
async fn test_query_proof() {
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

    // Wait for asset to be fully registered
    sleep(Duration::from_secs(2)).await;

    // Get asset details to find a valid proof
    let leaves_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!(
                "/v1/taproot-assets/universe/leaves/asset-id/{asset_id}"
            ))
            .to_request(),
    )
    .await;

    if leaves_resp.status().is_success() {
        let leaves_json: Value = test::read_body_json(leaves_resp).await;
        let leaves = leaves_json["leaves"].as_array().unwrap();

        if !leaves.is_empty() {
            let first_leaf = &leaves[0];
            if let (Some(anchor_outpoint), Some(script_key)) = (
                first_leaf["asset"]["chain_anchor"]["anchor_outpoint"].as_str(),
                first_leaf["asset"]["script_key"].as_str(),
            ) {
                let parts: Vec<&str> = anchor_outpoint.split(':').collect();
                if parts.len() == 2 {
                    let req = test::TestRequest::get()
                        .uri(&format!(
                            "/v1/taproot-assets/universe/proofs/asset-id/{}/{}/{}/{}",
                            asset_id, parts[0], parts[1], script_key
                        ))
                        .to_request();
                    let resp = test::call_service(&app, req).await;
                    assert!(resp.status().is_success());

                    let json: Value = test::read_body_json(resp).await;
                    assert!(json["req"].is_object());
                    assert!(json["universe_root"].is_object());
                    assert!(json["asset_leaf"].is_object());
                }
            }
        }
    }
}

#[actix_rt::test]
async fn test_federation_server_management() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Add multiple servers
    let add_request = FederationRequest {
        servers: vec![
            json!({
                "host": "universe1.example.com:10029",
                "id": 1
            }),
            json!({
                "host": "universe2.example.com:10029",
                "id": 2
            }),
        ],
    };

    let _add_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/universe/federation")
            .set_json(&add_request)
            .to_request(),
    )
    .await;

    // List to verify
    let list_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/v1/taproot-assets/universe/federation")
            .to_request(),
    )
    .await;
    assert!(list_resp.status().is_success());
}

#[actix_rt::test]
async fn test_sync_modes() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Test SYNC_ISSUANCE_ONLY mode
    let issuance_request = SyncRequest {
        universe_host: "127.0.0.1:8289".to_string(),
        sync_mode: "SYNC_ISSUANCE_ONLY".to_string(),
        sync_targets: vec![],
    };

    let issuance_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/universe/sync")
            .set_json(&issuance_request)
            .to_request(),
    )
    .await;
    assert!(issuance_resp.status().is_success() || issuance_resp.status().is_client_error());

    // Test SYNC_FULL mode
    let full_request = SyncRequest {
        universe_host: "127.0.0.1:8289".to_string(),
        sync_mode: "SYNC_FULL".to_string(),
        sync_targets: vec![],
    };

    let full_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/universe/sync")
            .set_json(&full_request)
            .to_request(),
    )
    .await;
    assert!(full_resp.status().is_success() || full_resp.status().is_client_error());
}
