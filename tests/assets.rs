use actix_web::{test, App};
use serial_test::serial;
use taproot_assets_rest_gateway::api::assets::{MintAsset, MintAssetRequest};
use taproot_assets_rest_gateway::api::routes::configure;
use taproot_assets_rest_gateway::tests::setup::{mint_test_asset, setup};
use uuid::Uuid;

#[actix_rt::test]
#[serial]
async fn test_create_normal_asset() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;
    let asset_name = format!("test-normal-asset-{}", Uuid::new_v4());
    let request = MintAssetRequest {
        asset: MintAsset {
            asset_type: "NORMAL".to_string(),
            name: asset_name,
            amount: "1000".to_string(),
        },
        short_response: true,
    };
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/assets")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let json: serde_json::Value = test::read_body_json(resp).await;
    println!("Mint response: {json:?}");
    assert!(json["pending_batch"].is_object());
}

#[actix_rt::test]
#[serial]
async fn test_list_assets() {
    let (client, base_url, macaroon_hex, lnd_macaroon_hex) = setup().await;
    mint_test_asset(
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
    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/assets")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let json: serde_json::Value = test::read_body_json(resp).await;

    // The response should have assets, unconfirmed_transfers, and unconfirmed_mints fields
    assert!(
        json["assets"].is_array(),
        "Expected assets field to be an array"
    );
    let assets = json["assets"].as_array().unwrap();
    assert!(!assets.is_empty(), "Expected at least one asset");
}

#[actix_rt::test]
#[serial]
async fn test_get_balance() {
    let (client, base_url, macaroon_hex, lnd_macaroon_hex) = setup().await;
    mint_test_asset(
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
    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/assets/balance")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}
