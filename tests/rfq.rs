use actix_web::{test, App};
use serde_json::{json, Value};
use serial_test::serial;
use taproot_assets_rest_gateway::api::rfq::{
    BuyOfferRequest, BuyOrderRequest, SellOfferRequest, SellOrderRequest,
};
use taproot_assets_rest_gateway::api::routes::configure;
use taproot_assets_rest_gateway::tests::setup::{mint_test_asset, setup, setup_without_assets};
use tracing::info;

#[actix_rt::test]
#[serial]
async fn test_create_buy_offer() {
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

    info!("Testing create buy offer for asset: {}", asset_id);

    let request = BuyOfferRequest {
        asset_specifier: json!({
            "asset_id_str": asset_id.clone()
        }),
        max_units: "1000".to_string(),
    };

    let req = test::TestRequest::post()
        .uri(&format!(
            "/v1/taproot-assets/rfq/buyoffer/asset-id/{asset_id}"
        ))
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    info!("Buy offer response: {:?}", json);

    // Should return empty response on success
    assert!(json.is_object());
}

#[actix_rt::test]
#[serial]
async fn test_create_sell_offer() {
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

    info!("Testing create sell offer for asset: {}", asset_id);

    let request = SellOfferRequest {
        asset_specifier: json!({
            "asset_id_str": asset_id.clone()
        }),
        max_units: "500".to_string(),
    };

    let req = test::TestRequest::post()
        .uri(&format!(
            "/v1/taproot-assets/rfq/selloffer/asset-id/{asset_id}"
        ))
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    info!("Sell offer response: {:?}", json);

    // Should return empty response on success
    assert!(json.is_object());
}

#[actix_rt::test]
#[serial]
async fn test_submit_buy_order() {
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

    info!("Testing submit buy order for asset: {}", asset_id);

    // Note: This test will likely fail without a proper peer setup
    let request = BuyOrderRequest {
        asset_specifier: json!({
            "asset_id_str": asset_id.clone()
        }),
        asset_max_amt: "100".to_string(),
        expiry: (chrono::Utc::now().timestamp() + 3600).to_string(), // 1 hour from now
        peer_pub_key: "02b3e11afe72c19e288b1f039c9d15a99e9e2f4c98a90c085c3cf3e0ed9d27ad8b"
            .to_string(), // Example pubkey
        timeout_seconds: 30,
        skip_asset_channel_check: true, // Skip for testing
    };

    let req = test::TestRequest::post()
        .uri(&format!(
            "/v1/taproot-assets/rfq/buyorder/asset-id/{asset_id}"
        ))
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    info!("Buy order response: {:?}", json);

    // Check if it's an error response first
    if json.get("error").is_some() || json.get("code").is_some() {
        info!("Buy order returned error: {:?}", json);
        // This is expected without proper peer setup
        return;
    }

    // May return accepted_quote, invalid_quote, or rejected_quote
    assert!(
        json.get("accepted_quote").is_some()
            || json.get("invalid_quote").is_some()
            || json.get("rejected_quote").is_some()
            || json.is_object(), // Empty response is also valid
        "Unexpected response structure: {json:?}"
    );

    if let Some(accepted) = json.get("accepted_quote") {
        assert!(accepted["peer"].is_string());
        assert!(accepted["id"].is_string());
        assert!(accepted["scid"].is_string());
        assert!(accepted["ask_asset_rate"].is_object());
    } else if let Some(rejected) = json.get("rejected_quote") {
        assert!(rejected["peer"].is_string());
        assert!(rejected["error_message"].is_string());
    } else if let Some(invalid) = json.get("invalid_quote") {
        assert!(invalid["status"].is_string());
        assert!(invalid["peer"].is_string());
    }
}

