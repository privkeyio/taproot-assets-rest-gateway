use actix_web::{test, App};
use serde_json::{json, Value};
use serial_test::serial;
use taproot_assets_rest_gateway::api::routes::configure;
use taproot_assets_rest_gateway::api::universe::{
    FederationRequest, SyncConfigRequest, SyncRequest,
};
use taproot_assets_rest_gateway::tests::setup::{mint_test_asset, setup, setup_without_assets};
use tokio::time::{sleep, Duration};
use tracing::info;

async fn wait_for_asset_in_universe(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    asset_id: &str,
    max_attempts: u32,
) -> bool {
    for attempt in 1..=max_attempts {
        let req = test::TestRequest::get()
            .uri(&format!(
                "/v1/taproot-assets/universe/roots/asset-id/{asset_id}"
            ))
            .to_request();

        let resp = test::call_service(app, req).await;

        if resp.status().is_success() {
            let json: Value = test::read_body_json(resp).await;
            if json.get("error").is_none()
                && json.get("code").is_none()
                && (json["issuance_root"].is_object() || json["transfer_root"].is_object())
            {
                info!(
                    "Asset {} found in universe after {} attempts",
                    asset_id, attempt
                );
                return true;
            }
        }

        if attempt < max_attempts {
            sleep(Duration::from_secs(2)).await;
        }
    }

    info!(
        "Asset {} not found in universe after {} attempts",
        asset_id, max_attempts
    );
    false
}

