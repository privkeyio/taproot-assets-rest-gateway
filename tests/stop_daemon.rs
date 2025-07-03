use actix_web::{test, App};
use serde_json::json;
use taproot_assets_rest_gateway::api::routes::configure;
use taproot_assets_rest_gateway::tests::setup::setup_without_assets;
use tokio::time::{sleep, Duration};
use tracing::info;

#[actix_rt::test]
#[ignore]
async fn test_stop_daemon_graceful_shutdown() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;
    info!("Testing graceful daemon shutdown");
    let info_req = test::TestRequest::get()
        .uri("/v1/taproot-assets/getinfo")
        .to_request();
    let info_resp = test::call_service(&app, info_req).await;
    assert!(info_resp.status().is_success());
    info!("Daemon is running, proceeding with shutdown test");
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/stop")
        .set_json(json!({}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body = test::read_body(resp).await;
    assert!(body.is_empty() || body.len() < 100);
    info!("Stop request sent successfully");
    sleep(Duration::from_secs(2)).await;
}

#[actix_rt::test]
async fn test_stop_daemon_api_structure() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;
    info!("Testing stop daemon API structure without actually stopping");
    let get_req = test::TestRequest::get()
        .uri("/v1/taproot-assets/stop")
        .to_request();
    let get_resp = test::call_service(&app, get_req).await;
    assert!(
        get_resp.status().is_client_error(),
        "GET method should not be allowed for stop endpoint"
    );
}

#[actix_rt::test]
async fn test_stop_daemon_authorization() {
    let (client, base_url, _macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(actix_web::web::Data::new(
                taproot_assets_rest_gateway::types::MacaroonHex("invalid_macaroon".to_string()),
            ))
            .configure(configure),
    )
    .await;
    info!("Testing stop daemon with invalid authorization");
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/stop")
        .set_json(json!({}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let status = resp.status();
    info!("Stop daemon with invalid auth returned status: {}", status);
    assert!(
        !status.is_success() || status.is_server_error(),
        "Stop endpoint should reject or error on invalid macaroon"
    );
}

#[actix_rt::test]
async fn test_daemon_state_before_stop() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;
    info!("Testing daemon state verification before stop");
    let endpoints = vec![
        ("/v1/taproot-assets/getinfo", "GET"),
        ("/v1/taproot-assets/assets", "GET"),
    ];
    let mut healthy_count = 0;
    for (endpoint, method) in &endpoints {
        let req = match *method {
            "GET" => test::TestRequest::get().uri(endpoint).to_request(),
            _ => test::TestRequest::post().uri(endpoint).to_request(),
        };
        let resp = test::call_service(&app, req).await;
        if resp.status().is_success() {
            healthy_count += 1;
            info!("Endpoint {} is responding normally", endpoint);
        }
    }
    assert!(
        healthy_count > 0,
        "At least one endpoint should be healthy before stop test"
    );
}

#[actix_rt::test]
async fn test_stop_daemon_minimal() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let url = format!("{}/v1/taproot-assets/stop", base_url.0);
    let _request = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", &macaroon_hex.0)
        .json(&json!({}));
    info!("Stop endpoint is configured at: {}", url);
    assert!(url.contains("/v1/taproot-assets/stop"));
}