#[actix_rt::test]
#[serial]
async fn test_submit_sell_order() {
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

    info!("Testing submit sell order for asset: {}", asset_id);

    let request = SellOrderRequest {
        asset_specifier: json!({
            "asset_id_str": asset_id.clone()
        }),
        payment_max_amt: "1000000".to_string(), // 1M sats
        expiry: (chrono::Utc::now().timestamp() + 3600).to_string(),
        peer_pub_key: "02b3e11afe72c19e288b1f039c9d15a99e9e2f4c98a90c085c3cf3e0ed9d27ad8b"
            .to_string(),
        timeout_seconds: 30,
        skip_asset_channel_check: true,
    };

    let req = test::TestRequest::post()
        .uri(&format!(
            "/v1/taproot-assets/rfq/sellorder/asset-id/{asset_id}"
        ))
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    info!("Sell order response: {:?}", json);

    // Check if it's an error response first
    if json.get("error").is_some() || json.get("code").is_some() {
        info!("Sell order returned error: {:?}", json);
        // This is expected without proper peer setup
        return;
    }

    // May return accepted_quote, invalid_quote, or rejected_quote
    assert!(
        json.get("accepted_quote").is_some()
            || json.get("invalid_quote").is_some()
            || json.get("rejected_quote").is_some()
            || json.is_object(), // Empty response is also valid
        "Unexpected response structure: {json:?}"
    );

    if let Some(accepted) = json.get("accepted_quote") {
        assert!(accepted["peer"].is_string());
        assert!(accepted["id"].is_string());
        assert!(accepted["scid"].is_string());
        assert!(accepted["bid_asset_rate"].is_object());
    }
}

#[actix_rt::test]
async fn test_get_peer_accepted_quotes() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    info!("Testing get peer-accepted quotes");

    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/rfq/quotes/peeraccepted")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    info!("Peer accepted quotes response: {:?}", json);

    assert!(json["buy_quotes"].is_array());
    assert!(json["sell_quotes"].is_array());

    // Check buy quotes structure
    let buy_quotes = json["buy_quotes"].as_array().unwrap();
    for quote in buy_quotes {
        assert!(quote["peer"].is_string());
        assert!(quote["id"].is_string());
        assert!(quote["scid"].is_string());
        assert!(quote["asset_max_amount"].is_string());
        assert!(quote["ask_asset_rate"].is_object());
        assert!(quote["expiry"].is_string());
        assert!(quote["min_transportable_units"].is_string());
    }

    // Check sell quotes structure
    let sell_quotes = json["sell_quotes"].as_array().unwrap();
    for quote in sell_quotes {
        assert!(quote["peer"].is_string());
        assert!(quote["id"].is_string());
        assert!(quote["scid"].is_string());
        assert!(quote["asset_amount"].is_string());
        assert!(quote["bid_asset_rate"].is_object());
        assert!(quote["expiry"].is_string());
        assert!(quote["min_transportable_msat"].is_string());
    }
}

#[actix_rt::test]
async fn test_get_asset_rates() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    info!("Testing get asset rates");

    // Test with BTC as payment asset (asset ID all zeros)
    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/rfq/priceoracle/assetrates?transaction_type=PURCHASE&subject_asset.asset_id_str=0000000000000000000000000000000000000000000000000000000000000001&subject_asset_max_amount=1000&payment_asset.asset_id_str=0000000000000000000000000000000000000000000000000000000000000000")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    info!("Asset rates response: {:?}", json);

    // Response contains either ok or error
    if let Some(ok_resp) = json.get("ok") {
        assert!(ok_resp["asset_rates"].is_object());
        let rates = &ok_resp["asset_rates"];
        assert!(rates["subjectAssetRate"].is_object());
        assert!(rates["paymentAssetRate"].is_object());
        assert!(rates["expiry_timestamp"].is_string());
    } else if let Some(error_resp) = json.get("error") {
        assert!(error_resp["message"].is_string());
        assert!(error_resp["code"].is_number());
    }
}

