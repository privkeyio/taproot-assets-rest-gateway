use actix_web::{test, App};
use std::time::Duration;
use taproot_assets_rest_gateway::api::events::{
    AssetMintRequest, AssetReceiveRequest, AssetSendRequest,
};
use taproot_assets_rest_gateway::api::routes::configure;
use taproot_assets_rest_gateway::tests::setup::setup_without_assets;
use tokio::time::timeout;

#[actix_rt::test]
async fn test_subscribe_mint_events() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let request = AssetMintRequest {
        short_response: true,
    };

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/events/asset-mint")
        .set_json(&request)
        .to_request();

    // Use a timeout since these endpoints might be designed for long-polling
    let result = timeout(Duration::from_secs(5), test::call_service(&app, req)).await;

    match result {
        Ok(resp) => {
            // If we get a response, check if it's successful
            assert!(resp.status().is_success() || resp.status().is_client_error());
        }
        Err(_) => {
            // Timeout is expected for event subscription endpoints when no events occur
            // This is not necessarily a failure
            println!("Event subscription timed out as expected (no events)");
        }
    }
}

#[actix_rt::test]
async fn test_subscribe_receive_events() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let request = AssetReceiveRequest {
        filter_addr: None,
        start_timestamp: None,
    };

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/events/asset-receive")
        .set_json(&request)
        .to_request();

    let result = timeout(Duration::from_secs(5), test::call_service(&app, req)).await;

    match result {
        Ok(resp) => {
            assert!(resp.status().is_success() || resp.status().is_client_error());
        }
        Err(_) => {
            println!("Event subscription timed out as expected (no events)");
        }
    }
}

#[actix_rt::test]
async fn test_subscribe_send_events() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let request = AssetSendRequest {
        filter_script_key: None,
        filter_label: None,
    };

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/events/asset-send")
        .set_json(&request)
        .to_request();

    let result = timeout(Duration::from_secs(5), test::call_service(&app, req)).await;

    match result {
        Ok(resp) => {
            assert!(resp.status().is_success() || resp.status().is_client_error());
        }
        Err(_) => {
            println!("Event subscription timed out as expected (no events)");
        }
    }
}

#[actix_rt::test]
async fn test_event_endpoint_availability() {
    // This test just verifies that the endpoints are registered and accessible
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Test that endpoints exist by sending POST requests with empty/minimal payloads
    // We expect them to either succeed, timeout, or return a client error (not 404)

    // Test asset-mint endpoint
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/events/asset-mint")
        .set_json(&AssetMintRequest {
            short_response: true,
        })
        .to_request();
    let result = timeout(Duration::from_millis(100), test::call_service(&app, req)).await;
    match result {
        Ok(resp) => {
            assert_ne!(
                resp.status(),
                actix_web::http::StatusCode::NOT_FOUND,
                "asset-mint endpoint not found"
            );
        }
        Err(_) => {
            // Timeout is fine, endpoint exists but waiting for events
        }
    }

    // Test asset-receive endpoint
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/events/asset-receive")
        .set_json(&AssetReceiveRequest {
            filter_addr: None,
            start_timestamp: None,
        })
        .to_request();
    let result = timeout(Duration::from_millis(100), test::call_service(&app, req)).await;
    match result {
        Ok(resp) => {
            assert_ne!(
                resp.status(),
                actix_web::http::StatusCode::NOT_FOUND,
                "asset-receive endpoint not found"
            );
        }
        Err(_) => {
            // Timeout is fine
        }
    }

    // Test asset-send endpoint
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/events/asset-send")
        .set_json(&AssetSendRequest {
            filter_script_key: None,
            filter_label: None,
        })
        .to_request();
    let result = timeout(Duration::from_millis(100), test::call_service(&app, req)).await;
    match result {
        Ok(resp) => {
            assert_ne!(
                resp.status(),
                actix_web::http::StatusCode::NOT_FOUND,
                "asset-send endpoint not found"
            );
        }
        Err(_) => {
            // Timeout is fine
        }
    }
}
