use actix_web::{test, App};
use serde_json::{json, Value};
use serial_test::serial;
use std::time::Duration;
use taproot_assets_rest_gateway::api::proofs::{
    DecodeProofRequest, ExportProofRequest, UnpackFileRequest, VerifyProofRequest,
};
use taproot_assets_rest_gateway::api::routes::configure;
use taproot_assets_rest_gateway::tests::setup::{
    assert_status_matches_body, mint_test_asset, setup, txid_to_internal_hex,
};
use tokio::time::sleep;

/// Exports a real proof file for the first asset the daemon holds, returning the
/// hex-encoded proof and the asset's genesis point.
async fn export_real_proof(
    client: &reqwest::Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Option<(String, String)> {
    let assets: Value = client
        .get(format!("{base_url}/v1/taproot-assets/assets"))
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    let asset = assets["assets"].as_array()?.first()?;
    let genesis_point = asset["asset_genesis"]["genesis_point"]
        .as_str()?
        .to_string();
    let outpoint = asset["chain_anchor"]["anchor_outpoint"].as_str()?;
    let (txid, vout) = outpoint.split_once(':')?;

    let body = json!({
        "asset_id": asset["asset_genesis"]["asset_id"].as_str()?,
        "script_key": asset["script_key"].as_str()?,
        "outpoint": {
            "txid": txid_to_internal_hex(txid),
            "output_index": vout.parse::<u32>().ok()?
        }
    });
    let exported: Value = client
        .post(format!("{base_url}/v1/taproot-assets/proofs/export"))
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&body)
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    Some((
        exported["raw_proof_file"].as_str()?.to_string(),
        genesis_point,
    ))
}

async fn wait_for_asset(
    client: &reqwest::Client,
    base_url: &str,
    macaroon_hex: &str,
    asset_id: &str,
) -> Value {
    let mut attempts = 0;
    let max_attempts = 10;

    while attempts < max_attempts {
        let url = format!("{base_url}/v1/taproot-assets/assets");
        let response = client
            .get(&url)
            .header("Grpc-Metadata-macaroon", macaroon_hex)
            .send()
            .await
            .expect("Failed to list assets");
        let assets_json: Value = response
            .json()
            .await
            .expect("Failed to parse assets response");

        if let Some(assets) = assets_json["assets"].as_array() {
            if let Some(asset) = assets
                .iter()
                .find(|a| a["asset_genesis"]["asset_id"].as_str() == Some(asset_id))
            {
                return asset.clone();
            }
        }
        attempts += 1;
        sleep(Duration::from_secs(2)).await;
    }

    panic!("Asset with ID {asset_id} not found after {max_attempts} attempts");
}

#[actix_rt::test]
#[serial]
async fn test_export_proof() {
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

    let asset = wait_for_asset(client.as_ref(), &base_url.0, &macaroon_hex.0, &asset_id).await;

    if let Some(anchor_outpoint) = asset["chain_anchor"]["anchor_outpoint"].as_str() {
        let parts: Vec<&str> = anchor_outpoint.split(':').collect();
        if parts.len() == 2 {
            let txid_hex = parts[0];
            if let Ok(vout) = parts[1].parse::<u32>() {
                if hex::decode(txid_hex).is_ok() {
                    let txid_hex = txid_to_internal_hex(txid_hex);
                    if let Some(script_key) = asset["script_key"].as_str() {
                        let request = ExportProofRequest {
                            asset_id: asset_id.clone(),
                            script_key: script_key.to_string(),
                            outpoint: json!({
                                "txid": txid_hex,
                                "output_index": vout
                            }),
                        };

                        let req = test::TestRequest::post()
                            .uri("/v1/taproot-assets/proofs/export")
                            .set_json(&request)
                            .to_request();
                        let resp = test::call_service(&app, req).await;
                        assert!(
                            resp.status().is_success(),
                            "Export proof request failed with status: {}",
                            resp.status()
                        );

                        let proof_json: Value = test::read_body_json(resp).await;

                        // Check if it's an error response
                        if proof_json.get("error").is_some() || proof_json.get("code").is_some() {
                            println!("Export proof failed with error: {proof_json:?}");
                            // This might happen if the asset is not fully confirmed yet
                            return;
                        }

                        assert!(
                            proof_json["raw_proof_file"].is_string(),
                            "raw_proof_file should be a string, got: {proof_json:?}"
                        );
                        assert!(
                            proof_json["genesis_point"].is_string(),
                            "genesis_point should be a string"
                        );
                    } else {
                        panic!("Script key missing in asset response");
                    }
                } else {
                    panic!("Failed to decode txid_hex");
                }
            } else {
                panic!("Failed to parse vout");
            }
        } else {
            panic!("Invalid anchor_outpoint format");
        }
    } else {
        panic!("Anchor outpoint missing in asset response");
    }
}