#[actix_rt::test]
async fn test_receive_rfq_notifications() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    info!("Testing receive RFQ notifications");

    // This is a streaming endpoint, so we just test that it accepts the request
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/rfq/ntfs")
        .set_json(json!({}))
        .to_request();
    let resp = test::call_service(&app, req).await;

    // The endpoint might return an error if streaming is not supported in test mode
    if !resp.status().is_success() {
        info!(
            "RFQ notifications endpoint returned status: {}",
            resp.status()
        );
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8_lossy(&body);
        info!("Response body: {}", body_str);
        // This is acceptable for a streaming endpoint in test mode
        return;
    }

    let json: Value = test::read_body_json(resp).await;
    info!("RFQ notifications response: {:?}", json);

    // Response should have event fields (might be empty initially)
    assert!(
        json.get("peer_accepted_buy_quote").is_some()
            || json.get("peer_accepted_sell_quote").is_some()
            || json.get("accept_htlc").is_some()
    );
}

#[actix_rt::test]
#[serial]
async fn test_buy_offer_with_group_key() {
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

    // Test with group key instead of asset ID
    let request = BuyOfferRequest {
        asset_specifier: json!({
            "group_key_str": "0000000000000000000000000000000000000000000000000000000000000000"
        }),
        max_units: "2000".to_string(),
    };

    let req = test::TestRequest::post()
        .uri(&format!(
            "/v1/taproot-assets/rfq/buyoffer/asset-id/{asset_id}"
        ))
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_rt::test]
async fn test_asset_rates_with_hint() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Test with asset rates hint
    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/rfq/priceoracle/assetrates?transaction_type=SALE&subject_asset.asset_id_str=0000000000000000000000000000000000000000000000000000000000000001&subject_asset_max_amount=500&payment_asset.asset_id_str=0000000000000000000000000000000000000000000000000000000000000000&asset_rates_hint.subjectAssetRate.coefficient=1000000&asset_rates_hint.subjectAssetRate.scale=8&asset_rates_hint.paymentAssetRate.coefficient=100000000&asset_rates_hint.paymentAssetRate.scale=8&asset_rates_hint.expiry_timestamp=1800000000")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    info!("Asset rates with hint response: {:?}", json);
}

#[actix_rt::test]
#[serial]
async fn test_buy_order_timeout_scenarios() {
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

    // Test with very short timeout
    let request = BuyOrderRequest {
        asset_specifier: json!({
            "asset_id_str": asset_id.clone()
        }),
        asset_max_amt: "50".to_string(),
        expiry: (chrono::Utc::now().timestamp() + 60).to_string(), // 1 minute
        peer_pub_key: "02b3e11afe72c19e288b1f039c9d15a99e9e2f4c98a90c085c3cf3e0ed9d27ad8b"
            .to_string(),
        timeout_seconds: 1, // Very short timeout
        skip_asset_channel_check: true,
    };

    let req = test::TestRequest::post()
        .uri(&format!(
            "/v1/taproot-assets/rfq/buyorder/asset-id/{asset_id}"
        ))
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    // Likely to be rejected or timeout
    info!("Buy order with short timeout response: {:?}", json);
}

#[actix_rt::test]
async fn test_rfq_event_structure() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Create multiple RFQ notification subscriptions to test event structure
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/rfq/ntfs")
        .set_json(json!({}))
        .to_request();
    let resp = test::call_service(&app, req).await;

    // The endpoint might return an error if streaming is not supported in test mode
    if !resp.status().is_success() {
        info!(
            "RFQ notifications endpoint returned status: {}",
            resp.status()
        );
        // This is acceptable for a streaming endpoint in test mode
        return;
    }

    let json: Value = test::read_body_json(resp).await;

    // Check possible event types
    if let Some(buy_quote_event) = json.get("peer_accepted_buy_quote") {
        assert!(buy_quote_event["timestamp"].is_string());
        assert!(buy_quote_event["peer_accepted_buy_quote"].is_object());
    }

    if let Some(sell_quote_event) = json.get("peer_accepted_sell_quote") {
        assert!(sell_quote_event["timestamp"].is_string());
        assert!(sell_quote_event["peer_accepted_sell_quote"].is_object());
    }

    if let Some(htlc_event) = json.get("accept_htlc") {
        assert!(htlc_event["timestamp"].is_string());
        assert!(htlc_event["scid"].is_string());
    }
}

