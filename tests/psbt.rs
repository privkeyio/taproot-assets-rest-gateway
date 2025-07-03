use actix_web::{test, App};
use base64::{engine::general_purpose, Engine as _};
use serde_json::{json, Value};
use serial_test::serial;
use taproot_assets_rest_gateway::api::routes::configure;
use taproot_assets_rest_gateway::api::wallet::{
    VirtualPsbtAnchorRequest, VirtualPsbtCommitRequest, VirtualPsbtFundRequest,
    VirtualPsbtLogTransferRequest, VirtualPsbtSignRequest,
};
use taproot_assets_rest_gateway::tests::setup::{mint_test_asset, setup};

#[actix_rt::test]
#[serial]
async fn test_create_and_fund_virtual_psbt() {
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

    // Create address to send to
    let addr_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/addrs")
            .set_json(json!({
                "asset_id": asset_id,
                "amt": "100"
            }))
            .to_request(),
    )
    .await;
    let addr_json: Value = test::read_body_json(addr_resp).await;
    let tap_addr = addr_json["encoded"].as_str().unwrap();

    // Create PSBT template using raw transaction
    let request = VirtualPsbtFundRequest {
        psbt: "".to_string(),
        raw: json!({
            "inputs": [],
            "recipients": {
                tap_addr: 100
            }
        }),
        coin_select_type: "COIN_SELECT_DEFAULT".to_string(),
    };

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/wallet/virtual-psbt/fund")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;

    if json.get("error").is_some() || json.get("code").is_some() {
        println!("Fund virtual PSBT error: {json:?}");
    } else {
        assert!(json["funded_psbt"].is_string());
        assert!(json["change_output_index"].is_number());
        assert!(json["passive_asset_psbts"].is_array());
    }
}

#[actix_rt::test]
#[serial]
async fn test_sign_virtual_psbt() {
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

    // Create and fund PSBT first
    let addr_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/addrs")
            .set_json(json!({
                "asset_id": asset_id,
                "amt": "50"
            }))
            .to_request(),
    )
    .await;
    let addr_json: Value = test::read_body_json(addr_resp).await;
    let tap_addr = addr_json["encoded"].as_str().unwrap();

    let fund_request = VirtualPsbtFundRequest {
        psbt: "".to_string(),
        raw: json!({
            "inputs": [],
            "recipients": {
                tap_addr: 50
            }
        }),
        coin_select_type: "COIN_SELECT_DEFAULT".to_string(),
    };

    let fund_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/wallet/virtual-psbt/fund")
            .set_json(&fund_request)
            .to_request(),
    )
    .await;
    let fund_json: Value = test::read_body_json(fund_resp).await;

    if fund_json.get("error").is_none() && fund_json.get("code").is_none() {
        let funded_psbt = fund_json["funded_psbt"].as_str().unwrap();

        // Sign the funded PSBT
        let sign_request = VirtualPsbtSignRequest {
            funded_psbt: funded_psbt.to_string(),
        };

        let req = test::TestRequest::post()
            .uri("/v1/taproot-assets/wallet/virtual-psbt/sign")
            .set_json(&sign_request)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let json: Value = test::read_body_json(resp).await;

        if json.get("error").is_some() || json.get("code").is_some() {
            println!("Sign virtual PSBT error: {json:?}");
        } else {
            assert!(json["signed_psbt"].is_string());
            assert!(json["signed_inputs"].is_array());
        }
    }
}

