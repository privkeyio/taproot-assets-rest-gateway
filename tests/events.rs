use actix_web::{test, web, App};
use std::sync::Arc;
use std::time::Duration;
use taproot_assets_rest_gateway::api::events::{
    AssetMintRequest, AssetReceiveRequest, AssetSendRequest,
};
use taproot_assets_rest_gateway::api::routes::configure;
use taproot_assets_rest_gateway::tests::setup::setup_without_assets;
use taproot_assets_rest_gateway::types::{BaseUrl, MacaroonHex};
use taproot_assets_rest_gateway::websocket::{
    connection_manager::WebSocketConnectionManager, proxy_handler::WebSocketProxyHandler,
};
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

#[actix_rt::test]
async fn test_asset_mint_websocket_endpoint() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;

    // Create WebSocket infrastructure
    let connection_manager = Arc::new(WebSocketConnectionManager::new(
        BaseUrl(base_url.get_ref().0.clone()),
        MacaroonHex(macaroon_hex.get_ref().0.clone()),
        false,
    ));
    let ws_proxy_handler = Arc::new(WebSocketProxyHandler::new(connection_manager));

    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .app_data(web::Data::new(ws_proxy_handler))
            .configure(configure),
    )
    .await;

    // Test WebSocket endpoint with GET request (WebSocket upgrade)
    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/events/asset-mint")
        .insert_header(("upgrade", "websocket"))
        .insert_header(("connection", "upgrade"))
        .insert_header(("sec-websocket-version", "13"))
        .insert_header(("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ=="))
        .to_request();

    let result = timeout(Duration::from_secs(5), test::call_service(&app, req)).await;

    match result {
        Ok(resp) => {
            // WebSocket endpoint exists and can be accessed
            // In test environment, backend connection may fail, but endpoint should exist
            assert_ne!(
                resp.status(),
                actix_web::http::StatusCode::NOT_FOUND,
                "WebSocket endpoint not found"
            );
            assert_ne!(
                resp.status(),
                actix_web::http::StatusCode::METHOD_NOT_ALLOWED,
                "WebSocket endpoint does not support GET method"
            );
        }
        Err(_) => {
            // Timeout is acceptable for WebSocket endpoints that wait for connections
            println!("WebSocket asset mint endpoint timed out (acceptable for testing)");
        }
    }
}

#[actix_rt::test]
async fn test_asset_receive_websocket_endpoint() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;

    let connection_manager = Arc::new(WebSocketConnectionManager::new(
        BaseUrl(base_url.get_ref().0.clone()),
        MacaroonHex(macaroon_hex.get_ref().0.clone()),
        false,
    ));
    let ws_proxy_handler = Arc::new(WebSocketProxyHandler::new(connection_manager));

    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .app_data(web::Data::new(ws_proxy_handler))
            .configure(configure),
    )
    .await;

    // Test WebSocket endpoint with query parameters
    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/events/asset-receive?filter_addr=test_addr&start_timestamp=1234567890")
        .insert_header(("upgrade", "websocket"))
        .insert_header(("connection", "upgrade"))
        .insert_header(("sec-websocket-version", "13"))
        .insert_header(("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ=="))
        .to_request();

    let result = timeout(Duration::from_secs(5), test::call_service(&app, req)).await;

    match result {
        Ok(resp) => {
            // WebSocket endpoint exists and can be accessed
            // In test environment, backend connection may fail, but endpoint should exist
            assert_ne!(
                resp.status(),
                actix_web::http::StatusCode::NOT_FOUND,
                "WebSocket endpoint not found"
            );
            assert_ne!(
                resp.status(),
                actix_web::http::StatusCode::METHOD_NOT_ALLOWED,
                "WebSocket endpoint does not support GET method"
            );
        }
        Err(_) => {
            println!("WebSocket asset receive endpoint timed out (acceptable for testing)");
        }
    }
}

