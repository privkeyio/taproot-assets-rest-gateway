use crate::{
    config::Config,
    middleware::{RateLimiter, RequestIdMiddleware},
    types::{BaseUrl, MacaroonHex},
    websocket::{
        connection_manager::WebSocketConnectionManager, proxy_handler::WebSocketProxyHandler,
    },
};
use actix_cors::Cors;
use actix_web::middleware::Logger;
use actix_web::{web, App, HttpServer};
use reqwest::Client;
use std::fs;
use std::sync::Arc;
use std::time::Duration;
use tracing_subscriber::{fmt, EnvFilter};

const MAX_PAYLOAD_SIZE: usize = 10 * 1024 * 1024;

mod api;
mod config;
pub mod connection_pool;
pub mod crypto;
pub mod database;
mod error;
mod middleware;
pub mod monitoring;
mod types;
mod websocket;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize tracing subscriber for structured logging
    let subscriber = fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");

    // Load environment configuration
    dotenv::from_filename(".env").ok();

    // Load and validate configuration
    let config = Config::load().expect("Failed to load configuration");

    // Read and encode macaroon for authentication
    let macaroon_bytes = fs::read(&config.macaroon_path)?;
    let macaroon_hex = hex::encode(macaroon_bytes);

    // Build base URL for backend communication
    let base_url = format!("https://{}", config.taproot_assets_host);

    // Create HTTP client with security settings
    let mut client_builder =
        Client::builder().timeout(Duration::from_secs(config.request_timeout_secs));

    // Only disable TLS verification if explicitly configured (development only)
    if !config.tls_verify {
        tracing::warn!("TLS verification is disabled - this should only be used in development!");
        client_builder = client_builder.danger_accept_invalid_certs(true);
    }

    let client = client_builder.build().expect("Failed to build HTTP client");

    // Create WebSocket infrastructure
    let ws_base_url = base_url
        .replace("https://", "wss://")
        .replace("http://", "ws://");
    let connection_manager = Arc::new(WebSocketConnectionManager::new(
        BaseUrl(ws_base_url),
        MacaroonHex(macaroon_hex.clone()),
        config.tls_verify,
    ));
    let ws_proxy_handler = Arc::new(WebSocketProxyHandler::new(connection_manager));

    let server_address = config.server_address.clone();
    let cors_origins = config.cors_origins.clone();
    let rate_limit = config.rate_limit_per_minute;

    println!("🚀 Starting Taproot Assets API Proxy");
    println!("📍 Server address: http://{server_address}");
    println!("🔗 Backend: {}", config.taproot_assets_host);
    println!(
        "🔒 TLS verification: {}",
        if config.tls_verify {
            "enabled"
        } else {
            "DISABLED ⚠️"
        }
    );
    println!("🌐 CORS origins: {cors_origins:?}");
    println!("⏱️  Request timeout: {}s", config.request_timeout_secs);
    println!("🚦 Rate limit: {rate_limit} req/min per IP");

    HttpServer::new({
        let ws_proxy_handler = ws_proxy_handler.clone();
        move || {
            // Configure CORS with dynamic origins
            let mut cors = Cors::default()
                .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
                .allowed_headers(vec![
                    actix_web::http::header::AUTHORIZATION,
                    actix_web::http::header::ACCEPT,
                    actix_web::http::header::CONTENT_TYPE,
                ])
                .supports_credentials()
                .max_age(3600);

            // Add each configured origin
            for origin in &cors_origins {
                cors = cors.allowed_origin(origin);
            }

            App::new()
                .wrap(cors)
                .wrap(RateLimiter::new(rate_limit))
                .wrap(RequestIdMiddleware)
                .wrap(Logger::new(
                    "%a \"%r\" %s %b \"%{Referer}i\" \"%{User-Agent}i\" %T",
                ))
                .app_data(web::PayloadConfig::new(MAX_PAYLOAD_SIZE))
                .app_data(web::JsonConfig::default().limit(MAX_PAYLOAD_SIZE))
                .app_data(web::Data::new(client.clone()))
                .app_data(web::Data::new(BaseUrl(base_url.clone())))
                .app_data(web::Data::new(MacaroonHex(macaroon_hex.clone())))
                .app_data(web::Data::new(config.clone()))
                .app_data(web::Data::new(ws_proxy_handler.clone()))
                .configure(api::routes::configure)
        }
    })
    .workers(num_cpus())
    .bind(&server_address)?
    .shutdown_timeout(30) // 30 second graceful shutdown
    .run()
    .await
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(2)
        .clamp(2, 16)
}