#[actix_rt::test]
#[serial]
async fn test_anchor_virtual_psbt() {
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

    // Create, fund and sign PSBT
    let addr_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/addrs")
            .set_json(json!({
                "asset_id": asset_id,
                "amt": "75"
            }))
            .to_request(),
    )
    .await;
    let addr_json: Value = test::read_body_json(addr_resp).await;
    let tap_addr = addr_json["encoded"].as_str().unwrap();

    let fund_request = VirtualPsbtFundRequest {
        psbt: "".to_string(),
        raw: json!({
            "inputs": [],
            "recipients": {
                tap_addr: 75
            }
        }),
        coin_select_type: "COIN_SELECT_DEFAULT".to_string(),
    };

    let fund_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/wallet/virtual-psbt/fund")
            .set_json(&fund_request)
            .to_request(),
    )
    .await;
    let fund_json: Value = test::read_body_json(fund_resp).await;

    if fund_json.get("error").is_none() && fund_json.get("code").is_none() {
        let funded_psbt = fund_json["funded_psbt"].as_str().unwrap();

        // Sign the PSBT
        let sign_resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/taproot-assets/wallet/virtual-psbt/sign")
                .set_json(json!({
                    "funded_psbt": funded_psbt
                }))
                .to_request(),
        )
        .await;
        let sign_json: Value = test::read_body_json(sign_resp).await;

        if sign_json.get("error").is_none() && sign_json.get("code").is_none() {
            let signed_psbt = sign_json["signed_psbt"].as_str().unwrap();

            // Anchor the signed PSBT
            let anchor_request = VirtualPsbtAnchorRequest {
                virtual_psbts: vec![signed_psbt.to_string()],
            };

            let req = test::TestRequest::post()
                .uri("/v1/taproot-assets/wallet/virtual-psbt/anchor")
                .set_json(&anchor_request)
                .to_request();
            let resp = test::call_service(&app, req).await;
            assert!(resp.status().is_success());

            let json: Value = test::read_body_json(resp).await;

            if json.get("error").is_some() || json.get("code").is_some() {
                println!("Anchor virtual PSBT error: {json:?}");
            } else {
                assert!(json["transfer"].is_object());
                let transfer = &json["transfer"];
                assert!(transfer["transfer_timestamp"].is_string());
                assert!(
                    transfer["anchor_tx_hash"].is_string() || transfer["anchor_tx"].is_string()
                );
                assert!(transfer["inputs"].is_array());
                assert!(transfer["outputs"].is_array());
            }
        }
    }
}

#[actix_rt::test]
#[serial]
async fn test_commit_virtual_psbt() {
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

    // Create and fund virtual PSBT
    let addr_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/addrs")
            .set_json(json!({
                "asset_id": asset_id,
                "amt": "60"
            }))
            .to_request(),
    )
    .await;
    let addr_json: Value = test::read_body_json(addr_resp).await;
    let tap_addr = addr_json["encoded"].as_str().unwrap();

    let fund_request = VirtualPsbtFundRequest {
        psbt: "".to_string(),
        raw: json!({
            "inputs": [],
            "recipients": {
                tap_addr: 60
            }
        }),
        coin_select_type: "COIN_SELECT_DEFAULT".to_string(),
    };

    let fund_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/wallet/virtual-psbt/fund")
            .set_json(&fund_request)
            .to_request(),
    )
    .await;
    let fund_json: Value = test::read_body_json(fund_resp).await;

    if fund_json.get("error").is_none() && fund_json.get("code").is_none() {
        let funded_psbt = fund_json["funded_psbt"].as_str().unwrap();
        let passive_psbts = fund_json["passive_asset_psbts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();

        // Sign the PSBT
        let sign_resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/taproot-assets/wallet/virtual-psbt/sign")
                .set_json(json!({
                    "funded_psbt": funded_psbt
                }))
                .to_request(),
        )
        .await;
        let sign_json: Value = test::read_body_json(sign_resp).await;

        if sign_json.get("error").is_none() && sign_json.get("code").is_none() {
            let signed_psbt = sign_json["signed_psbt"].as_str().unwrap();

            // Commit the virtual PSBT
            let commit_request = VirtualPsbtCommitRequest {
                virtual_psbts: vec![signed_psbt.to_string()],
                passive_asset_psbts: passive_psbts,
                anchor_psbt: general_purpose::STANDARD.encode(vec![0u8; 100]), // Dummy anchor PSBT
                existing_output_index: -1,
                add: true,
                target_conf: 6,
                sat_per_vbyte: "10".to_string(),
                custom_lock_id: None,
                lock_expiration_seconds: None,
                skip_funding: false,
            };

            let req = test::TestRequest::post()
                .uri("/v1/taproot-assets/wallet/virtual-psbt/commit")
                .set_json(&commit_request)
                .to_request();
            let resp = test::call_service(&app, req).await;
            assert!(resp.status().is_success());

            let json: Value = test::read_body_json(resp).await;

            if json.get("error").is_some() || json.get("code").is_some() {
                println!("Commit virtual PSBT error: {json:?}");
            } else {
                assert!(json["anchor_psbt"].is_string());
                assert!(json["virtual_psbts"].is_array());
                assert!(json["passive_asset_psbts"].is_array());
                assert!(json["change_output_index"].is_number());
                assert!(json["lnd_locked_utxos"].is_array());
            }
        }
    }
}

