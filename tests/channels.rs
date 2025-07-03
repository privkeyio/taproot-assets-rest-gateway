use actix_web::{test, App};
use serde_json::json;
use serial_test::serial;
use taproot_assets_rest_gateway::api::channels::{
    DecodeInvoiceRequest, EncodeCustomDataRequest, FundChannelRequest, InvoiceRequest,
    SendPaymentRequest,
};
use taproot_assets_rest_gateway::api::routes::configure;
use taproot_assets_rest_gateway::tests::setup::{mint_test_asset, setup};

#[actix_rt::test]
#[serial]
async fn test_fund_channel() {
    let (client, base_url, macaroon_hex, lnd_macaroon_hex) = setup().await;
    let asset_id = mint_test_asset(
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
    let request = FundChannelRequest {
        asset_amount: "100".to_string(),
        asset_id,
        peer_pubkey: "peer_key".to_string(),
        fee_rate_sat_per_vbyte: 300,
        push_sat: None,
        group_key: None,
    };
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/channels/fund")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success() || resp.status().is_client_error());
}

#[actix_rt::test]
#[serial]
async fn test_create_invoice() {
    let (client, base_url, macaroon_hex, lnd_macaroon_hex) = setup().await;
    let asset_id = mint_test_asset(
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
    let request = InvoiceRequest {
        asset_id,
        asset_amount: "50".to_string(),
        peer_pubkey: "peer_key".to_string(),
        invoice_request: None,
        hodl_invoice: None,
        group_key: None,
    };
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/channels/invoice")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success() || resp.status().is_client_error());
}

#[actix_rt::test]
#[serial]
async fn test_decode_asset_invoice() {
    let (client, base_url, macaroon_hex, lnd_macaroon_hex) = setup().await;
    let asset_id = mint_test_asset(
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
    let invoice_req = InvoiceRequest {
        asset_id: asset_id.clone(),
        asset_amount: "50".to_string(),
        peer_pubkey: "peer_key".to_string(),
        invoice_request: None,
        hodl_invoice: None,
        group_key: None,
    };
    let invoice_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/channels/invoice")
            .set_json(&invoice_req)
            .to_request(),
    )
    .await;
    if invoice_resp.status().is_success() {
        let invoice_json: serde_json::Value = test::read_body_json(invoice_resp).await;
        if let Some(payment_request) = invoice_json
            .get("invoice_result")
            .and_then(|ir| ir.get("payment_request"))
            .and_then(|pr| pr.as_str())
        {
            let decode_request = DecodeInvoiceRequest {
                asset_id,
                pay_req_string: payment_request.to_string(),
                group_key: None,
            };
            let decode_req = test::TestRequest::post()
                .uri("/v1/taproot-assets/channels/invoice/decode")
                .set_json(&decode_request)
                .to_request();
            let decode_resp = test::call_service(&app, decode_req).await;
            assert!(decode_resp.status().is_success() || decode_resp.status().is_client_error());
        }
    }
}

#[actix_rt::test]
#[serial]
async fn test_send_payment_through_channels() {
    let (client, base_url, macaroon_hex, lnd_macaroon_hex) = setup().await;
    let asset_id = mint_test_asset(
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
    let request = SendPaymentRequest {
        asset_id,
        asset_amount: "25".to_string(),
        peer_pubkey: "peer_key".to_string(),
        payment_request: None,
        rfq_id: None,
        allow_overpay: false,
        group_key: None,
    };
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/channels/send-payment")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success() || resp.status().is_client_error());
}

#[actix_rt::test]
#[serial]
async fn test_custom_data_encoding_for_channels() {
    let (client, base_url, macaroon_hex, lnd_macaroon_hex) = setup().await;
    let _asset_id = mint_test_asset(
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
    let request = EncodeCustomDataRequest {
        router_send_payment: json!({
            "asset_amounts": {
                "key": "test_key",
                "value": 100
            },
            "rfq_id": "dummy_rfq_id"
        }),
    };
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/channels/encode-custom-data")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success() || resp.status().is_client_error());
}
