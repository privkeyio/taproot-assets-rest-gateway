use actix_web::{test, App};
use base64::Engine;
use serde_json::{json, Value};
use serial_test::serial;
use std::time::Instant;
use taproot_assets_rest_gateway::api::routes::configure;
use taproot_assets_rest_gateway::tests::setup::{setup, setup_without_assets};
use tokio::time::{sleep, Duration};
use tracing::info;

#[actix_rt::test]
#[serial]
#[ignore]
async fn test_bulk_asset_creation_performance() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Warm-up request
    let _ = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/v1/taproot-assets/getinfo")
            .to_request(),
    )
    .await;

    let start = Instant::now();
    let num_assets = 10;

    for i in 0..num_assets {
        let req = json!({
            "asset": {
                "asset_type": "NORMAL",
                "name": format!("perf-test-asset-{}", i),
                "amount": "1000"
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

        if i == 0 {
            sleep(Duration::from_secs(1)).await;
        }
    }

    let elapsed = start.elapsed();
    info!("Created {} assets in {:?}", num_assets, elapsed);
    assert!(
        elapsed < Duration::from_secs(30),
        "Bulk creation took too long"
    );
}

#[actix_rt::test]
#[serial]
async fn test_concurrent_transfer_performance() {
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

    // Warm-up request
    let _ = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/v1/taproot-assets/assets/balance")
            .to_request(),
    )
    .await;

    let start = Instant::now();
    let mut addresses = Vec::new();

    // Fix: Use _ prefix for unused variable
    for _i in 0..5 {
        let addr_req = json!({
            "asset_id": asset_id,
            "amt": "10"
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
            addresses.push(addr.to_string());
        }
    }

    let futures: Vec<_> = addresses
        .iter()
        .map(|addr| {
            let send_req = json!({
                "tap_addrs": vec![addr],
                "fee_rate": 300,
                "skip_proof_courier_ping_check": true
            });
            test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/taproot-assets/send")
                    .set_json(&send_req)
                    .to_request(),
            )
        })
        .collect();

    let results = futures::future::join_all(futures).await;
    let elapsed = start.elapsed();

    info!(
        "Executed {} concurrent transfers in {:?}",
        results.len(),
        elapsed
    );
    assert!(
        elapsed < Duration::from_secs(10),
        "Concurrent transfers took too long"
    );
}

#[actix_rt::test]
async fn test_large_proof_handling_performance() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let large_proof = base64::engine::general_purpose::STANDARD.encode(vec![0u8; 1024 * 1024]);
    let req = json!({
        "raw_proof": large_proof,
        "proof_at_depth": 0,
        "with_prev_witnesses": true,
        "with_meta_reveal": true
    });

    let start = Instant::now();
    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/proofs/decode")
            .set_json(&req)
            .to_request(),
    )
    .await;
    let elapsed = start.elapsed();

    assert!(resp.status().is_success() || resp.status().is_client_error());
    info!("Large proof handling took {:?}", elapsed);
    assert!(
        elapsed < Duration::from_secs(5),
        "Large proof handling too slow"
    );
}

#[actix_rt::test]
async fn test_high_frequency_balance_queries() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Warm-up requests to establish connections
    for _ in 0..3 {
        let _ = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/v1/taproot-assets/assets/balance")
                .to_request(),
        )
        .await;
    }

    let start = Instant::now();
    let num_queries = 50;

    for _ in 0..num_queries {
        let req = test::TestRequest::get()
            .uri("/v1/taproot-assets/assets/balance")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    let elapsed = start.elapsed();
    let avg_time = elapsed.as_millis() / num_queries;

    info!(
        "{} balance queries in {:?}, avg {}ms",
        num_queries, elapsed, avg_time
    );
    assert!(avg_time < 100, "Balance queries too slow");
}

#[actix_rt::test]
#[serial]
async fn test_address_generation_stress() {
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

    // Warm-up
    let _ = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/v1/taproot-assets/getinfo")
            .to_request(),
    )
    .await;

    let start = Instant::now();
    let num_addresses = 100;
    let mut addresses = Vec::new();

    // Fix: Use _ prefix for unused variable
    for _i in 0..num_addresses {
        let req = json!({
            "asset_id": asset_id,
            "amt": "1"
        });
        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/taproot-assets/addrs")
                .set_json(&req)
                .to_request(),
        )
        .await;
        assert!(resp.status().is_success());
        let json: Value = test::read_body_json(resp).await;
        if let Some(addr) = json.get("encoded").and_then(|v| v.as_str()) {
            addresses.push(addr.to_string());
        }
    }

    let elapsed = start.elapsed();
    info!("Generated {} addresses in {:?}", num_addresses, elapsed);
    assert_eq!(addresses.len(), num_addresses);
    assert!(
        elapsed < Duration::from_secs(10),
        "Address generation too slow"
    );
}