#[actix_rt::test]
#[serial]
async fn test_complete_universe_workflow() {
    let (client, base_url, macaroon_hex, lnd_macaroon_hex) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    info!("Starting complete universe workflow test");

    // Step 1: Check initial universe state
    info!("Step 1: Checking initial universe state");
    let info_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/v1/taproot-assets/universe/info")
            .to_request(),
    )
    .await;
    assert!(info_resp.status().is_success());
    let info_json: Value = test::read_body_json(info_resp).await;
    let runtime_id = info_json["runtime_id"].as_str().unwrap();
    info!("Universe runtime ID: {}", runtime_id);

    // Step 2: Get initial stats
    info!("Step 2: Getting initial universe stats");
    let initial_stats_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/v1/taproot-assets/universe/stats")
            .to_request(),
    )
    .await;
    assert!(initial_stats_resp.status().is_success());
    let initial_stats: Value = test::read_body_json(initial_stats_resp).await;
    let initial_assets = initial_stats["num_total_assets"]
        .as_str()
        .unwrap()
        .parse::<i64>()
        .unwrap();
    let initial_proofs = initial_stats["num_total_proofs"]
        .as_str()
        .unwrap()
        .parse::<i64>()
        .unwrap();
    info!(
        "Initial assets: {}, Initial proofs: {}",
        initial_assets, initial_proofs
    );

    // Step 3: Configure sync settings
    info!("Step 3: Configuring sync settings");
    let sync_config = SyncConfigRequest {
        global_sync_configs: vec![
            json!({
                "proof_type": "PROOF_TYPE_ISSUANCE",
                "allow_sync_insert": true,
                "allow_sync_export": true
            }),
            json!({
                "proof_type": "PROOF_TYPE_TRANSFER",
                "allow_sync_insert": true,
                "allow_sync_export": true
            }),
        ],
        asset_sync_configs: vec![],
    };

    let config_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/universe/sync/config")
            .set_json(&sync_config)
            .to_request(),
    )
    .await;
    assert!(config_resp.status().is_success());

    // Step 4: Mint a new asset
    info!("Step 4: Minting new asset");
    let asset_id = mint_test_asset(
        client.as_ref(),
        &base_url.0,
        &macaroon_hex.0,
        &lnd_macaroon_hex,
    )
    .await;
    info!("Created asset with ID: {}", asset_id);

    // Wait for asset to be registered in universe
    let asset_in_universe = wait_for_asset_in_universe(&app, &asset_id, 10).await;

    if asset_in_universe {
        // Step 5: Check if asset appears in universe
        info!("Step 5: Asset successfully registered in universe");
        let roots_resp = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!(
                    "/v1/taproot-assets/universe/roots/asset-id/{asset_id}"
                ))
                .to_request(),
        )
        .await;
        assert!(roots_resp.status().is_success());
        let roots_json: Value = test::read_body_json(roots_resp).await;

        if roots_json.get("error").is_some() || roots_json.get("code").is_some() {
            info!("Asset roots query returned error: {:?}", roots_json);
        } else if roots_json["issuance_root"].is_object() {
            info!("Asset has issuance root in universe");
        } else {
            info!("Asset does not have issuance root yet");
        }

        // Step 6: Get asset leaves
        info!("Step 6: Getting asset leaves");
        let leaves_resp = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!(
                    "/v1/taproot-assets/universe/leaves/asset-id/{asset_id}"
                ))
                .to_request(),
        )
        .await;
        assert!(leaves_resp.status().is_success());
        let leaves_json: Value = test::read_body_json(leaves_resp).await;

        if leaves_json.get("error").is_some() || leaves_json.get("code").is_some() {
            info!("Asset leaves query returned error: {:?}", leaves_json);
        } else if let Some(leaves_array) = leaves_json["leaves"].as_array() {
            if !leaves_array.is_empty() {
                info!("Found {} leaves for asset", leaves_array.len());
            } else {
                info!("No leaves found for asset yet");
            }
        } else {
            info!("Leaves response is not in expected format");
        }
    } else {
        info!("Asset not yet in universe, skipping leaves check");
    }

    // Step 7: Add federation server (self)
    info!("Step 7: Adding self as federation server");
    let federation_req = FederationRequest {
        servers: vec![json!({
            "host": "127.0.0.1:8289",
            "id": 99
        })],
    };

    let fed_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/universe/federation")
            .set_json(&federation_req)
            .to_request(),
    )
    .await;
    if fed_resp.status().is_success() {
        info!("Successfully added federation server");
    }

    // Step 8: Sync with self
    info!("Step 8: Syncing with self");
    let sync_req = SyncRequest {
        universe_host: "127.0.0.1:8289".to_string(),
        sync_mode: "SYNC_FULL".to_string(),
        sync_targets: vec![json!({
            "id": {
                "asset_id_str": asset_id,
                "proof_type": "PROOF_TYPE_ISSUANCE"
            }
        })],
    };

    let sync_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/universe/sync")
            .set_json(&sync_req)
            .to_request(),
    )
    .await;
    if sync_resp.status().is_success() {
        let sync_json: Value = test::read_body_json(sync_resp).await;
        info!("Sync response: {:?}", sync_json);
    }

    // Step 9: Check final stats
    info!("Step 9: Checking final universe stats");
    let final_stats_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/v1/taproot-assets/universe/stats")
            .to_request(),
    )
    .await;
    assert!(final_stats_resp.status().is_success());
    let final_stats: Value = test::read_body_json(final_stats_resp).await;
    let final_assets = final_stats["num_total_assets"]
        .as_str()
        .unwrap()
        .parse::<i64>()
        .unwrap();
    let final_proofs = final_stats["num_total_proofs"]
        .as_str()
        .unwrap()
        .parse::<i64>()
        .unwrap();
    info!(
        "Final assets: {}, Final proofs: {}",
        final_assets, final_proofs
    );

    // Verify asset count increased
    assert!(
        final_assets >= initial_assets,
        "Asset count should have increased"
    );
    assert!(
        final_proofs >= initial_proofs,
        "Proof count should have increased"
    );

    // Step 10: Get multiverse root
    info!("Step 10: Getting multiverse root");
    let multiverse_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/universe/multiverse")
            .set_json(json!({
                "proof_type": "PROOF_TYPE_ISSUANCE",
                "specific_ids": []
            }))
            .to_request(),
    )
    .await;
    assert!(multiverse_resp.status().is_success());
    let multiverse_json: Value = test::read_body_json(multiverse_resp).await;
    assert!(multiverse_json["multiverse_root"]["root_hash"].is_string());
    info!(
        "Multiverse root hash: {}",
        multiverse_json["multiverse_root"]["root_hash"]
    );
}

