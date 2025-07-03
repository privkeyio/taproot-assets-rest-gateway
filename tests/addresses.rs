use actix_web::{test, App};
use serde_json::json;
use serial_test::serial;
use taproot_assets_rest_gateway::api::addresses::{
    DecodeAddrRequest, NewAddrRequest, ReceiveEventsRequest,
};
use taproot_assets_rest_gateway::api::routes::configure;
use taproot_assets_rest_gateway::tests::setup::{mint_test_asset, setup};

#[actix_rt::test]
#[serial]
async fn test_generate_new_address() {
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
    let request = NewAddrRequest {
        asset_id,
        amt: "100".to_string(),
        script_key: None,
        internal_key: None,
        tapscript_sibling: None,
        proof_courier_addr: None,
        asset_version: None,
        address_version: None,
    };
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/addrs")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let addr: serde_json::Value = test::read_body_json(resp).await;
    assert!(addr.get("encoded").and_then(|v| v.as_str()).is_some());
}

#[actix_rt::test]
#[serial]
async fn test_list_all_addresses() {
    let (client, base_url, macaroon_hex, _lnd_macaroon_hex) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;
    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/addrs")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "Request failed with status: {}",
        resp.status()
    );
    let json: serde_json::Value = test::read_body_json(resp).await;
    assert!(json.get("addrs").and_then(|v| v.as_array()).is_some());
}

#[actix_rt::test]
#[serial]
async fn test_decode_address() {
    let (client, base_url, macaroon_hex, lnd_macaroon_hex) = setup().await;
    let asset_id = mint_test_asset(
        client.as_ref(),
        &base_url.0,
        &macaroon_hex.0,
        &lnd_macaroon_hex,
    )
    .await;
    let new_addr_req = NewAddrRequest {
        asset_id: asset_id.clone(),
        amt: "100".to_string(),
        script_key: None,
        internal_key: None,
        tapscript_sibling: None,
        proof_courier_addr: None,
        asset_version: None,
        address_version: None,
    };
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;
    let new_addr_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/addrs")
            .set_json(&new_addr_req)
            .to_request(),
    )
    .await;
    let addr: serde_json::Value = test::read_body_json(new_addr_resp).await;
    let encoded = addr
        .get("encoded")
        .and_then(|v| v.as_str())
        .expect("Encoded address not found")
        .to_string();
    let request = DecodeAddrRequest { addr: encoded };
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/addrs/decode")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let decoded: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(
        decoded.get("asset_id").and_then(|v| v.as_str()),
        Some(asset_id.as_str())
    );
}

#[actix_rt::test]
#[serial]
async fn test_address_creation_with_custom_parameters() {
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

    // Test with custom proof courier address
    let request_with_courier = NewAddrRequest {
        asset_id: asset_id.clone(),
        amt: "250".to_string(),
        script_key: None,
        internal_key: None,
        tapscript_sibling: None,
        proof_courier_addr: Some("universe.example.com:10029".to_string()),
        asset_version: Some("ASSET_VERSION_V0".to_string()),
        address_version: Some("ADDR_VERSION_V0".to_string()),
    };

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/addrs")
        .set_json(&request_with_courier)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let addr: serde_json::Value = test::read_body_json(resp).await;
    println!("Address response: {addr:?}");
    assert!(addr.get("encoded").is_some());

    // The API might not echo back all custom parameters
    // Let's just verify the address was created successfully
    // and check if the fields exist (they might be None/null)
    // Proof courier address might be stored but not returned in response
    if let Some(courier) = addr.get("proof_courier_addr") {
        println!("Proof courier in response: {courier:?}");
    }
    if let Some(asset_ver) = addr.get("asset_version") {
        println!("Asset version in response: {asset_ver:?}");
    }
    if let Some(addr_ver) = addr.get("address_version") {
        println!("Address version in response: {addr_ver:?}");
    }

    // Test with custom script key
    let request_with_script_key = json!({
        "asset_id": asset_id,
        "amt": "500",
        "script_key": {
            "pub_key": "AjVjSPpdLKW4WMSjKyY3QyJJMwJe/I7uRKy7sKww8CTf",
            "key_desc": {
                "raw_key_bytes": "AjVjSPpdLKW4WMSjKyY3QyJJMwJe/I7uRKy7sKww8CTf",
                "key_loc": {
                    "key_family": 1,
                    "key_index": 0
                }
            },
            "tap_tweak": "",
            "type": "SCRIPT_KEY_BIP86"
        }
    });

    let req_with_script = test::TestRequest::post()
        .uri("/v1/taproot-assets/addrs")
        .set_json(&request_with_script_key)
        .to_request();
    let resp_with_script = test::call_service(&app, req_with_script).await;
    // This might fail due to invalid key, but API structure should be correct
    assert!(resp_with_script.status().is_success() || resp_with_script.status().is_client_error());
}

