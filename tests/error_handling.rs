use actix_web::{test, App};
use serde_json::{json, Value};
use serial_test::serial;
use taproot_assets_rest_gateway::api::routes::configure;
use taproot_assets_rest_gateway::tests::setup::{setup, setup_without_assets};
use tokio::time::{sleep, Duration};

#[actix_rt::test]
#[serial]
async fn test_invalid_asset_id_handling() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let invalid_asset_id = "invalid_asset_id_123";
    let req = test::TestRequest::get()
        .uri(&format!(
            "/v1/taproot-assets/assets/meta/asset-id/{invalid_asset_id}"
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_client_error() || resp.status().is_success());

    if resp.status().is_success() {
        let json: Value = test::read_body_json(resp).await;
        assert!(json.get("error").is_some() || json.get("code").is_some());
    }
}

#[actix_rt::test]
#[serial]
async fn test_insufficient_balance_error() {
    let (client, base_url, macaroon_hex, lnd_macaroon_hex) = setup().await;
    let asset_id = taproot_assets_rest_gateway::tests::setup::mint_test_asset(
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

    let addr_req = json!({
        "asset_id": asset_id,
        "amt": "999999999999"
    });
    let addr_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/addrs")
            .set_json(&addr_req)
            .to_request(),
    )
    .await;
    let addr_json: Value = test::read_body_json(addr_resp).await;

    if let Some(addr) = addr_json.get("encoded").and_then(|v| v.as_str()) {
        let send_req = json!({
            "tap_addrs": vec![addr],
            "fee_rate": 300
        });
        let send_resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/taproot-assets/send")
                .set_json(&send_req)
                .to_request(),
        )
        .await;
        assert!(send_resp.status().is_success());
        let send_json: Value = test::read_body_json(send_resp).await;
        assert!(send_json.get("error").is_some() || send_json.get("code").is_some());
    }
}

#[actix_rt::test]
async fn test_network_timeout_handling() {
    let (_client, base_url, macaroon_hex) = setup_without_assets().await;
    let timeout_client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_millis(1))
        .build()
        .unwrap();

    let app = test::init_service(
        App::new()
            .app_data(actix_web::web::Data::new(timeout_client))
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/getinfo")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_server_error() || resp.status().as_u16() == 504);
}

#[actix_rt::test]
#[serial]
async fn test_invalid_macaroon_authentication() {
    let (client, base_url, _, _) = setup().await;
    let invalid_macaroon = "0000000000000000000000000000000000000000";
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(actix_web::web::Data::new(
                taproot_assets_rest_gateway::types::MacaroonHex(invalid_macaroon.to_string()),
            ))
            .configure(configure),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/getinfo")
        .to_request();
    let resp = test::call_service(&app, req).await;

    // The backend might return 200 OK with an error in the JSON body
    // or it might return a proper error status code
    if resp.status().is_success() {
        let json: Value = test::read_body_json(resp).await;
        // Check if the response contains an error field indicating authentication failure
        assert!(
            json.get("error").is_some()
                || json.get("code").is_some()
                || json
                    .get("message")
                    .and_then(|m| m.as_str())
                    .map(|s| s.contains("macaroon"))
                    .unwrap_or(false),
            "Expected error response for invalid macaroon, got: {json:?}"
        );
    } else {
        // If not successful, it should be a client or server error
        assert!(resp.status().is_client_error() || resp.status().is_server_error());
    }
}

#[actix_rt::test]
#[serial]
async fn test_malformed_request_validation() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let malformed_req = json!({
        "asset_id": "invalid",
        "amt": "not_a_number"
    });
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/addrs")
        .set_json(&malformed_req)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success() || resp.status().is_client_error());

    if resp.status().is_success() {
        let json: Value = test::read_body_json(resp).await;
        assert!(json.get("error").is_some() || json.get("code").is_some());
    }
}

#[actix_rt::test]
#[serial]
async fn test_rate_limiting_behavior() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let mut responses = Vec::new();
    for i in 0..100 {
        let req = test::TestRequest::get()
            .uri("/v1/taproot-assets/getinfo")
            .to_request();
        let resp = test::call_service(&app, req).await;
        responses.push((i, resp.status()));

        if i % 10 == 0 {
            sleep(Duration::from_millis(10)).await;
        }
    }

    let rate_limited = responses.iter().any(|(_, status)| status.as_u16() == 429);
    if rate_limited {
        println!("Rate limiting detected during rapid requests");
    }
}

#[actix_rt::test]
#[serial]
async fn test_empty_response_handling() {
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
        .uri("/v1/taproot-assets/universe/roots?limit=0")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let json: Value = test::read_body_json(resp).await;
    assert!(json["universe_roots"].is_object() || json["universe_roots"].is_array());
}

#[actix_rt::test]
#[serial]
async fn test_concurrent_operation_errors() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let futures: Vec<_> = (0..10)
        .map(|_| {
            let req = test::TestRequest::get()
                .uri("/v1/taproot-assets/assets")
                .to_request();
            test::call_service(&app, req)
        })
        .collect();

    let results = futures::future::join_all(futures).await;
    let all_successful = results.iter().all(|resp| resp.status().is_success());
    assert!(all_successful, "Concurrent operations should not fail");
}

#[actix_rt::test]
#[serial]
async fn test_invalid_hex_encoding() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let invalid_hex = "gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg";
    let req = test::TestRequest::get()
        .uri(&format!(
            "/v1/taproot-assets/assets/meta/asset-id/{invalid_hex}"
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_client_error() || resp.status().is_success());
}

#[actix_rt::test]
#[serial]
async fn test_missing_required_fields() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let incomplete_req = json!({
        "asset_id": "0000000000000000000000000000000000000000000000000000000000000000"
    });
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/addrs")
        .set_json(&incomplete_req)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success() || resp.status().is_client_error());

    if resp.status().is_success() {
        let json: Value = test::read_body_json(resp).await;
        assert!(json.get("error").is_some() || json.get("code").is_some());
    }
}
