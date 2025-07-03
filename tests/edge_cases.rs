use actix_web::{test, App};
use serde_json::{json, Value};
use serial_test::serial;
use taproot_assets_rest_gateway::api::routes::configure;
use taproot_assets_rest_gateway::tests::setup::{setup, setup_without_assets};
use uuid::Uuid;

#[actix_rt::test]
#[serial]
async fn test_zero_value_transfers() {
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

    let req = json!({
        "asset_id": asset_id,
        "amt": "0"
    });
    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/addrs")
            .set_json(&req)
            .to_request(),
    )
    .await;

    // With validation, this should return a 400 Bad Request
    assert!(resp.status().is_client_error());
    let json: Value = test::read_body_json(resp).await;
    assert!(json.get("error").is_some());
    assert!(json["error"]
        .as_str()
        .unwrap()
        .contains("amt must be greater than zero"));
}

#[actix_rt::test]
#[serial]
async fn test_maximum_asset_amount() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let _ = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/assets/mint/cancel")
            .set_json(json!({}))
            .to_request(),
    )
    .await;

    let req = json!({
        "asset": {
            "asset_type": "NORMAL",
            "name": format!("max-amount-{}", Uuid::new_v4()),
            "amount": "18446744073709551615"
        },
        "short_response": true
    });
    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/assets")
            .set_json(&req)
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
}

#[actix_rt::test]
#[serial]
async fn test_unicode_special_characters() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let _ = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/assets/mint/cancel")
            .set_json(json!({}))
            .to_request(),
    )
    .await;

    let special_names = vec![
        "🚀-rocket-asset",
        "测试资产",
        "アセット",
        "Ñoño-asset",
        "asset_with_emoji_😊",
    ];

    for name in special_names {
        let req = json!({
            "asset": {
                "asset_type": "NORMAL",
                "name": format!("{}-{}", name, Uuid::new_v4()),
                "amount": "100"
            },
            "short_response": true
        });
        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/taproot-assets/assets")
                .set_json(&req)
                .to_request(),
        )
        .await;
        assert!(resp.status().is_success());
    }
}

#[actix_rt::test]
#[serial]
async fn test_very_long_asset_names() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let _ = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/assets/mint/cancel")
            .set_json(json!({}))
            .to_request(),
    )
    .await;

    let long_name = format!("{}-{}", "a".repeat(255), Uuid::new_v4());
    let req = json!({
        "asset": {
            "asset_type": "NORMAL",
            "name": long_name,
            "amount": "100"
        },
        "short_response": true
    });
    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/assets")
            .set_json(&req)
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
}

#[actix_rt::test]
#[serial]
async fn test_concurrent_operations_same_asset() {
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

    let futures: Vec<_> = (0..5)
        .map(|i| {
            let req = json!({
                "asset_id": asset_id,
                "amt": format!("{}", (i + 1) * 10)
            });
            test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/taproot-assets/addrs")
                    .set_json(&req)
                    .to_request(),
            )
        })
        .collect();

    let results = futures::future::join_all(futures).await;
    let all_successful = results.iter().all(|resp| resp.status().is_success());
    assert!(all_successful);
}

#[actix_rt::test]
#[serial]
async fn test_negative_amounts() {
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

    let req = json!({
        "asset_id": asset_id,
        "amt": "-100"
    });
    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/addrs")
            .set_json(&req)
            .to_request(),
    )
    .await;

    // With validation, this should return a 400 Bad Request
    assert!(resp.status().is_client_error());
    let json: Value = test::read_body_json(resp).await;
    assert!(json.get("error").is_some());
    assert!(json["error"]
        .as_str()
        .unwrap()
        .contains("amt must be greater than zero"));
}

#[actix_rt::test]
async fn test_empty_string_fields() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let req = json!({
        "asset_id": "",
        "amt": "100"
    });
    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/addrs")
            .set_json(&req)
            .to_request(),
    )
    .await;

    // With validation, this should return a 400 Bad Request
    assert!(resp.status().is_client_error());
    let json: Value = test::read_body_json(resp).await;
    assert!(json.get("error").is_some());
    assert!(json["error"]
        .as_str()
        .unwrap()
        .contains("asset_id cannot be empty"));
}

#[actix_rt::test]
async fn test_base64_encoding_edge_cases() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let edge_cases = vec!["", "A", "AA", "AAA", "AAAA", "////", "++++", "===="];

    for case in edge_cases {
        let req = json!({
            "raw_proof": case,
            "proof_at_depth": 0,
            "with_prev_witnesses": true,
            "with_meta_reveal": true
        });
        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/taproot-assets/proofs/decode")
                .set_json(&req)
                .to_request(),
        )
        .await;
        assert!(resp.status().is_success() || resp.status().is_client_error());
    }
}

#[actix_rt::test]
async fn test_url_safe_base64() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let _standard_b64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=";
    let url_safe_b64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_=";

    let req = test::TestRequest::get()
        .uri(&format!(
            "/v1/taproot-assets/universe/proofs/asset-id/{url_safe_b64}/hash/index/{url_safe_b64}"
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success() || resp.status().is_client_error());
}

#[actix_rt::test]
#[serial]
async fn test_float_amount_handling() {
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

    let req = json!({
        "asset_id": asset_id,
        "amt": "10.5"
    });
    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/addrs")
            .set_json(&req)
            .to_request(),
    )
    .await;

    // With validation, float amounts should return a 400 Bad Request
    assert!(resp.status().is_client_error());
    let json: Value = test::read_body_json(resp).await;
    assert!(json.get("error").is_some());
    assert!(json["error"]
        .as_str()
        .unwrap()
        .contains("amt must be a valid integer"));
}

#[actix_rt::test]
#[serial]
async fn test_null_vs_empty_fields() {
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

    // Test with null values (should be accepted)
    let req_with_null = json!({
        "asset_id": asset_id,
        "amt": "100",
        "script_key": null,
        "internal_key": null
    });
    let resp1 = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/addrs")
            .set_json(&req_with_null)
            .to_request(),
    )
    .await;
    assert!(resp1.status().is_success());

    // Test with empty strings (should be rejected)
    let req_with_empty = json!({
        "asset_id": asset_id,
        "amt": "100",
        "script_key": "",
        "internal_key": ""
    });
    let resp2 = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/addrs")
            .set_json(&req_with_empty)
            .to_request(),
    )
    .await;

    // With validation, empty strings for optional fields should return a 400 Bad Request
    assert!(resp2.status().is_client_error());
    let json: Value = test::read_body_json(resp2).await;
    assert!(json.get("error").is_some());
}