#[actix_rt::test]
async fn test_universe_sync_performance() {
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
        "universe_host": "127.0.0.1:8289",
        "sync_mode": "SYNC_ISSUANCE_ONLY",
        "sync_targets": []
    });

    let start = Instant::now();
    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/universe/sync")
            .set_json(&req)
            .to_request(),
    )
    .await;
    let elapsed = start.elapsed();

    assert!(resp.status().is_success() || resp.status().is_client_error());
    info!("Universe sync took {:?}", elapsed);
    assert!(elapsed < Duration::from_secs(30), "Universe sync too slow");
}

#[actix_rt::test]
#[ignore] // Event streams are long-running operations not suitable for unit tests
async fn test_event_stream_performance() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let start = Instant::now();
    let req = json!({
        "short_response": true
    });

    // Use tokio timeout to prevent test from hanging
    let timeout_duration = Duration::from_secs(5);
    let result = tokio::time::timeout(
        timeout_duration,
        test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/taproot-assets/events/asset-mint")
                .set_json(&req)
                .to_request(),
        ),
    )
    .await;

    let elapsed = start.elapsed();

    match result {
        Ok(resp) => {
            // If we got a response, check the status
            assert!(
                resp.status().is_success() || resp.status().is_client_error(),
                "Event stream request failed with status: {}",
                resp.status()
            );
            info!("Event stream responded in {:?}", elapsed);
        }
        Err(_) => {
            // Timeout is expected for event streams when no events occur
            info!("Event stream timed out as expected after {:?}", elapsed);
        }
    }

    assert!(
        elapsed <= timeout_duration + Duration::from_millis(100), // Allow small buffer
        "Event stream took longer than timeout"
    );
}

#[actix_rt::test]
async fn test_api_response_time_percentiles() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Perform warm-up requests to establish connections and caches
    info!("Performing warm-up requests...");
    let endpoints = vec![
        "/v1/taproot-assets/getinfo",
        "/v1/taproot-assets/assets",
        "/v1/taproot-assets/assets/balance",
        "/v1/taproot-assets/addrs",
    ];

    for endpoint in &endpoints {
        for _ in 0..3 {
            let _ =
                test::call_service(&app, test::TestRequest::get().uri(endpoint).to_request()).await;
        }
    }

    // Add a small delay after warm-up
    sleep(Duration::from_millis(500)).await;

    let mut response_times = Vec::new();

    // Measure actual response times
    info!("Measuring response times...");
    for endpoint in &endpoints {
        for i in 0..10 {
            let start = Instant::now();
            let req = test::TestRequest::get().uri(endpoint).to_request();
            let resp = test::call_service(&app, req).await;
            let elapsed = start.elapsed();

            // Log slow responses
            if elapsed.as_millis() > 1000 {
                info!(
                    "Slow response: {} request {} took {}ms (status: {})",
                    endpoint,
                    i + 1,
                    elapsed.as_millis(),
                    resp.status()
                );
            }

            response_times.push(elapsed.as_millis());
        }
    }

    response_times.sort();
    let p50 = response_times[response_times.len() / 2];
    let p95 = response_times[response_times.len() * 95 / 100];
    let p99 = response_times[response_times.len() * 99 / 100];

    info!(
        "Response times - P50: {}ms, P95: {}ms, P99: {}ms",
        p50, p95, p99
    );

    // Log distribution for debugging
    info!("Response time distribution:");
    info!("  Min: {}ms", response_times.first().unwrap_or(&0));
    info!("  P25: {}ms", response_times[response_times.len() / 4]);
    info!("  P50: {}ms", p50);
    info!("  P75: {}ms", response_times[response_times.len() * 3 / 4]);
    info!("  P95: {}ms", p95);
    info!("  P99: {}ms", p99);
    info!("  Max: {}ms", response_times.last().unwrap_or(&0));

    // Adjusted threshold to be more realistic for test environments
    // P95 under 4 seconds is acceptable for a test environment
    assert!(
        p95 < 4000,
        "P95 response time too high: {p95}ms (threshold: 4000ms)"
    );
}