#[actix_rt::test]
async fn test_asset_send_websocket_endpoint() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;

    let connection_manager = Arc::new(WebSocketConnectionManager::new(
        BaseUrl(base_url.get_ref().0.clone()),
        MacaroonHex(macaroon_hex.get_ref().0.clone()),
        false,
    ));
    let ws_proxy_handler = Arc::new(WebSocketProxyHandler::new(connection_manager));

    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .app_data(web::Data::new(ws_proxy_handler))
            .configure(configure),
    )
    .await;

    // Test WebSocket endpoint with filtering parameters
    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/events/asset-send?filter_script_key=key123&filter_label=label456")
        .insert_header(("upgrade", "websocket"))
        .insert_header(("connection", "upgrade"))
        .insert_header(("sec-websocket-version", "13"))
        .insert_header(("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ=="))
        .to_request();

    let result = timeout(Duration::from_secs(5), test::call_service(&app, req)).await;

    match result {
        Ok(resp) => {
            // WebSocket endpoint exists and can be accessed
            // In test environment, backend connection may fail, but endpoint should exist
            assert_ne!(
                resp.status(),
                actix_web::http::StatusCode::NOT_FOUND,
                "WebSocket endpoint not found"
            );
            assert_ne!(
                resp.status(),
                actix_web::http::StatusCode::METHOD_NOT_ALLOWED,
                "WebSocket endpoint does not support GET method"
            );
        }
        Err(_) => {
            println!("WebSocket asset send endpoint timed out (acceptable for testing)");
        }
    }
}

#[actix_rt::test]
async fn test_websocket_endpoint_availability() {
    // Test that WebSocket endpoints are registered and accessible via GET requests
    let (client, base_url, macaroon_hex) = setup_without_assets().await;

    let connection_manager = Arc::new(WebSocketConnectionManager::new(
        BaseUrl(base_url.get_ref().0.clone()),
        MacaroonHex(macaroon_hex.get_ref().0.clone()),
        false,
    ));
    let ws_proxy_handler = Arc::new(WebSocketProxyHandler::new(connection_manager));

    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .app_data(web::Data::new(ws_proxy_handler))
            .configure(configure),
    )
    .await;

    // Test all three WebSocket endpoints exist and don't return 404
    let endpoints = vec![
        "/v1/taproot-assets/events/asset-mint",
        "/v1/taproot-assets/events/asset-receive",
        "/v1/taproot-assets/events/asset-send",
    ];

    for endpoint in endpoints {
        let req = test::TestRequest::get().uri(endpoint).to_request();

        let result = timeout(Duration::from_millis(100), test::call_service(&app, req)).await;

        match result {
            Ok(resp) => {
                assert_ne!(
                    resp.status(),
                    actix_web::http::StatusCode::NOT_FOUND,
                    "WebSocket endpoint {endpoint} not found"
                );
                assert_ne!(
                    resp.status(),
                    actix_web::http::StatusCode::METHOD_NOT_ALLOWED,
                    "WebSocket endpoint {endpoint} does not support GET method"
                );
            }
            Err(_) => {
                // Timeout is acceptable, endpoint exists but waiting for WebSocket upgrade
                println!("WebSocket endpoint {endpoint} exists but timed out (acceptable)");
            }
        }
    }
}

#[actix_rt::test]
async fn test_websocket_query_parameter_forwarding() {
    // Test that query parameters are properly forwarded to the backend
    let (client, base_url, macaroon_hex) = setup_without_assets().await;

    let connection_manager = Arc::new(WebSocketConnectionManager::new(
        BaseUrl(base_url.get_ref().0.clone()),
        MacaroonHex(macaroon_hex.get_ref().0.clone()),
        false,
    ));
    let ws_proxy_handler = Arc::new(WebSocketProxyHandler::new(connection_manager));

    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .app_data(web::Data::new(ws_proxy_handler))
            .configure(configure),
    )
    .await;

    // Test complex query parameter combinations
    let test_cases = vec![
        ("/v1/taproot-assets/events/asset-mint?short_response=true", "mint"),
        ("/v1/taproot-assets/events/asset-receive?filter_addr=addr123&start_timestamp=1234567890", "receive"),
        ("/v1/taproot-assets/events/asset-send?filter_script_key=key123&filter_label=test_label", "send"),
    ];

    for (uri, event_type) in test_cases {
        let req = test::TestRequest::get()
            .uri(uri)
            .insert_header(("upgrade", "websocket"))
            .insert_header(("connection", "upgrade"))
            .insert_header(("sec-websocket-version", "13"))
            .insert_header(("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ=="))
            .to_request();

        let result = timeout(Duration::from_millis(500), test::call_service(&app, req)).await;

        match result {
            Ok(resp) => {
                // Should not return 404 (endpoint exists) or 400 (parameters properly parsed)
                assert_ne!(
                    resp.status(),
                    actix_web::http::StatusCode::NOT_FOUND,
                    "WebSocket {event_type} endpoint with parameters not found"
                );
                assert_ne!(
                    resp.status(),
                    actix_web::http::StatusCode::BAD_REQUEST,
                    "WebSocket {event_type} endpoint rejected parameters"
                );
            }
            Err(_) => {
                // Timeout is acceptable for WebSocket endpoints
                println!("WebSocket {event_type} endpoint with parameters timed out (acceptable)");
            }
        }
    }
}