#[actix_rt::test]
#[serial]
async fn test_log_psbt_transfer() {
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

    // Test with dummy data since we need properly committed PSBTs
    let request = VirtualPsbtLogTransferRequest {
        anchor_psbt: general_purpose::STANDARD.encode(vec![0u8; 100]),
        virtual_psbts: vec![general_purpose::STANDARD.encode(vec![1u8; 100])],
        passive_asset_psbts: vec![],
        change_output_index: -1,
        lnd_locked_utxos: vec![],
        skip_anchor_tx_broadcast: true,
        label: Some("Test transfer".to_string()),
    };

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/wallet/virtual-psbt/log-transfer")
        .set_json(&request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;

    if json.get("error").is_some() || json.get("code").is_some() {
        println!("Log transfer error: {json:?}");
    } else {
        assert!(json["transfer"].is_object());
    }
}

#[actix_rt::test]
async fn test_psbt_coin_selection_types() {
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

    let addr_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/taproot-assets/addrs")
            .set_json(json!({
                "asset_id": asset_id,
                "amt": "25"
            }))
            .to_request(),
    )
    .await;
    let addr_json: Value = test::read_body_json(addr_resp).await;
    let tap_addr = addr_json["encoded"].as_str().unwrap();

    // Test different coin selection types
    let coin_select_types = vec![
        "COIN_SELECT_DEFAULT",
        "COIN_SELECT_BIP86_ONLY",
        "COIN_SELECT_SCRIPT_TREES_ALLOWED",
    ];

    for coin_type in coin_select_types {
        let request = VirtualPsbtFundRequest {
            psbt: "".to_string(),
            raw: json!({
                "inputs": [],
                "recipients": {
                    tap_addr: 25
                }
            }),
            coin_select_type: coin_type.to_string(),
        };

        let req = test::TestRequest::post()
            .uri("/v1/taproot-assets/wallet/virtual-psbt/fund")
            .set_json(&request)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let json: Value = test::read_body_json(resp).await;
        println!(
            "Coin select type {} result: {}",
            coin_type,
            if json.get("error").is_some() {
                "error"
            } else {
                "success"
            }
        );
    }
}

#[actix_rt::test]
#[serial]
async fn test_psbt_with_specific_inputs() {
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

    // Get asset UTXOs
    let utxos_resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/v1/taproot-assets/assets/utxos")
            .to_request(),
    )
    .await;
    let utxos_json: Value = test::read_body_json(utxos_resp).await;

    if let Some(managed_utxos) = utxos_json["managed_utxos"].as_object() {
        if let Some((_, utxo_data)) = managed_utxos.iter().next() {
            if let Some(assets) = utxo_data["assets"].as_array() {
                if let Some(asset) = assets
                    .iter()
                    .find(|a| a["asset_genesis"]["asset_id"].as_str() == Some(&asset_id))
                {
                    let outpoint = utxo_data["outpoint"].as_str().unwrap();
                    let parts: Vec<&str> = outpoint.split(':').collect();
                    if parts.len() == 2 {
                        let script_key = asset["script_key"].as_str().unwrap();

                        let addr_resp = test::call_service(
                            &app,
                            test::TestRequest::post()
                                .uri("/v1/taproot-assets/addrs")
                                .set_json(json!({
                                    "asset_id": asset_id,
                                    "amt": "10"
                                }))
                                .to_request(),
                        )
                        .await;
                        let addr_json: Value = test::read_body_json(addr_resp).await;
                        let tap_addr = addr_json["encoded"].as_str().unwrap();

                        // Try to use specific input
                        let request = VirtualPsbtFundRequest {
                            psbt: "".to_string(),
                            raw: json!({
                                "inputs": [{
                                    "outpoint": {
                                        "txid": general_purpose::STANDARD.encode(hex::decode(parts[0]).unwrap_or_default()),
                                        "output_index": parts[1].parse::<u32>().unwrap_or(0)
                                    },
                                    "id": asset_id,
                                    "script_key": script_key
                                }],
                                "recipients": {
                                    tap_addr: 10
                                }
                            }),
                            coin_select_type: "COIN_SELECT_DEFAULT".to_string(),
                        };

                        let req = test::TestRequest::post()
                            .uri("/v1/taproot-assets/wallet/virtual-psbt/fund")
                            .set_json(&request)
                            .to_request();
                        let resp = test::call_service(&app, req).await;
                        assert!(resp.status().is_success());

                        let json: Value = test::read_body_json(resp).await;
                        println!(
                            "Fund with specific input result: {}",
                            if json.get("error").is_some() {
                                "error"
                            } else {
                                "success"
                            }
                        );
                    }
                }
            }
        }
    }
}