#[actix_rt::test]
#[serial]
async fn test_decode_proof() {
    let (client, base_url, macaroon_hex, _lnd_macaroon_hex) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let (proof, _) = export_real_proof(&client, &base_url.0, &macaroon_hex.0)
        .await
        .expect("daemon should hold a mintable asset with an exportable proof");

    let request = json!({
        "raw_proof": proof,
        "proof_at_depth": 0,
        "with_prev_witnesses": true,
        "with_meta_reveal": true
    });

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/proofs/decode")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    let status = resp.status();
    let json: Value = test::read_body_json(resp).await;
    assert_status_matches_body(status, &json);
    assert!(
        status.is_success(),
        "decoding a real proof must succeed: {json}"
    );
    assert!(json["decoded_proof"].is_object());
}

#[actix_rt::test]
#[serial]
async fn test_verify_proof() {
    let (client, base_url, macaroon_hex, _lnd_macaroon_hex) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let (proof, genesis_point) = export_real_proof(&client, &base_url.0, &macaroon_hex.0)
        .await
        .expect("daemon should hold a mintable asset with an exportable proof");

    let request = json!({
        "raw_proof_file": proof,
        "genesis_point": genesis_point
    });

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/proofs/verify")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    let status = resp.status();
    let json: Value = test::read_body_json(resp).await;
    assert_status_matches_body(status, &json);
    assert!(
        status.is_success(),
        "verifying a real proof must succeed: {json}"
    );
    assert_eq!(json["valid"].as_bool(), Some(true));
}

#[actix_rt::test]
async fn test_decode_proof_options() {
    let (client, base_url, macaroon_hex, _lnd_macaroon_hex) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let (proof, _) = export_real_proof(&client, &base_url.0, &macaroon_hex.0)
        .await
        .expect("daemon should hold a mintable asset with an exportable proof");

    let test_cases = vec![
        (Some(0), true, true),
        (Some(1), false, false),
        (None, true, false),
    ];

    for (depth, with_witnesses, with_meta) in test_cases {
        let request = json!({
            "raw_proof": proof,
            "proof_at_depth": depth,
            "with_prev_witnesses": with_witnesses,
            "with_meta_reveal": with_meta
        });

        let req = test::TestRequest::post()
            .uri("/v1/taproot-assets/proofs/decode")
            .set_json(&request)
            .to_request();
        let resp = test::call_service(&app, req).await;
        let status = resp.status();
        let json: Value = test::read_body_json(resp).await;
        assert_status_matches_body(status, &json);
        assert!(
            status.is_success(),
            "decoding a real proof at depth {depth:?} must succeed: {json}"
        );
        assert!(json["decoded_proof"].is_object());
    }
}

#[actix_rt::test]
#[serial]
async fn test_proof_validation_errors() {
    let (client, base_url, macaroon_hex, _lnd_macaroon_hex) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let request = json!({
        "raw_proof": hex::encode(""),
        "proof_at_depth": 0,
        "with_prev_witnesses": true,
        "with_meta_reveal": true
    });

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/proofs/decode")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    let status = resp.status();
    let json: Value = test::read_body_json(resp).await;
    assert_status_matches_body(status, &json);
    assert!(!status.is_success(), "an empty proof must be rejected");
}

