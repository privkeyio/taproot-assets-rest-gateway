use actix_web::{test, App};
use serde_json::Value;
use std::fs;
use std::path::Path;
use taproot_assets_rest_gateway::api::routes::configure;
use taproot_assets_rest_gateway::config::Config;
use taproot_assets_rest_gateway::tests::setup::setup_without_assets;

#[actix_rt::test]
async fn test_verify_connection_to_taproot_daemon() {
    // Ensure .env.local is loaded for this test - do this before any other operations
    dotenv::from_filename(".env.local").ok();

    // Force reload environment to ensure TLS_VERIFY is properly set
    std::env::set_var("TLS_VERIFY", "false");

    let (client, base_url, macaroon_hex) = setup_without_assets().await;

    // Test direct connection to daemon using the properly configured client
    let url = format!("{}/v1/taproot-assets/getinfo", base_url.0);
    let response = client
        .get_ref()
        .get(&url)
        .header("Grpc-Metadata-macaroon", &macaroon_hex.0)
        .send()
        .await
        .expect("Failed to connect to daemon");

    assert!(response.status().is_success());
    let json: Value = response.json().await.expect("Failed to parse response");
    assert!(json["version"].is_string());
    assert!(json["lnd_version"].is_string());
    assert!(json["network"].is_string());
}

#[actix_rt::test]
async fn test_macaroon_authentication() {
    // Ensure .env.local is loaded for this test
    dotenv::from_filename(".env.local").ok();
    std::env::set_var("TLS_VERIFY", "false");
    let (client, base_url, _macaroon_hex) = setup_without_assets().await;

    // Test with invalid macaroon
    let invalid_macaroon = "0000000000000000000000000000000000000000";
    let url = format!("{}/v1/taproot-assets/getinfo", base_url.0);
    let response = client
        .get_ref()
        .get(&url)
        .header("Grpc-Metadata-macaroon", invalid_macaroon)
        .send()
        .await
        .expect("Failed to send request");

    assert!(response.status().is_client_error() || response.status().is_server_error());

    // Test without macaroon
    let response_no_auth = client
        .get_ref()
        .get(&url)
        .send()
        .await
        .expect("Failed to send request");

    assert!(
        response_no_auth.status().is_client_error() || response_no_auth.status().is_server_error()
    );
}

#[actix_rt::test]
async fn test_ssl_tls_certificate_handling() {
    // Ensure .env.local is loaded for this test
    dotenv::from_filename(".env.local").ok();
    std::env::set_var("TLS_VERIFY", "false");

    // Use the setup() function which properly configures TLS based on .env.local
    let (client, base_url, macaroon_hex) = setup_without_assets().await;

    // Should connect successfully with the properly configured client
    let url = format!("{}/v1/taproot-assets/getinfo", base_url.0);
    let response = client
        .get_ref()
        .get(&url)
        .header("Grpc-Metadata-macaroon", &macaroon_hex.0)
        .send()
        .await
        .expect("Failed to connect");

    assert!(response.status().is_success());

    // Additionally test that the config is loaded correctly
    let config = Config::load().expect("Failed to load config");

    // Verify TLS_VERIFY is properly loaded from .env.local
    assert!(
        !config.tls_verify,
        "TLS_VERIFY should be false as set in .env.local"
    );
}

#[actix_rt::test]
async fn test_health_check_endpoint() {
    // Ensure .env.local is loaded for this test
    dotenv::from_filename(".env.local").ok();
    std::env::set_var("TLS_VERIFY", "false");

    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let req = test::TestRequest::get().uri("/health").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    assert_eq!(json["status"], "healthy");
    assert!(json["timestamp"].is_string());

    // Verify timestamp format
    let timestamp = json["timestamp"].as_str().unwrap();
    assert!(timestamp.contains("T")); // RFC3339 format
}

#[actix_rt::test]
async fn test_readiness_probe_validates_connectivity() {
    // Ensure .env.local is loaded for this test
    dotenv::from_filename(".env.local").ok();
    std::env::set_var("TLS_VERIFY", "false");

    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    let req = test::TestRequest::get().uri("/readiness").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    assert_eq!(json["status"], "ready");
    assert_eq!(json["services"]["taproot_assets"], "up");
}

