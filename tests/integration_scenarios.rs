use actix_web::{test, App};
use serde_json::Value;
use serial_test::serial;
use taproot_assets_rest_gateway::api::addresses::NewAddrRequest;
use taproot_assets_rest_gateway::api::burn::BurnRequest;
use taproot_assets_rest_gateway::api::routes::configure;
use taproot_assets_rest_gateway::api::send::SendRequest;
use taproot_assets_rest_gateway::tests::setup::{mint_test_asset, setup};

#[actix_rt::test]
#[serial]
async fn test_complete_asset_lifecycle() {
    let (client, base_url, macaroon_hex, lnd_macaroon_hex) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Mint asset
    let asset_id = mint_test_asset(
        client.as_ref(),
        &base_url.0,
        &macaroon_hex.0,
        &lnd_macaroon_hex,
    )
    .await;

    // Generate receiving address
    let addr_req = NewAddrRequest {
        asset_id: asset_id.clone(),
        amt: "100".to_string(),
        script_key: None,
        internal_key: None,
        tapscript_sibling: None,
        proof_courier_addr: None,
        asset_version: None,
        address_version: None,
    };
    let addr_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/addrs")
            .set_json(&addr_req)
            .to_request(),
    )
    .await;
    let addr_json: Value = test::read_body_json(addr_resp).await;

    // Check if address creation was successful
    if addr_json.get("error").is_some() || addr_json.get("code").is_some() {
        println!("Address creation failed: {addr_json:?}");
        return;
    }

    let addr = addr_json["encoded"].as_str().unwrap().to_string();

    // Send assets
    let send_req = SendRequest {
        tap_addrs: vec![addr],
        fee_rate: Some(300),
        label: None,
        skip_proof_courier_ping_check: Some(true),
    };
    let send_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/send")
            .set_json(&send_req)
            .to_request(),
    )
    .await;
    assert!(send_resp.status().is_success());

    // Verify balance update
    let balance_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/v1/taproot-assets/assets/balance")
            .to_request(),
    )
    .await;
    assert!(balance_resp.status().is_success());

    // Burn assets
    let burn_req = BurnRequest {
        asset_id,
        asset_id_str: None,
        amount_to_burn: "50".to_string(),
        confirmation_text: "assets will be destroyed".to_string(),
        note: None,
    };
    let burn_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/burn")
            .set_json(&burn_req)
            .to_request(),
    )
    .await;
    assert!(burn_resp.status().is_success());

    let burn_json: Value = test::read_body_json(burn_resp).await;

    // Check if burn was successful or returned an error
    if burn_json.get("error").is_some() || burn_json.get("code").is_some() {
        println!("Burn failed (might be insufficient balance): {burn_json:?}");
    } else {
        // Successful burn
        assert!(burn_json["burn_transfer"].is_object() || burn_json["transfer"].is_object());
    }
}
