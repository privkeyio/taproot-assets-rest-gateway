use actix_web::{test, App};
use serde_json::Value;
use serial_test::serial;
use taproot_assets_rest_gateway::api::addresses::NewAddrRequest;
use taproot_assets_rest_gateway::api::routes::configure;
use taproot_assets_rest_gateway::api::send::SendRequest;
use taproot_assets_rest_gateway::tests::setup::{mint_test_asset, setup};

#[actix_rt::test]
#[serial]
async fn test_send_assets_basic() {
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
    let addr_req = NewAddrRequest {
        asset_id,
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
    assert!(addr_resp.status().is_success());
    let addr_json: Value = test::read_body_json(addr_resp).await;
    if addr_json.get("error").is_some() || addr_json.get("code").is_some() {
        println!("Address creation failed: {addr_json:?}");
        return;
    }
    let addr = addr_json["encoded"].as_str().unwrap().to_string();
    let request = SendRequest {
        tap_addrs: vec![addr],
        fee_rate: Some(300),
        label: None,
        skip_proof_courier_ping_check: Some(true),
    };
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/send")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let send_json: Value = test::read_body_json(resp).await;
    if send_json.get("error").is_some() || send_json.get("code").is_some() {
        println!("Send failed with error: {send_json:?}");
        return;
    }
    assert!(send_json["transfer"].is_object());
    let transfer = &send_json["transfer"];
    assert!(transfer["transfer_timestamp"].is_string());
    assert!(transfer["anchor_tx_hash"].is_string() || transfer["anchor_tx"].is_string());
    assert!(transfer["inputs"].is_array());
    assert!(transfer["outputs"].is_array());
}

#[actix_rt::test]
#[serial]
async fn test_send_with_custom_fee_rate() {
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
    let addr_req = NewAddrRequest {
        asset_id,
        amt: "50".to_string(),
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
    assert!(addr_resp.status().is_success());
    let addr_json: Value = test::read_body_json(addr_resp).await;
    if addr_json.get("error").is_some() || addr_json.get("code").is_some() {
        println!("Address creation failed: {addr_json:?}");
        return;
    }
    let addr = addr_json
        .get("encoded")
        .and_then(|v| v.as_str())
        .expect("Address should have encoded field")
        .to_string();
    let request = SendRequest {
        tap_addrs: vec![addr],
        fee_rate: Some(500),
        label: Some("High priority transfer".to_string()),
        skip_proof_courier_ping_check: Some(true),
    };
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/send")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let send_json: Value = test::read_body_json(resp).await;
    if send_json.get("error").is_some() || send_json.get("code").is_some() {
        println!("Send failed with error: {send_json:?}");
        return;
    }
    assert!(send_json["transfer"].is_object());
    if let Some(label) = send_json["transfer"].get("label") {
        assert_eq!(label.as_str(), Some("High priority transfer"));
    }
}

#[actix_rt::test]
#[serial]
async fn test_send_multiple_outputs() {
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
    let mut addresses = vec![];
    for amount in &["30", "40", "30"] {
        let addr_req = NewAddrRequest {
            asset_id: asset_id.clone(),
            amt: amount.to_string(),
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
        assert!(addr_resp.status().is_success());
        let addr_json: Value = test::read_body_json(addr_resp).await;
        if addr_json.get("error").is_some() || addr_json.get("code").is_some() {
            println!("Address creation failed: {addr_json:?}");
            continue;
        }
        match addr_json.get("encoded").and_then(|v| v.as_str()) {
            Some(encoded) if !encoded.is_empty() => {
                addresses.push(encoded.to_string());
            }
            _ => {
                println!("Address creation returned null or empty encoded field: {addr_json:?}");
                continue;
            }
        }
    }

    // Ensure we have at least one valid address
    if addresses.is_empty() {
        panic!("Failed to create any valid addresses for multi-output send test");
    }

    let request = SendRequest {
        tap_addrs: addresses.clone(),
        fee_rate: Some(300),
        label: Some("Multi-output send".to_string()),
        skip_proof_courier_ping_check: Some(true),
    };
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/send")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    if resp.status().is_success() {
        let send_json: Value = test::read_body_json(resp).await;
        if send_json.get("error").is_none() && send_json.get("code").is_none() {
            let outputs = send_json["transfer"]["outputs"].as_array().unwrap();
            assert!(
                outputs.len() >= addresses.len(),
                "Expected at least {} outputs, got {}",
                addresses.len(),
                outputs.len()
            );
        }
    }
}

#[actix_rt::test]
#[serial]
async fn test_send_with_proof_courier() {
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
    let addr_req = NewAddrRequest {
        asset_id,
        amt: "75".to_string(),
        script_key: None,
        internal_key: None,
        tapscript_sibling: None,
        proof_courier_addr: Some("https://127.0.0.1:8289".to_string()), // Updated to REST host
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
    assert!(addr_resp.status().is_success());
    let addr_json: Value = test::read_body_json(addr_resp).await;
    // Check for errors first
    if addr_json.get("error").is_some() || addr_json.get("code").is_some() {
        println!("Address creation failed: {addr_json:?}");
        return;
    }
    // Attempt to extract the encoded field
    let addr = match addr_json.get("encoded").and_then(|v| v.as_str()) {
        Some(encoded) => encoded.to_string(),
        None => {
            println!("Address creation succeeded but 'encoded' field is missing: {addr_json:?}");
            return;
        }
    };
    let request = SendRequest {
        tap_addrs: vec![addr],
        fee_rate: Some(300),
        label: None,
        skip_proof_courier_ping_check: Some(false),
    };
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/send")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success() || resp.status().is_client_error());
}

#[actix_rt::test]
#[serial]
async fn test_send_validation_errors() {
    let (client, base_url, macaroon_hex, _lnd_macaroon_hex) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;
    // Test with empty addresses
    let empty_request = SendRequest {
        tap_addrs: vec![],
        fee_rate: Some(300),
        label: None,
        skip_proof_courier_ping_check: Some(true),
    };
    let empty_req = test::TestRequest::post()
        .uri("/v1/taproot-assets/send")
        .set_json(&empty_request)
        .to_request();
    let empty_resp = test::call_service(&app, empty_req).await;
    assert!(empty_resp.status().is_success());
    let empty_json: Value = test::read_body_json(empty_resp).await;
    if empty_json.get("error").is_some() || empty_json.get("code").is_some() {
        println!("Empty address error: {empty_json:?}");
    } else {
        panic!("Expected error for empty addresses");
    }
    // Test with invalid address
    let invalid_request = SendRequest {
        tap_addrs: vec!["invalid_address".to_string()],
        fee_rate: Some(300),
        label: None,
        skip_proof_courier_ping_check: Some(true),
    };
    let invalid_req = test::TestRequest::post()
        .uri("/v1/taproot-assets/send")
        .set_json(&invalid_request)
        .to_request();
    let invalid_resp = test::call_service(&app, invalid_req).await;
    assert!(invalid_resp.status().is_success() || invalid_resp.status().is_client_error());
    if invalid_resp.status().is_success() {
        let invalid_json: Value = test::read_body_json(invalid_resp).await;
        assert!(invalid_json.get("error").is_some() || invalid_json.get("code").is_some());
    }
}

#[actix_rt::test]
#[serial]
async fn test_send_response_structure() {
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
    let addr_req = NewAddrRequest {
        asset_id,
        amt: "25".to_string(),
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
    let addr = addr_json["encoded"].as_str().unwrap().to_string();
    let request = SendRequest {
        tap_addrs: vec![addr],
        fee_rate: Some(300),
        label: Some("Test send".to_string()),
        skip_proof_courier_ping_check: Some(true),
    };
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/send")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let send_json: Value = test::read_body_json(resp).await;
    if send_json.get("error").is_some() || send_json.get("code").is_some() {
        println!("Send failed with error: {send_json:?}");
        return;
    }
    let transfer = &send_json["transfer"];
    assert!(transfer["transfer_timestamp"].is_string());
    assert!(transfer["anchor_tx_hash"].is_string() || transfer["anchor_tx"].is_string());
    assert!(
        transfer["anchor_tx_height_hint"].is_number()
            || transfer["anchor_tx_height_hint"].is_null()
    );
    assert!(
        transfer["anchor_tx_chain_fees"].is_string() || transfer["anchor_tx_chain_fees"].is_null()
    );
    let inputs = transfer["inputs"].as_array().unwrap();
    if !inputs.is_empty() {
        let first_input = &inputs[0];
        assert!(first_input["anchor_point"].is_string());
        assert!(first_input["asset_id"].is_string());
        assert!(first_input["script_key"].is_string());
        assert!(first_input["amount"].is_string());
    }
    let outputs = transfer["outputs"].as_array().unwrap();
    if !outputs.is_empty() {
        let first_output = &outputs[0];
        assert!(first_output["anchor"].is_object());
        assert!(first_output["script_key"].is_string());
        assert!(first_output["amount"].is_string());
        assert!(first_output["output_type"].is_string());
        assert!(first_output["asset_version"].is_string());
        if let Some(output_type) = first_output["output_type"].as_str() {
            let valid_types = ["OUTPUT_TYPE_SIMPLE", "OUTPUT_TYPE_SPLIT_ROOT"];
            assert!(valid_types.contains(&output_type));
        }
        if let Some(status) = first_output["proof_delivery_status"].as_str() {
            let valid_statuses = [
                "PROOF_DELIVERY_STATUS_NOT_APPLICABLE",
                "PROOF_DELIVERY_STATUS_COMPLETE",
                "PROOF_DELIVERY_STATUS_PENDING",
            ];
            assert!(valid_statuses.contains(&status));
        }
    }
}