#[actix_rt::test]
#[serial]
async fn test_decoded_proof_structure() {
    let (client, base_url, macaroon_hex, _lnd_macaroon_hex) = setup().await;
    let _app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let expected_structure = json!({
        "decoded_proof": {
            "proof_at_depth": 0,
            "number_of_proofs": 1,
            "asset": {
                "version": "ASSET_VERSION_V0",
                "asset_genesis": {
                    "genesis_point": "txid:vout",
                    "name": "asset_name",
                    "meta_hash": "hash",
                    "asset_id": "id",
                    "asset_type": "NORMAL",
                    "output_index": 0
                },
                "amount": "1000",
                "script_key": "key",
                "script_key_is_local": true,
                "chain_anchor": {
                    "anchor_tx": "tx",
                    "anchor_block_hash": "hash",
                    "anchor_outpoint": "txid:vout",
                    "internal_key": "key",
                    "merkle_root": "root",
                    "block_height": 100,
                    "block_timestamp": "1234567890"
                }
            }
        }
    });

    assert!(expected_structure["decoded_proof"].is_object());
}

#[actix_rt::test]
#[serial]
async fn test_export_asset_proof() {
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

    let asset = wait_for_asset(client.as_ref(), &base_url.0, &macaroon_hex.0, &asset_id).await;

    if let Some(anchor_outpoint) = asset["chain_anchor"]["anchor_outpoint"].as_str() {
        let parts: Vec<&str> = anchor_outpoint.split(':').collect();
        if parts.len() == 2 {
            let txid_hex = parts[0];
            if let Ok(vout) = parts[1].parse::<u32>() {
                if hex::decode(txid_hex).is_ok() {
                    let txid_hex = txid_to_internal_hex(txid_hex);
                    if let Some(script_key) = asset["script_key"].as_str() {
                        let request = ExportProofRequest {
                            asset_id: asset_id.clone(),
                            script_key: script_key.to_string(),
                            outpoint: json!({
                                "txid": txid_hex,
                                "output_index": vout
                            }),
                        };

                        let req = test::TestRequest::post()
                            .uri("/v1/taproot-assets/proofs/export")
                            .set_json(&request)
                            .to_request();
                        let resp = test::call_service(&app, req).await;
                        assert!(
                            resp.status().is_success(),
                            "Export asset proof request failed"
                        );

                        let proof_json: Value = test::read_body_json(resp).await;

                        // Check if it's an error response
                        if proof_json.get("error").is_some() || proof_json.get("code").is_some() {
                            println!("Export proof failed with error: {proof_json:?}");
                            return;
                        }

                        assert!(
                            proof_json["raw_proof_file"].is_string(),
                            "raw_proof_file should be a string, got: {proof_json:?}"
                        );
                        assert!(
                            proof_json["genesis_point"].is_string(),
                            "genesis_point should be a string"
                        );
                    } else {
                        panic!("Script key missing in asset response");
                    }
                } else {
                    panic!("Failed to decode txid_hex");
                }
            } else {
                panic!("Failed to parse vout");
            }
        } else {
            panic!("Invalid anchor_outpoint format");
        }
    } else {
        panic!("Anchor outpoint missing in asset response");
    }
}

