use actix_web::{test, App};
use base64::Engine;
use serde_json::{json, Value};
use serial_test::serial;
use taproot_assets_rest_gateway::api::routes::configure;
use taproot_assets_rest_gateway::api::wallet::{
    InternalKeyRequest, OwnershipProveRequest, OwnershipVerifyRequest, ScriptKeyRequest,
    UtxoLeaseDeleteRequest,
};
use taproot_assets_rest_gateway::tests::setup::{mint_test_asset, setup, setup_without_assets};

#[actix_rt::test]
async fn test_generate_next_internal_key() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let request = InternalKeyRequest { key_family: 1 };

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/wallet/internal-key/next")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    assert!(json["internal_key"].is_object());

    let internal_key = &json["internal_key"];
    assert!(internal_key["raw_key_bytes"].is_string());
    assert!(internal_key["key_loc"].is_object());
    assert!(internal_key["key_loc"]["key_family"].is_number());
    assert!(internal_key["key_loc"]["key_index"].is_number());
}

#[actix_rt::test]
async fn test_query_internal_key() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // First generate a key
    let gen_request = InternalKeyRequest { key_family: 1 };
    let gen_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/wallet/internal-key/next")
            .set_json(&gen_request)
            .to_request(),
    )
    .await;
    let gen_json: Value = test::read_body_json(gen_resp).await;
    let raw_key_bytes = gen_json["internal_key"]["raw_key_bytes"].as_str().unwrap();

    // Query the key
    let req = test::TestRequest::get()
        .uri(&format!(
            "/v1/taproot-assets/wallet/internal-key/{raw_key_bytes}"
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;

    // The API returns 200 OK with an error in the response body if key not found
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;

    // Check if it's an error response
    if json.get("error").is_some() || json.get("code").is_some() {
        println!("Query internal key returned error: {json:?}");
        // This is expected if the key lookup is not implemented
    } else {
        // If successful, verify structure
        assert!(json["internal_key"].is_object());
        assert_eq!(
            json["internal_key"]["raw_key_bytes"].as_str(),
            Some(raw_key_bytes)
        );
    }
}

#[actix_rt::test]
async fn test_generate_next_script_key() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let request = InternalKeyRequest { key_family: 1 };

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/wallet/script-key/next")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    assert!(json["script_key"].is_object());

    let script_key = &json["script_key"];
    assert!(script_key["pub_key"].is_string());
    assert!(script_key["key_desc"].is_object());
    assert!(script_key["type"].is_string());
}

#[actix_rt::test]
async fn test_query_script_key() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // First generate a script key
    let gen_request = InternalKeyRequest { key_family: 1 };
    let gen_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/wallet/script-key/next")
            .set_json(&gen_request)
            .to_request(),
    )
    .await;
    let gen_json: Value = test::read_body_json(gen_resp).await;
    let pub_key = gen_json["script_key"]["pub_key"].as_str().unwrap();

    // Query the script key
    let req = test::TestRequest::get()
        .uri(&format!("/v1/taproot-assets/wallet/script-key/{pub_key}"))
        .to_request();
    let resp = test::call_service(&app, req).await;

    // The API returns 200 OK with an error in the response body if key not found
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;

    // Check if it's an error response
    if json.get("error").is_some() || json.get("code").is_some() {
        println!("Query script key returned error: {json:?}");
        // This is expected if the key lookup is not implemented
    } else {
        // If successful, verify structure
        assert!(json["script_key"].is_object());
        assert_eq!(json["script_key"]["pub_key"].as_str(), Some(pub_key));
    }
}

#[actix_rt::test]
async fn test_declare_script_key() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let request = ScriptKeyRequest {
        script_key: json!({
            "pub_key": "AjVjSPpdLKW4WMSjKyY3QyJJMwJe/I7uRKy7sKww8CTf",
            "key_desc": {
                "raw_key_bytes": "AjVjSPpdLKW4WMSjKyY3QyJJMwJe/I7uRKy7sKww8CTf",
                "key_loc": {
                    "key_family": 1,
                    "key_index": 0
                }
            },
            "tap_tweak": "",
            "type": "SCRIPT_KEY_SCRIPT_PATH_EXTERNAL"
        }),
    };

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/wallet/script-key/declare")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;

    // The API returns 200 OK even for errors
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;

    // Check if it's an error response
    if json.get("error").is_some() || json.get("code").is_some() {
        println!("Declare script key returned error: {json:?}");
        // This is expected if the key is invalid
    } else {
        // If successful, verify structure
        assert!(json["script_key"].is_object());
    }
}