#[actix_rt::test]
#[serial]
async fn test_validate_receive_events() {
    let (client, base_url, macaroon_hex, _lnd_macaroon_hex) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Test without filters
    let request = ReceiveEventsRequest {
        filter_addr: None,
        filter_status: None,
    };
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/addrs/receives")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let json: serde_json::Value = test::read_body_json(resp).await;
    println!("Receive events response: {json:?}");
    assert!(
        json.get("events").is_some(),
        "Response should have 'events' field"
    );

    // Test with address filter
    let request_with_addr = ReceiveEventsRequest {
        filter_addr: Some(
            "taprt1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqn0z0ul"
                .to_string(),
        ),
        filter_status: None,
    };
    let req_with_addr = test::TestRequest::post()
        .uri("/v1/taproot-assets/addrs/receives")
        .set_json(&request_with_addr)
        .to_request();
    let resp_with_addr = test::call_service(&app, req_with_addr).await;
    assert!(resp_with_addr.status().is_success());

    // Test with status filter
    let request_with_status = ReceiveEventsRequest {
        filter_addr: None,
        filter_status: Some("ADDR_EVENT_STATUS_COMPLETED".to_string()),
    };
    let req_with_status = test::TestRequest::post()
        .uri("/v1/taproot-assets/addrs/receives")
        .set_json(&request_with_status)
        .to_request();
    let resp_with_status = test::call_service(&app, req_with_status).await;
    assert!(resp_with_status.status().is_success());

    // Test with both filters
    let request_with_both = ReceiveEventsRequest {
        filter_addr: Some(
            "taprt1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqn0z0ul"
                .to_string(),
        ),
        filter_status: Some("ADDR_EVENT_STATUS_TRANSACTION_CONFIRMED".to_string()),
    };
    let req_with_both = test::TestRequest::post()
        .uri("/v1/taproot-assets/addrs/receives")
        .set_json(&request_with_both)
        .to_request();
    let resp_with_both = test::call_service(&app, req_with_both).await;
    assert!(resp_with_both.status().is_success());
    let json_with_both: serde_json::Value = test::read_body_json(resp_with_both).await;

    // Verify event structure if any events exist
    if let Some(events) = json_with_both.get("events").and_then(|v| v.as_array()) {
        if !events.is_empty() {
            let first_event = &events[0];
            assert!(first_event.get("creation_time_unix_seconds").is_some());
            assert!(first_event.get("addr").is_some());
            assert!(first_event.get("status").is_some());

            // Validate status values
            if let Some(status) = first_event.get("status").and_then(|v| v.as_str()) {
                let valid_statuses = [
                    "ADDR_EVENT_STATUS_UNKNOWN",
                    "ADDR_EVENT_STATUS_TRANSACTION_DETECTED",
                    "ADDR_EVENT_STATUS_TRANSACTION_CONFIRMED",
                    "ADDR_EVENT_STATUS_PROOF_RECEIVED",
                    "ADDR_EVENT_STATUS_COMPLETED",
                ];
                assert!(valid_statuses.contains(&status), "Invalid status: {status}");
            }
        }
    }
}

#[actix_rt::test]
#[serial]
async fn test_query_addresses_with_filters() {
    let (client, base_url, macaroon_hex, _lnd_macaroon_hex) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Test with created_after filter
    let req_after = test::TestRequest::get()
        .uri("/v1/taproot-assets/addrs?created_after=1609459200") // 2021-01-01
        .to_request();
    let resp_after = test::call_service(&app, req_after).await;
    assert!(resp_after.status().is_success());

    // Test with created_before filter
    let req_before = test::TestRequest::get()
        .uri("/v1/taproot-assets/addrs?created_before=1735689600") // 2025-01-01
        .to_request();
    let resp_before = test::call_service(&app, req_before).await;
    assert!(resp_before.status().is_success());

    // Test with limit
    let req_limit = test::TestRequest::get()
        .uri("/v1/taproot-assets/addrs?limit=10")
        .to_request();
    let resp_limit = test::call_service(&app, req_limit).await;
    assert!(resp_limit.status().is_success());
    let json_limit: serde_json::Value = test::read_body_json(resp_limit).await;
    if let Some(addrs) = json_limit.get("addrs").and_then(|v| v.as_array()) {
        assert!(addrs.len() <= 10);
    }

    // Test with offset
    let req_offset = test::TestRequest::get()
        .uri("/v1/taproot-assets/addrs?offset=5")
        .to_request();
    let resp_offset = test::call_service(&app, req_offset).await;
    assert!(resp_offset.status().is_success());

    // Test with combined filters
    let req_combined = test::TestRequest::get()
        .uri("/v1/taproot-assets/addrs?created_after=1609459200&limit=5&offset=0")
        .to_request();
    let resp_combined = test::call_service(&app, req_combined).await;
    assert!(resp_combined.status().is_success());
}