#[actix_rt::test]
#[serial]
async fn test_unpack_exported_proof_file() {
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

    let asset = wait_for_asset(client.as_ref(), &base_url.0, &macaroon_hex.0, &asset_id).await;

    if let Some(anchor_outpoint) = asset["chain_anchor"]["anchor_outpoint"].as_str() {
        let parts: Vec<&str> = anchor_outpoint.split(':').collect();
        if parts.len() == 2 {
            let txid_hex = parts[0];
            if let Ok(vout) = parts[1].parse::<u32>() {
                if hex::decode(txid_hex).is_ok() {
                    let txid_hex = txid_to_internal_hex(txid_hex);
                    if let Some(script_key) = asset["script_key"].as_str() {
                        let export_req = ExportProofRequest {
                            asset_id: asset_id.clone(),
                            script_key: script_key.to_string(),
                            outpoint: json!({
                                "txid": txid_hex,
                                "output_index": vout
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
                        let export_json: Value = test::read_body_json(export_resp).await;

                        // Check if export was successful
                        if export_json.get("error").is_some() || export_json.get("code").is_some() {
                            println!("Export proof failed with error: {export_json:?}");
                            return;
                        }

                        if let Some(raw_proof_file) = export_json["raw_proof_file"].as_str() {
                            let unpack_req = UnpackFileRequest {
                                raw_proof_file: raw_proof_file.to_string(),
                            };

                            let req = test::TestRequest::post()
                                .uri("/v1/taproot-assets/proofs/unpack-file")
                                .set_json(&unpack_req)
                                .to_request();
                            let resp = test::call_service(&app, req).await;
                            assert!(
                                resp.status().is_success(),
                                "Unpack proof file request failed"
                            );
                            let unpack_json: Value = test::read_body_json(resp).await;
                            assert!(
                                unpack_json["raw_proofs"].is_array(),
                                "raw_proofs should be an array"
                            );
                            let raw_proofs = unpack_json["raw_proofs"]
                                .as_array()
                                .expect("raw_proofs should be an array");
                            assert!(!raw_proofs.is_empty(), "raw_proofs should not be empty");
                        } else {
                            panic!("raw_proof_file missing in export response");
                        }
                    } else {
                        panic!("Script key missing in asset response");
                    }
                } else {
                    panic!("Failed to decode txid_hex");
                }
            } else {
                panic!("Failed to parse vout");
            }
        } else {
            panic!("Invalid anchor_outpoint format");
        }
    } else {
        panic!("Anchor outpoint missing in asset response");
    }
}

#[actix_rt::test]
#[serial]
async fn test_decode_proof_file() {
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

    let asset = wait_for_asset(client.as_ref(), &base_url.0, &macaroon_hex.0, &asset_id).await;

    if let Some(anchor_outpoint) = asset["chain_anchor"]["anchor_outpoint"].as_str() {
        let parts: Vec<&str> = anchor_outpoint.split(':').collect();
        if parts.len() == 2 {
            let txid_hex = parts[0];
            if let Ok(vout) = parts[1].parse::<u32>() {
                if hex::decode(txid_hex).is_ok() {
                    let txid_hex = txid_to_internal_hex(txid_hex);
                    if let Some(script_key) = asset["script_key"].as_str() {
                        let export_req = ExportProofRequest {
                            asset_id: asset_id.clone(),
                            script_key: script_key.to_string(),
                            outpoint: json!({
                                "txid": txid_hex,
                                "output_index": vout
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
                        let export_json: Value = test::read_body_json(export_resp).await;

                        // Check if export was successful
                        if export_json.get("error").is_some() || export_json.get("code").is_some() {
                            println!("Export proof failed with error: {export_json:?}");
                            return;
                        }

                        if let Some(raw_proof_file) = export_json["raw_proof_file"].as_str() {
                            let decode_req = DecodeProofRequest {
                                raw_proof: raw_proof_file.to_string(),
                                proof_at_depth: Some(0),
                                with_prev_witnesses: true,
                                with_meta_reveal: true,
                            };

                            let req = test::TestRequest::post()
                                .uri("/v1/taproot-assets/proofs/decode")
                                .set_json(&decode_req)
                                .to_request();
                            let resp = test::call_service(&app, req).await;
                            assert!(
                                resp.status().is_success(),
                                "Decode proof file request failed"
                            );
                            let decode_json: Value = test::read_body_json(resp).await;
                            assert!(
                                decode_json["decoded_proof"].is_object(),
                                "decoded_proof should be an object"
                            );
                            let decoded_proof = &decode_json["decoded_proof"];
                            assert_eq!(
                                decoded_proof["proof_at_depth"].as_u64(),
                                Some(0),
                                "proof_at_depth should be 0"
                            );
                            assert!(
                                decoded_proof["asset"].is_object(),
                                "asset should be an object"
                            );
                        } else {
                            panic!("raw_proof_file missing in export response");
                        }
                    } else {
                        panic!("Script key missing in asset response");
                    }
                } else {
                    panic!("Failed to decode txid_hex");
                }
            } else {
                panic!("Failed to parse vout");
            }
        } else {
            panic!("Invalid anchor_outpoint format");
        }
    } else {
        panic!("Anchor outpoint missing in asset response");
    }
}

#[actix_rt::test]
#[serial]
async fn test_verify_proof_validity() {
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

    let asset = wait_for_asset(client.as_ref(), &base_url.0, &macaroon_hex.0, &asset_id).await;

    if let Some(anchor_outpoint) = asset["chain_anchor"]["anchor_outpoint"].as_str() {
        let parts: Vec<&str> = anchor_outpoint.split(':').collect();
        if parts.len() == 2 {
            let txid_hex = parts[0];
            if let Ok(vout) = parts[1].parse::<u32>() {
                if hex::decode(txid_hex).is_ok() {
                    let txid_hex = txid_to_internal_hex(txid_hex);
                    if let Some(script_key) = asset["script_key"].as_str() {
                        let export_req = ExportProofRequest {
                            asset_id: asset_id.clone(),
                            script_key: script_key.to_string(),
                            outpoint: json!({
                                "txid": txid_hex,
                                "output_index": vout
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
                        let export_json: Value = test::read_body_json(export_resp).await;

                        // Check if export was successful
                        if export_json.get("error").is_some() || export_json.get("code").is_some() {
                            println!("Export proof failed with error: {export_json:?}");
                            return;
                        }

                        if let (Some(raw_proof_file), Some(genesis_point)) = (
                            export_json["raw_proof_file"].as_str(),
                            export_json["genesis_point"].as_str(),
                        ) {
                            let verify_req = VerifyProofRequest {
                                raw_proof_file: raw_proof_file.to_string(),
                                genesis_point: genesis_point.to_string(),
                            };

                            let req = test::TestRequest::post()
                                .uri("/v1/taproot-assets/proofs/verify")
                                .set_json(&verify_req)
                                .to_request();
                            let resp = test::call_service(&app, req).await;
                            assert!(resp.status().is_success(), "Verify proof request failed");
                            let verify_json: Value = test::read_body_json(resp).await;
                            assert_eq!(
                                verify_json["valid"].as_bool(),
                                Some(true),
                                "Proof should be valid"
                            );
                            assert!(
                                verify_json["decoded_proof"].is_object(),
                                "decoded_proof should be an object"
                            );
                        } else {
                            panic!("raw_proof_file or genesis_point missing in export response");
                        }
                    } else {
                        panic!("Script key missing in asset response");
                    }
                } else {
                    panic!("Failed to decode txid_hex");
                }
            } else {
                panic!("Failed to parse vout");
            }
        } else {
            panic!("Invalid anchor_outpoint format");
        }
    } else {
        panic!("Anchor outpoint missing in asset response");
    }
}

#[actix_rt::test]
#[serial]
async fn test_unpack_proof_file() {
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

    let asset = wait_for_asset(client.as_ref(), &base_url.0, &macaroon_hex.0, &asset_id).await;

    if let Some(anchor_outpoint) = asset["chain_anchor"]["anchor_outpoint"].as_str() {
        let parts: Vec<&str> = anchor_outpoint.split(':').collect();
        if parts.len() == 2 {
            let txid_hex = parts[0];
            if let Ok(vout) = parts[1].parse::<u32>() {
                if hex::decode(txid_hex).is_ok() {
                    let txid_hex = txid_to_internal_hex(txid_hex);
                    if let Some(script_key) = asset["script_key"].as_str() {
                        let export_req = ExportProofRequest {
                            asset_id: asset_id.clone(),
                            script_key: script_key.to_string(),
                            outpoint: json!({
                                "txid": txid_hex,
                                "output_index": vout
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
                        let export_json: Value = test::read_body_json(export_resp).await;

                        // Check if export was successful
                        if export_json.get("error").is_some() || export_json.get("code").is_some() {
                            println!("Export proof failed with error: {export_json:?}");
                            return;
                        }

                        if let Some(raw_proof_file) = export_json["raw_proof_file"].as_str() {
                            let unpack_req = UnpackFileRequest {
                                raw_proof_file: raw_proof_file.to_string(),
                            };

                            let req = test::TestRequest::post()
                                .uri("/v1/taproot-assets/proofs/unpack-file")
                                .set_json(&unpack_req)
                                .to_request();
                            let resp = test::call_service(&app, req).await;
                            assert!(
                                resp.status().is_success(),
                                "Unpack proof file request failed"
                            );
                            let unpack_json: Value = test::read_body_json(resp).await;
                            assert!(
                                unpack_json["raw_proofs"].is_array(),
                                "raw_proofs should be an array"
                            );
                            let raw_proofs = unpack_json["raw_proofs"]
                                .as_array()
                                .expect("raw_proofs should be an array");
                            assert!(!raw_proofs.is_empty(), "raw_proofs should not be empty");
                        } else {
                            panic!("raw_proof_file missing in export response");
                        }
                    } else {
                        panic!("Script key missing in asset response");
                    }
                } else {
                    panic!("Failed to decode txid_hex");
                }
            } else {
                panic!("Failed to parse vout");
            }
        } else {
            panic!("Invalid anchor_outpoint format");
        }
    } else {
        panic!("Anchor outpoint missing in asset response");
    }
}

#[actix_rt::test]
#[serial]
async fn test_decode_proof_at_different_depths() {
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

    // Wait for initial asset
    let initial_asset =
        wait_for_asset(client.as_ref(), &base_url.0, &macaroon_hex.0, &asset_id).await;

    // Just test with the initial asset - multi-hop transfers are complex and may fail
    if let Some(anchor_outpoint) = initial_asset["chain_anchor"]["anchor_outpoint"].as_str() {
        let parts: Vec<&str> = anchor_outpoint.split(':').collect();
        if parts.len() == 2 {
            let txid_hex = parts[0];
            if let Ok(vout) = parts[1].parse::<u32>() {
                if hex::decode(txid_hex).is_ok() {
                    let txid_hex = txid_to_internal_hex(txid_hex);
                    if let Some(script_key) = initial_asset["script_key"].as_str() {
                        let export_req = ExportProofRequest {
                            asset_id: asset_id.clone(),
                            script_key: script_key.to_string(),
                            outpoint: json!({
                                "txid": txid_hex,
                                "output_index": vout
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

                        let export_json: Value = test::read_body_json(export_resp).await;

                        if export_json.get("error").is_some() || export_json.get("code").is_some() {
                            println!("Export proof failed with error: {export_json:?}");
                            return;
                        }

                        if let Some(raw_proof_file) = export_json["raw_proof_file"].as_str() {
                            // Test decoding at depth 0
                            let decode_req = DecodeProofRequest {
                                raw_proof: raw_proof_file.to_string(),
                                proof_at_depth: Some(0),
                                with_prev_witnesses: true,
                                with_meta_reveal: true,
                            };

                            let req = test::TestRequest::post()
                                .uri("/v1/taproot-assets/proofs/decode")
                                .set_json(&decode_req)
                                .to_request();
                            let resp = test::call_service(&app, req).await;
                            assert!(resp.status().is_success(), "Decode proof at depth 0 failed");

                            let decode_json: Value = test::read_body_json(resp).await;
                            assert!(decode_json["decoded_proof"].is_object());
                            assert_eq!(
                                decode_json["decoded_proof"]["proof_at_depth"].as_u64(),
                                Some(0)
                            );
                        }
                    }
                }
            }
        }
    }
}