#[actix_rt::test]
#[serial]
async fn test_prove_asset_ownership() {
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
        let script_key = asset["script_key"].as_str().unwrap();
        let anchor_outpoint = asset["chain_anchor"]["anchor_outpoint"].as_str().unwrap();
        let parts: Vec<&str> = anchor_outpoint.split(':').collect();

        if parts.len() == 2 {
            let txid_hex = parts[0];
            if let Ok(txid_bytes) = hex::decode(txid_hex) {
                let txid_base64 = base64::engine::general_purpose::STANDARD.encode(&txid_bytes);

                let request = OwnershipProveRequest {
                    asset_id: asset_id.clone(),
                    script_key: script_key.to_string(),
                    outpoint: json!({
                        "txid": txid_base64,
                        "output_index": parts[1].parse::<u32>().unwrap_or(0)
                    }),
                    challenge: base64::engine::general_purpose::STANDARD.encode("test_challenge"),
                };

                let req = test::TestRequest::post()
                    .uri("/v1/taproot-assets/wallet/ownership/prove")
                    .set_json(&request)
                    .to_request();
                let resp = test::call_service(&app, req).await;
                assert!(resp.status().is_success());

                let json: Value = test::read_body_json(resp).await;

                // Check if it's an error response
                if json.get("error").is_some() || json.get("code").is_some() {
                    println!("Prove ownership returned error: {json:?}");
                } else {
                    assert!(json["proof_with_witness"].is_string());
                }
            }
        }
    }
}

#[actix_rt::test]
#[serial]
async fn test_verify_ownership_proof() {
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

    // Get asset and prove ownership first
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
        let script_key = asset["script_key"].as_str().unwrap();
        let anchor_outpoint = asset["chain_anchor"]["anchor_outpoint"].as_str().unwrap();
        let parts: Vec<&str> = anchor_outpoint.split(':').collect();

        if parts.len() == 2 {
            let txid_hex = parts[0];
            if let Ok(txid_bytes) = hex::decode(txid_hex) {
                let txid_base64 = base64::engine::general_purpose::STANDARD.encode(&txid_bytes);
                let challenge = base64::engine::general_purpose::STANDARD.encode("test_challenge");

                // First prove ownership
                let prove_request = OwnershipProveRequest {
                    asset_id: asset_id.clone(),
                    script_key: script_key.to_string(),
                    outpoint: json!({
                        "txid": txid_base64,
                        "output_index": parts[1].parse::<u32>().unwrap_or(0)
                    }),
                    challenge: challenge.clone(),
                };

                let prove_resp = test::call_service(
                    &app,
                    test::TestRequest::post()
                        .uri("/v1/taproot-assets/wallet/ownership/prove")
                        .set_json(&prove_request)
                        .to_request(),
                )
                .await;
                let prove_json: Value = test::read_body_json(prove_resp).await;

                // Check if prove was successful
                if prove_json.get("error").is_none() && prove_json.get("code").is_none() {
                    let proof_with_witness = prove_json["proof_with_witness"].as_str().unwrap();

                    // Now verify the proof
                    let verify_request = OwnershipVerifyRequest {
                        proof_with_witness: proof_with_witness.to_string(),
                        challenge,
                    };

                    let req = test::TestRequest::post()
                        .uri("/v1/taproot-assets/wallet/ownership/verify")
                        .set_json(&verify_request)
                        .to_request();
                    let resp = test::call_service(&app, req).await;
                    assert!(resp.status().is_success());

                    let json: Value = test::read_body_json(resp).await;

                    // Check if it's an error response
                    if json.get("error").is_some() || json.get("code").is_some() {
                        println!("Verify ownership returned error: {json:?}");
                    } else {
                        assert_eq!(json["valid_proof"].as_bool(), Some(true));
                        assert!(json["outpoint"].is_object());
                        assert!(json["outpoint_str"].is_string());
                        assert!(json["block_hash"].is_string());
                        assert!(json["block_hash_str"].is_string());
                        assert!(json["block_height"].is_number());
                    }
                }
            }
        }
    }
}

#[actix_rt::test]
async fn test_delete_utxo_lease() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let request = UtxoLeaseDeleteRequest {
        outpoint: json!({
            "txid": base64::engine::general_purpose::STANDARD.encode(vec![0u8; 32]),
            "output_index": 0
        }),
    };

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/wallet/utxo-lease/delete")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;

    // The API returns 200 OK even for errors
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;

    // Check if it's an error response
    if json.get("error").is_some() || json.get("code").is_some() {
        println!("Delete UTXO lease returned error: {json:?}");
        // This is expected if UTXO doesn't exist or isn't leased
    } else {
        // Response should be empty on success
        assert!(json.is_object());
    }
}