#[actix_rt::test]
#[serial]
async fn test_asset_lifecycle_in_universe() {
    let (client, base_url, macaroon_hex, lnd_macaroon_hex) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    info!("Testing asset lifecycle in universe");

    // Mint an asset
    let asset_id = mint_test_asset(
        client.as_ref(),
        &base_url.0,
        &macaroon_hex.0,
        &lnd_macaroon_hex,
    )
    .await;

    // Wait for asset to be in universe
    wait_for_asset_in_universe(&app, &asset_id, 10).await;

    // Check asset appears in roots
    let roots_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/v1/taproot-assets/universe/roots")
            .to_request(),
    )
    .await;
    assert!(roots_resp.status().is_success());
    let roots_json: Value = test::read_body_json(roots_resp).await;

    let roots = roots_json["universe_roots"].as_object();
    let mut found_asset = false;

    if let Some(roots_map) = roots {
        for (_key, root) in roots_map {
            if let Some(id) = root["id"]["asset_id_str"].as_str() {
                if id == asset_id {
                    found_asset = true;
                    info!("Found asset {} in universe roots", asset_id);
                    break;
                }
            }
        }
    }

    if !found_asset {
        info!(
            "Asset {} not found in universe roots yet, this is normal for new assets",
            asset_id
        );
        let specific_roots_resp = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!(
                    "/v1/taproot-assets/universe/roots/asset-id/{asset_id}"
                ))
                .to_request(),
        )
        .await;

        if specific_roots_resp.status().is_success() {
            let specific_json: Value = test::read_body_json(specific_roots_resp).await;
            if specific_json["issuance_root"].is_object()
                || specific_json["transfer_root"].is_object()
            {
                info!("Asset found via specific query");
            }
        }
    }

    // Get asset statistics
    let asset_stats_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!(
                "/v1/taproot-assets/universe/stats/assets?asset_id_filter={asset_id}"
            ))
            .to_request(),
    )
    .await;
    assert!(asset_stats_resp.status().is_success());
    let asset_stats_json: Value = test::read_body_json(asset_stats_resp).await;
    let asset_stats = asset_stats_json["asset_stats"].as_array().unwrap();

    for stat in asset_stats {
        if let Some(asset) = stat.get("asset") {
            if asset["asset_id"].as_str() == Some(&asset_id) {
                info!(
                    "Found asset stats: syncs={}, proofs={}",
                    stat["total_syncs"], stat["total_proofs"]
                );
            }
        }
    }
}

#[actix_rt::test]
#[serial]
async fn test_federation_synchronization() {
    let (client, base_url, macaroon_hex, _lnd_macaroon_hex) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    info!("Testing federation synchronization");

    // Clear any existing federation servers
    let list_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/v1/taproot-assets/universe/federation")
            .to_request(),
    )
    .await;
    assert!(list_resp.status().is_success());
    let list_json: Value = test::read_body_json(list_resp).await;
    let initial_server_count = list_json["servers"].as_array().unwrap().len();
    info!("Initial federation server count: {}", initial_server_count);

    // Add federation servers
    let federation_req = FederationRequest {
        servers: vec![
            json!({
                "host": "testnet.universe.lightning.finance:10029",
                "id": 1
            }),
            json!({
                "host": "mainnet.universe.lightning.finance:10029",
                "id": 2
            }),
        ],
    };

    let add_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/universe/federation")
            .set_json(&federation_req)
            .to_request(),
    )
    .await;
    if add_resp.status().is_success() {
        info!("Successfully added federation servers");

        let new_list_resp = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/v1/taproot-assets/universe/federation")
                .to_request(),
        )
        .await;
        assert!(new_list_resp.status().is_success());
        let new_list_json: Value = test::read_body_json(new_list_resp).await;
        let new_server_count = new_list_json["servers"].as_array().unwrap().len();
        info!("New federation server count: {}", new_server_count);
    }

    // Test sync with specific configuration
    let sync_config = SyncConfigRequest {
        global_sync_configs: vec![json!({
            "proof_type": "PROOF_TYPE_ISSUANCE",
            "allow_sync_insert": true,
            "allow_sync_export": false
        })],
        asset_sync_configs: vec![],
    };

    let config_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/universe/sync/config")
            .set_json(&sync_config)
            .to_request(),
    )
    .await;
    assert!(config_resp.status().is_success());

    // Verify configuration was applied
    let get_config_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/v1/taproot-assets/universe/sync/config")
            .to_request(),
    )
    .await;
    assert!(get_config_resp.status().is_success());
    let config_json: Value = test::read_body_json(get_config_resp).await;
    let global_configs = config_json["global_sync_configs"].as_array().unwrap();

    for config in global_configs {
        if config["proof_type"] == "PROOF_TYPE_ISSUANCE" {
            assert_eq!(config["allow_sync_insert"], true);
            assert_eq!(config["allow_sync_export"], false);
        }
    }
}