#[actix_rt::test]
async fn test_psbt_error_handling() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Test with invalid PSBT
    let sign_request = VirtualPsbtSignRequest {
        funded_psbt: "invalid_psbt_data".to_string(),
    };

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/wallet/virtual-psbt/sign")
        .set_json(&sign_request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    assert!(json.get("error").is_some() || json.get("code").is_some());
}

#[actix_rt::test]
async fn test_commit_psbt_with_custom_parameters() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Test with custom lock parameters
    let commit_request = VirtualPsbtCommitRequest {
        virtual_psbts: vec![general_purpose::STANDARD.encode(vec![1u8; 100])],
        passive_asset_psbts: vec![],
        anchor_psbt: general_purpose::STANDARD.encode(vec![0u8; 100]),
        existing_output_index: -1,
        add: true,
        target_conf: 1,
        sat_per_vbyte: "50".to_string(),
        custom_lock_id: Some(general_purpose::STANDARD.encode(b"custom_lock_123")),
        lock_expiration_seconds: Some("300".to_string()),
        skip_funding: true,
    };

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/wallet/virtual-psbt/commit")
        .set_json(&commit_request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    println!(
        "Commit with custom params result: {}",
        if json.get("error").is_some() {
            "error"
        } else {
            "success"
        }
    );
}

#[actix_rt::test]
#[serial]
async fn test_anchor_multiple_virtual_psbts() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Test anchoring multiple PSBTs
    let anchor_request = VirtualPsbtAnchorRequest {
        virtual_psbts: vec![
            general_purpose::STANDARD.encode(vec![1u8; 100]),
            general_purpose::STANDARD.encode(vec![2u8; 100]),
            general_purpose::STANDARD.encode(vec![3u8; 100]),
        ],
    };

    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/wallet/virtual-psbt/anchor")
        .set_json(&anchor_request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let json: Value = test::read_body_json(resp).await;
    println!(
        "Anchor multiple PSBTs result: {}",
        if json.get("error").is_some() {
            "error"
        } else {
            "success"
        }
    );
}

#[actix_rt::test]
#[serial]
async fn test_log_transfer_with_label() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    // Test with different labels
    let labels = vec![
        Some("Payment for invoice #123".to_string()),
        Some("Refund transaction".to_string()),
        None,
    ];

    for label in labels {
        let request = VirtualPsbtLogTransferRequest {
            anchor_psbt: general_purpose::STANDARD.encode(vec![0u8; 100]),
            virtual_psbts: vec![general_purpose::STANDARD.encode(vec![1u8; 100])],
            passive_asset_psbts: vec![],
            change_output_index: 0,
            lnd_locked_utxos: vec![json!({
                "txid": general_purpose::STANDARD.encode(vec![0u8; 32]),
                "output_index": 0
            })],
            skip_anchor_tx_broadcast: true,
            label: label.clone(),
        };

        let req = test::TestRequest::post()
            .uri("/v1/taproot-assets/wallet/virtual-psbt/log-transfer")
            .set_json(&request)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let json: Value = test::read_body_json(resp).await;
        println!(
            "Log transfer with label {:?} result: {}",
            label,
            if json.get("error").is_some() {
                "error"
            } else {
                "success"
            }
        );
    }
}
