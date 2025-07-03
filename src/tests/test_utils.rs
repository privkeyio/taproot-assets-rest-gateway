use hex;
use reqwest::Client;
use std::fs;
use std::time::Duration;
use taproot_assets::config::Config;
use tracing::warn;

/// Create a raw reqwest::Client with proper TLS configuration for tests
pub async fn create_test_client() -> (Client, String, String, String) {
    // Load test environment
    dotenv::from_filename(".env").ok();
    let config = Config::load().expect("Failed to load test configuration");

    // Read and encode macaroons
    let macaroon_bytes =
        fs::read(&config.macaroon_path).expect("Failed to read tapd macaroon for tests");
    let macaroon_hex = hex::encode(macaroon_bytes);

    let lnd_macaroon_bytes =
        fs::read(&config.lnd_macaroon_path).expect("Failed to read LND macaroon for tests");
    let lnd_macaroon_hex = hex::encode(lnd_macaroon_bytes);

    let base_url = format!("https://{}", config.taproot_assets_host);

    // Create client with test configuration
    let mut client_builder = Client::builder().timeout(Duration::from_secs(60));

    // Only disable TLS verification if explicitly set for tests
    if !config.tls_verify {
        warn!("TLS verification disabled for tests");
        client_builder = client_builder.danger_accept_invalid_certs(true);
    }

    let client = client_builder.build().unwrap();

    (client, base_url, macaroon_hex, lnd_macaroon_hex)
}
