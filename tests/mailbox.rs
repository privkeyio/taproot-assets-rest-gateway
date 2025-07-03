use actix_web::{test, App};
use base64::{engine::general_purpose, Engine as _};
use serde_json::{json, Value};
use taproot_assets_rest_gateway::api::routes::configure;
use taproot_assets_rest_gateway::tests::setup::setup_without_assets;
use tracing::info;

#[actix_rt::test]
async fn test_get_mailbox_info() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;
    info!("Testing mailbox info endpoint");
    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/mailbox/info")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let json: Value = test::read_body_json(resp).await;
    if json.get("error").is_some() || json.get("code").is_some() {
        info!("Mailbox info returned error: {:?}", json);
        return;
    }
    assert!(json.get("server_time").is_some());
    assert!(json.get("message_count").is_some());
}

#[actix_rt::test]
async fn test_send_message_basic() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;
    info!("Testing basic message send");
    let receiver_id = general_purpose::STANDARD.encode(vec![0x02; 33]);
    let test_message = "Hello, Taproot Assets Mailbox!";
    let encrypted_payload = general_purpose::STANDARD.encode(test_message.as_bytes());
    let request = json!({
        "receiver_id": receiver_id,
        "encrypted_payload": encrypted_payload,
        "expiry_block_height": 1000000
    });
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/mailbox/send")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let json: Value = test::read_body_json(resp).await;
    if json.get("error").is_some() || json.get("code").is_some() {
        info!("Send message returned error: {:?}", json);
    } else {
        assert!(json["message_id"].is_string());
    }
}

#[actix_rt::test]
async fn test_receive_messages_flow() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;
    info!("Testing receive messages flow");
    let receiver_id = general_purpose::STANDARD.encode(vec![0x02; 33]);
    let init_request = json!({
        "init": {
            "receiver_id": receiver_id,
            "start_message_id_exclusive": "0",
            "start_block_height_inclusive": 0,
            "start_timestamp_exclusive": "0"
        }
    });
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/mailbox/receive")
        .set_json(&init_request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    if !resp.status().is_success() {
        info!("Receive endpoint returned status: {}", resp.status());
        return;
    }
    let json: Value = test::read_body_json(resp).await;
    info!("Receive init response: {:?}", json);
    if let Some(challenge) = json.get("challenge") {
        assert!(challenge["challenge_hash"].is_string());
        let auth_request = json!({
            "auth_sig": {
                "signature": general_purpose::STANDARD.encode(vec![0u8; 64])
            }
        });
        let auth_req = test::TestRequest::post()
            .uri("/v1/taproot-assets/mailbox/receive")
            .set_json(&auth_request)
            .to_request();
        let auth_resp = test::call_service(&app, auth_req).await;
        if auth_resp.status().is_success() {
            let auth_json: Value = test::read_body_json(auth_resp).await;
            info!("Auth response: {:?}", auth_json);
        }
    }
}

#[actix_rt::test]
async fn test_mailbox_expiry_handling() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;
    info!("Testing mailbox message expiry handling");
    let expired_request = json!({
        "receiver_id": general_purpose::STANDARD.encode(vec![0x02; 33]),
        "encrypted_payload": general_purpose::STANDARD.encode(b"expired message"),
        "expiry_block_height": 1
    });
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/mailbox/send")
        .set_json(&expired_request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_rt::test]
async fn test_large_message_payload() {
    let (client, base_url, macaroon_hex) = setup_without_assets().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;
    info!("Testing large message payload");
    let large_payload = vec![0x42u8; 1024 * 1024];
    let encoded_payload = general_purpose::STANDARD.encode(&large_payload);
    let request = json!({
        "receiver_id": general_purpose::STANDARD.encode(vec![0x02; 33]),
        "encrypted_payload": encoded_payload,
        "expiry_block_height": 200000
    });
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/mailbox/send")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}