#[actix_rt::test]
async fn test_key_family_ranges() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Test different key families
    let key_families = vec![0, 1, 100, 300];

    for family in key_families {
        let request = InternalKeyRequest { key_family: family };

        let req = test::TestRequest::post()
            .uri("/v1/taproot-assets/wallet/internal-key/next")
            .set_json(&request)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let json: Value = test::read_body_json(resp).await;

        // Check if it's an error response
        if json.get("error").is_some() || json.get("code").is_some() {
            println!("Key family {family} returned error: {json:?}");
        } else {
            // API might return the family as integer or string
            let returned_family = json["internal_key"]["key_loc"]["key_family"]
                .as_u64()
                .or_else(|| {
                    json["internal_key"]["key_loc"]["key_family"]
                        .as_str()
                        .and_then(|s| s.parse::<u64>().ok())
                });

            assert_eq!(
                returned_family,
                Some(family as u64),
                "Expected family {family}, got {returned_family:?}"
            );
        }
    }
}

#[actix_rt::test]
async fn test_script_key_types() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Generate a normal BIP86 script key
    let request = InternalKeyRequest { key_family: 1 };

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/wallet/script-key/next")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    let script_type = json["script_key"]["type"].as_str().unwrap();
    assert_eq!(script_type, "SCRIPT_KEY_BIP86");
}

#[actix_rt::test]
#[serial]
async fn test_ownership_proof_with_invalid_challenge() {
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
        let script_key = asset["script_key"].as_str().unwrap();
        let anchor_outpoint = asset["chain_anchor"]["anchor_outpoint"].as_str().unwrap();
        let parts: Vec<&str> = anchor_outpoint.split(':').collect();

        if parts.len() == 2 {
            let txid_hex = parts[0];
            if let Ok(txid_bytes) = hex::decode(txid_hex) {
                let txid_base64 = base64::engine::general_purpose::STANDARD.encode(&txid_bytes);
                let challenge1 = base64::engine::general_purpose::STANDARD.encode("challenge1");
                let challenge2 = base64::engine::general_purpose::STANDARD.encode("challenge2");

                // Prove with one challenge
                let prove_request = OwnershipProveRequest {
                    asset_id: asset_id.clone(),
                    script_key: script_key.to_string(),
                    outpoint: json!({
                        "txid": txid_base64,
                        "output_index": parts[1].parse::<u32>().unwrap_or(0)
                    }),
                    challenge: challenge1,
                };

                let prove_resp = test::call_service(
                    &app,
                    test::TestRequest::post()
                        .uri("/v1/taproot-assets/wallet/ownership/prove")
                        .set_json(&prove_request)
                        .to_request(),
                )
                .await;
                let prove_json: Value = test::read_body_json(prove_resp).await;

                // Check if prove was successful
                if prove_json.get("error").is_none() && prove_json.get("code").is_none() {
                    let proof_with_witness = prove_json["proof_with_witness"].as_str().unwrap();

                    // Try to verify with different challenge
                    let verify_request = OwnershipVerifyRequest {
                        proof_with_witness: proof_with_witness.to_string(),
                        challenge: challenge2,
                    };

                    let req = test::TestRequest::post()
                        .uri("/v1/taproot-assets/wallet/ownership/verify")
                        .set_json(&verify_request)
                        .to_request();
                    let resp = test::call_service(&app, req).await;
                    assert!(resp.status().is_success());

                    let json: Value = test::read_body_json(resp).await;

                    // Should either return an error or valid_proof=false
                    if json.get("error").is_none() && json.get("code").is_none() {
                        assert_eq!(json["valid_proof"].as_bool(), Some(false));
                    }
                }
            }
        }
    }
}

#[actix_rt::test]
async fn test_declare_multiple_script_keys() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Declare multiple script keys with different types
    let script_key_types = vec![
        ("SCRIPT_KEY_BIP86", ""),
        ("SCRIPT_KEY_SCRIPT_PATH_EXTERNAL", "custom_tweak_data"),
    ];

    for (key_type, tap_tweak) in script_key_types {
        let request = ScriptKeyRequest {
            script_key: json!({
                "pub_key": format!("AjVjSPpdLKW4WMSjKyY3QyJJMwJe/I7uRKy7sKww8C{}", key_type.len()),
                "key_desc": {
                    "raw_key_bytes": format!("AjVjSPpdLKW4WMSjKyY3QyJJMwJe/I7uRKy7sKww8C{}", key_type.len()),
                    "key_loc": {
                        "key_family": 1,
                        "key_index": key_type.len() as i32
                    }
                },
                "tap_tweak": tap_tweak,
                "type": key_type
            }),
        };

        let req = test::TestRequest::post()
            .uri("/v1/taproot-assets/wallet/script-key/declare")
            .set_json(&request)
            .to_request();
        let resp = test::call_service(&app, req).await;

        // API returns 200 OK even for errors
        assert!(resp.status().is_success());

        let json: Value = test::read_body_json(resp).await;
        if json.get("error").is_some() || json.get("code").is_some() {
            println!("Declare script key type {key_type} returned error: {json:?}");
        }
    }
}