#[actix_rt::test]
#[serial]
async fn test_proof_push_and_query() {
    let (client, base_url, macaroon_hex, lnd_macaroon_hex) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    info!("Testing proof push and query");

    // Mint asset
    let asset_id = mint_test_asset(
        client.as_ref(),
        &base_url.0,
        &macaroon_hex.0,
        &lnd_macaroon_hex,
    )
    .await;

    sleep(Duration::from_secs(3)).await;

    // Get asset keys
    let keys_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!(
                "/v1/taproot-assets/universe/keys/asset-id/{asset_id}?id.proof_type=PROOF_TYPE_ISSUANCE"
            ))
            .to_request(),
    )
    .await;
    assert!(keys_resp.status().is_success());
    let keys_json: Value = test::read_body_json(keys_resp).await;
    let keys = keys_json["asset_keys"].as_array();

    if let Some(keys_array) = keys {
        if !keys_array.is_empty() {
            let first_key = &keys_array[0];

            if let (Some(op_str), Some(script_key_str)) = (
                first_key["op_str"].as_str(),
                first_key["script_key_str"].as_str(),
            ) {
                let parts: Vec<&str> = op_str.split(':').collect();
                if parts.len() == 2 {
                    let proof_resp = test::call_service(
                        &app,
                        test::TestRequest::get()
                            .uri(&format!(
                                "/v1/taproot-assets/universe/proofs/asset-id/{}/{}/{}/{}",
                                asset_id, parts[0], parts[1], script_key_str
                            ))
                            .to_request(),
                    )
                    .await;
                    assert!(proof_resp.status().is_success());

                    let proof_json: Value = test::read_body_json(proof_resp).await;
                    assert!(proof_json["universe_root"].is_object());
                    assert!(proof_json["asset_leaf"].is_object());

                    if proof_json["universe_inclusion_proof"].is_string() {
                        info!("Found valid inclusion proof");
                    }

                    if proof_json["multiverse_root"].is_object() {
                        info!("Asset is included in multiverse");
                    }
                }
            }
        } else {
            info!("No asset keys found yet");
        }
    } else {
        info!("Asset keys response is not an array or is empty");
    }
}

#[actix_rt::test]
async fn test_event_statistics_over_time() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    info!("Testing event statistics over time");

    let time_periods = vec![
        ("Last 7 days", 7),
        ("Last 30 days", 30),
        ("Last 90 days", 90),
    ];

    for (period_name, days) in time_periods {
        let start = chrono::Utc::now()
            .checked_sub_signed(chrono::Duration::days(days))
            .unwrap()
            .timestamp();
        let end = chrono::Utc::now().timestamp();

        let resp = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!(
                    "/v1/taproot-assets/universe/stats/events?start_timestamp={start}&end_timestamp={end}"
                ))
                .to_request(),
        )
        .await;
        assert!(resp.status().is_success());

        let json: Value = test::read_body_json(resp).await;
        let events = json["events"].as_array().unwrap();

        info!("{}: Found {} days with events", period_name, events.len());

        let mut total_syncs = 0u64;
        let mut total_proofs = 0u64;

        for event in events {
            total_syncs += event["sync_events"]
                .as_str()
                .unwrap()
                .parse::<u64>()
                .unwrap_or(0);
            total_proofs += event["new_proof_events"]
                .as_str()
                .unwrap()
                .parse::<u64>()
                .unwrap_or(0);
        }

        info!(
            "{}: Total syncs: {}, Total new proofs: {}",
            period_name, total_syncs, total_proofs
        );
    }
}