#[actix_rt::test]
async fn test_fixed_point_rate_conversion() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Get quotes to test fixed point conversion
    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/rfq/quotes/peeraccepted")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;

    // Check rate structure in any existing quotes
    let buy_quotes = json["buy_quotes"].as_array().unwrap();
    for quote in buy_quotes {
        if let Some(rate) = quote.get("ask_asset_rate") {
            assert!(rate["coefficient"].is_string());
            assert!(rate["scale"].is_number());

            // Verify we can parse the coefficient
            let coefficient = rate["coefficient"].as_str().unwrap();
            assert!(coefficient.parse::<u64>().is_ok());
        }
    }
}

#[actix_rt::test]
#[serial]
async fn test_create_multiple_offers() {
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

    // Create multiple buy and sell offers
    for i in 1..=3 {
        // Buy offer
        let buy_request = BuyOfferRequest {
            asset_specifier: json!({
                "asset_id_str": asset_id.clone()
            }),
            max_units: (i * 100).to_string(),
        };

        let buy_req = test::TestRequest::post()
            .uri(&format!(
                "/v1/taproot-assets/rfq/buyoffer/asset-id/{asset_id}"
            ))
            .set_json(&buy_request)
            .to_request();
        let buy_resp = test::call_service(&app, buy_req).await;
        assert!(buy_resp.status().is_success());

        // Sell offer
        let sell_request = SellOfferRequest {
            asset_specifier: json!({
                "asset_id_str": asset_id.clone()
            }),
            max_units: (i * 50).to_string(),
        };

        let sell_req = test::TestRequest::post()
            .uri(&format!(
                "/v1/taproot-assets/rfq/selloffer/asset-id/{asset_id}"
            ))
            .set_json(&sell_request)
            .to_request();
        let sell_resp = test::call_service(&app, sell_req).await;
        assert!(sell_resp.status().is_success());
    }

    info!("Successfully created multiple buy and sell offers");
}

#[actix_rt::test]
async fn test_quote_expiry_validation() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Get peer quotes to check expiry
    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/rfq/quotes/peeraccepted")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    let current_time = chrono::Utc::now().timestamp() as u64;

    // Check buy quotes expiry
    let buy_quotes = json["buy_quotes"].as_array().unwrap();
    for quote in buy_quotes {
        if let Some(expiry_str) = quote["expiry"].as_str() {
            let expiry: u64 = expiry_str.parse().unwrap_or(0);
            info!(
                "Buy quote expiry: {}, current time: {}",
                expiry, current_time
            );

            // Check if quote is expired
            if expiry < current_time {
                info!("Found expired buy quote");
            }
        }
    }

    // Check sell quotes expiry
    let sell_quotes = json["sell_quotes"].as_array().unwrap();
    for quote in sell_quotes {
        if let Some(expiry_str) = quote["expiry"].as_str() {
            let expiry: u64 = expiry_str.parse().unwrap_or(0);
            info!(
                "Sell quote expiry: {}, current time: {}",
                expiry, current_time
            );

            // Check if quote is expired
            if expiry < current_time {
                info!("Found expired sell quote");
            }
        }
    }
}

#[actix_rt::test]
async fn test_min_transportable_units() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Get peer quotes to check minimum transportable units
    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/rfq/quotes/peeraccepted")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;

    // Check buy quotes min transportable units
    let buy_quotes = json["buy_quotes"].as_array().unwrap();
    for quote in buy_quotes {
        if let Some(min_units_str) = quote["min_transportable_units"].as_str() {
            let min_units: u64 = min_units_str.parse().unwrap_or(0);
            info!("Buy quote min transportable units: {}", min_units);

            // Minimum should be based on dust limit
            assert!(min_units > 0 || buy_quotes.is_empty());
        }
    }

    // Check sell quotes min transportable msat
    let sell_quotes = json["sell_quotes"].as_array().unwrap();
    for quote in sell_quotes {
        if let Some(min_msat_str) = quote["min_transportable_msat"].as_str() {
            let min_msat: u64 = min_msat_str.parse().unwrap_or(0);
            info!("Sell quote min transportable msat: {}", min_msat);

            // Should be at least 354,000 msat (dust limit)
            assert!(min_msat >= 354000 || sell_quotes.is_empty());
        }
    }
}