#[actix_rt::test]
async fn test_configuration_loading() {
    // Test config loading from environment
    dotenv::from_filename(".env.local").ok();
    let config = Config::load();
    assert!(config.is_ok());

    let config = config.unwrap();
    assert!(!config.taproot_assets_host.is_empty());
    assert!(Path::new(&config.macaroon_path).exists());
    assert!(Path::new(&config.lnd_macaroon_path).exists());
}

#[actix_rt::test]
async fn test_polar_network_connectivity() {
    // Ensure .env.local is loaded for this test
    dotenv::from_filename(".env.local").ok();
    std::env::set_var("TLS_VERIFY", "false");

    // Use the proper setup function that respects TLS_VERIFY from .env.local
    let (client, base_url, macaroon_hex) = setup_without_assets().await;

    // Verify we're connecting to Polar regtest
    let url = format!("{}/v1/taproot-assets/getinfo", base_url.0);
    let response = client
        .get_ref()
        .get(&url)
        .header("Grpc-Metadata-macaroon", &macaroon_hex.0)
        .send()
        .await
        .expect("Failed to connect");

    let json: Value = response.json().await.expect("Failed to parse response");
    assert_eq!(json["network"].as_str(), Some("regtest"));
}

#[actix_rt::test]
async fn test_base_url_format() {
    // Ensure .env.local is loaded for this test
    dotenv::from_filename(".env.local").ok();
    std::env::set_var("TLS_VERIFY", "false");

    let (_client, base_url, _macaroon_hex) = setup_without_assets().await;

    // Verify base URL is properly formatted
    assert!(base_url.0.starts_with("https://"));
    assert!(base_url.0.contains("127.0.0.1") || base_url.0.contains("localhost"));
    assert!(base_url.0.contains(":8289")); // Default Taproot Assets REST port
}

#[actix_rt::test]
async fn test_macaroon_hex_encoding() {
    // Load .env.local first
    dotenv::from_filename(".env.local").ok();

    let config = Config::load().expect("Failed to load config");
    let macaroon_bytes = fs::read(&config.macaroon_path).expect("Failed to read macaroon");
    let macaroon_hex = hex::encode(&macaroon_bytes);

    // Verify hex encoding
    assert_eq!(macaroon_hex.len(), macaroon_bytes.len() * 2);
    assert!(macaroon_hex.chars().all(|c| c.is_ascii_hexdigit()));

    // Verify we can decode back
    let decoded = hex::decode(&macaroon_hex).expect("Failed to decode hex");
    assert_eq!(decoded, macaroon_bytes);
}

#[actix_rt::test]
async fn test_concurrent_requests() {
    // Ensure .env.local is loaded for this test
    dotenv::from_filename(".env.local").ok();
    std::env::set_var("TLS_VERIFY", "false");

    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Send multiple concurrent requests
    let futures: Vec<_> = (0..5)
        .map(|_| {
            let req = test::TestRequest::get().uri("/health").to_request();
            test::call_service(&app, req)
        })
        .collect();

    let results = futures::future::join_all(futures).await;

    // All should succeed
    for resp in results {
        assert!(resp.status().is_success());
    }
}

#[actix_rt::test]
async fn test_cors_configuration() {
    // Load .env.local first
    dotenv::from_filename(".env.local").ok();

    let config = Config::load().expect("Failed to load config");

    // Verify CORS origins are loaded
    assert!(!config.cors_origins.is_empty());

    // Verify default origins are included
    let default_origins = ["http://localhost:5173", "http://127.0.0.1:5173"];
    for origin in &default_origins {
        assert!(
            config.cors_origins.iter().any(|o| o == origin),
            "Expected CORS origin {origin} not found"
        );
    }
}

#[actix_rt::test]
async fn test_tls_security_configuration() {
    // Load .env.local first to get the actual TLS_VERIFY value
    dotenv::from_filename(".env.local").ok();

    // Store the original value
    let original_tls_verify = std::env::var("TLS_VERIFY").ok();

    // Test with explicit true setting
    std::env::set_var("TLS_VERIFY", "true");
    let config = Config::load().expect("Failed to load config");
    assert!(
        config.tls_verify,
        "TLS verification should be enabled when set to true"
    );

    // Test with explicit false setting
    std::env::set_var("TLS_VERIFY", "false");
    let config = Config::load().expect("Failed to load config");
    assert!(
        !config.tls_verify,
        "TLS verification should be disabled when set to false"
    );

    // Restore original value
    match original_tls_verify {
        Some(val) => std::env::set_var("TLS_VERIFY", val),
        None => std::env::remove_var("TLS_VERIFY"),
    }
}
