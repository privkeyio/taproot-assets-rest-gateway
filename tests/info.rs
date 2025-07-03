use actix_web::{test, App};
use serial_test::serial;
use taproot_assets_rest_gateway::api::routes::configure;
use taproot_assets_rest_gateway::tests::setup::setup;

#[actix_rt::test]
#[serial]
async fn test_get_info() {
    let (client, base_url, macaroon_hex, _lnd_macaroon_hex) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;
    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/getinfo")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let json: serde_json::Value = test::read_body_json(resp).await;
    assert!(json["block_height"].as_u64().is_some());
}
