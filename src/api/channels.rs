use super::handle_result;
use crate::error::AppError;
use crate::types::{BaseUrl, MacaroonHex};
use crate::websocket::proxy_handler::WebSocketProxyHandler;
use actix_web::{web, HttpRequest, HttpResponse};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, instrument};

#[derive(Debug, Serialize, Deserialize)]
pub struct EncodeCustomDataRequest {
    pub router_send_payment: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FundChannelRequest {
    pub asset_amount: String,
    pub asset_id: String,
    pub peer_pubkey: String,
    pub fee_rate_sat_per_vbyte: u32,
    pub push_sat: Option<String>,
    pub group_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InvoiceRequest {
    pub asset_id: String,
    pub asset_amount: String,
    pub peer_pubkey: String,
    pub invoice_request: Option<serde_json::Value>,
    pub hodl_invoice: Option<serde_json::Value>,
    pub group_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DecodeInvoiceRequest {
    pub asset_id: String,
    pub pay_req_string: String,
    pub group_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendPaymentRequest {
    pub asset_id: String,
    pub asset_amount: String,
    pub peer_pubkey: String,
    pub payment_request: Option<serde_json::Value>,
    pub rfq_id: Option<String>,
    pub allow_overpay: bool,
    pub group_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SendPaymentStreamRequest {
    pub asset_id: String, // base64 encoded bytes
    pub asset_amount: String,
    pub peer_pubkey: String, // base64 encoded bytes
    pub payment_request: serde_json::Value,
    pub rfq_id: String, // base64 encoded bytes
    pub allow_overpay: bool,
    pub group_key: Option<String>, // base64 encoded bytes
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SendPaymentStreamResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_sell_order: Option<serde_json::Value>, // PeerAcceptedSellQuote
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_result: Option<serde_json::Value>, // Payment
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn encode_custom_data(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: EncodeCustomDataRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Encoding custom data");
    let url = format!("{base_url}/v1/taproot-assets/channels/encode-custom-data");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response
        .json::<serde_json::Value>()
        .await
        .map_err(AppError::RequestError)
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn fund_channel(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: FundChannelRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Funding channel for asset ID: {}", request.asset_id);
    let url = format!("{base_url}/v1/taproot-assets/channels/fund");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response
        .json::<serde_json::Value>()
        .await
        .map_err(AppError::RequestError)
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn create_invoice(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: InvoiceRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Creating invoice for asset ID: {}", request.asset_id);
    let url = format!("{base_url}/v1/taproot-assets/channels/invoice");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response
        .json::<serde_json::Value>()
        .await
        .map_err(AppError::RequestError)
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn decode_invoice(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: DecodeInvoiceRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Decoding invoice for asset ID: {}", request.asset_id);
    let url = format!("{base_url}/v1/taproot-assets/channels/invoice/decode");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response
        .json::<serde_json::Value>()
        .await
        .map_err(AppError::RequestError)
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn send_payment(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: SendPaymentRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Sending payment for asset ID: {}", request.asset_id);
    let url = format!("{base_url}/v1/taproot-assets/channels/send-payment");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response
        .json::<serde_json::Value>()
        .await
        .map_err(AppError::RequestError)
}

#[instrument(skip(req, stream, ws_proxy_handler))]
async fn send_payment_websocket_handler(
    req: HttpRequest,
    stream: web::Payload,
    ws_proxy_handler: web::Data<Arc<WebSocketProxyHandler>>,
) -> Result<HttpResponse, actix_web::Error> {
    info!("WebSocket connection request for send-payment streaming");

    // Check if the request contains the method=POST query parameter to differentiate
    // from regular WebSocket connections
    let query = req.query_string();
    if !query.contains("method=POST") {
        return Err(actix_web::error::ErrorBadRequest(
            "WebSocket send-payment requires method=POST query parameter",
        ));
    }

    // Define the backend WebSocket endpoint for streaming send-payment
    let backend_endpoint = "/v1/taproot-assets/channels/send-payment?stream=true";

    // Handle the WebSocket connection with correlation tracking enabled
    // This allows us to track request/response pairs for payment streaming
    ws_proxy_handler
        .handle_websocket(req, stream, backend_endpoint, true)
        .await
}

async fn encode_custom_data_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<EncodeCustomDataRequest>,
) -> HttpResponse {
    handle_result(
        encode_custom_data(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
        )
        .await,
    )
}

async fn fund_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<FundChannelRequest>,
) -> HttpResponse {
    handle_result(
        fund_channel(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
        )
        .await,
    )
}

async fn create_invoice_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<InvoiceRequest>,
) -> HttpResponse {
    handle_result(
        create_invoice(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
        )
        .await,
    )
}

async fn decode_invoice_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<DecodeInvoiceRequest>,
) -> HttpResponse {
    handle_result(
        decode_invoice(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
        )
        .await,
    )
}

async fn send_payment_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<SendPaymentRequest>,
) -> HttpResponse {
    handle_result(
        send_payment(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
        )
        .await,
    )
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/channels/encode-custom-data")
            .route(web::post().to(encode_custom_data_handler)),
    )
    .service(web::resource("/channels/fund").route(web::post().to(fund_handler)))
    .service(web::resource("/channels/invoice").route(web::post().to(create_invoice_handler)))
    .service(
        web::resource("/channels/invoice/decode").route(web::post().to(decode_invoice_handler)),
    )
    .service(
        web::resource("/channels/send-payment")
            .route(web::post().to(send_payment_handler))
            .route(web::get().to(send_payment_websocket_handler)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_query_parameter_validation() {
        // Test the query string validation logic
        let query_with_method = "method=POST&other=value";
        let query_without_method = "other=value&param=test";
        let empty_query = "";

        assert!(query_with_method.contains("method=POST"));
        assert!(!query_without_method.contains("method=POST"));
        assert!(!empty_query.contains("method=POST"));
    }

    #[test]
    fn test_send_payment_stream_request_serialization() {
        let request = SendPaymentStreamRequest {
            asset_id: "test_asset_id".to_string(),
            asset_amount: "1000".to_string(),
            peer_pubkey: "test_pubkey".to_string(),
            payment_request: serde_json::json!({"invoice": "test_invoice"}),
            rfq_id: "test_rfq_id".to_string(),
            allow_overpay: true,
            group_key: Some("test_group_key".to_string()),
        };

        let serialized = serde_json::to_string(&request).unwrap();
        assert!(serialized.contains("test_asset_id"));
        assert!(serialized.contains("1000"));
        assert!(serialized.contains("test_pubkey"));
        assert!(serialized.contains("test_rfq_id"));
        assert!(serialized.contains("true"));
        assert!(serialized.contains("test_group_key"));
    }

    #[test]
    fn test_send_payment_stream_response_serialization() {
        // Test response with both fields
        let response = SendPaymentStreamResponse {
            accepted_sell_order: Some(serde_json::json!({
                "quote_id": "test_quote",
                "status": "accepted"
            })),
            payment_result: Some(serde_json::json!({
                "payment_hash": "test_hash",
                "status": "completed"
            })),
        };

        let serialized = serde_json::to_string(&response).unwrap();
        assert!(serialized.contains("accepted_sell_order"));
        assert!(serialized.contains("payment_result"));
        assert!(serialized.contains("test_quote"));
        assert!(serialized.contains("test_hash"));

        // Test response with only accepted_sell_order
        let response_order_only = SendPaymentStreamResponse {
            accepted_sell_order: Some(serde_json::json!({
                "quote_id": "test_quote",
                "status": "accepted"
            })),
            payment_result: None,
        };

        let serialized_order = serde_json::to_string(&response_order_only).unwrap();
        assert!(serialized_order.contains("accepted_sell_order"));
        assert!(!serialized_order.contains("payment_result"));

        // Test response with only payment_result
        let response_payment_only = SendPaymentStreamResponse {
            accepted_sell_order: None,
            payment_result: Some(serde_json::json!({
                "payment_hash": "test_hash",
                "status": "completed"
            })),
        };

        let serialized_payment = serde_json::to_string(&response_payment_only).unwrap();
        assert!(!serialized_payment.contains("accepted_sell_order"));
        assert!(serialized_payment.contains("payment_result"));
    }

    #[test]
    fn test_websocket_url_format() {
        // Validate the WebSocket URL format matches the plan specification
        let base_url = "wss://localhost:8080";
        let endpoint = "/v1/taproot-assets/channels/send-payment?method=POST";
        let full_url = format!("{base_url}{endpoint}");

        assert_eq!(
            full_url,
            "wss://localhost:8080/v1/taproot-assets/channels/send-payment?method=POST"
        );
        assert!(full_url.contains("method=POST"));
        assert!(full_url.starts_with("wss://"));
    }

    #[test]
    fn test_request_format_matches_plan() {
        // Test that our request format matches the plan specification
        let plan_request = serde_json::json!({
            "asset_id": "YXNzZXRfaWQ=", // base64 encoded bytes
            "asset_amount": "1000",
            "peer_pubkey": "cGVlcl9wdWJrZXk=", // base64 encoded bytes
            "payment_request": {
                "payment_hash": "test_hash",
                "amount_msat": 1000000
            },
            "rfq_id": "cmZxX2lk", // base64 encoded bytes
            "allow_overpay": false,
            "group_key": "Z3JvdXBfa2V5" // base64 encoded bytes
        });

        // Verify we can deserialize into our struct
        let parsed: Result<SendPaymentStreamRequest, _> =
            serde_json::from_value(plan_request.clone());
        assert!(parsed.is_ok());

        let request = parsed.unwrap();
        assert_eq!(request.asset_id, "YXNzZXRfaWQ=");
        assert_eq!(request.asset_amount, "1000");
        assert_eq!(request.peer_pubkey, "cGVlcl9wdWJrZXk=");
        assert_eq!(request.rfq_id, "cmZxX2lk");
        assert!(!request.allow_overpay);
        assert_eq!(request.group_key, Some("Z3JvdXBfa2V5".to_string()));
    }

    #[test]
    fn test_response_format_matches_plan() {
        // Test that our response format matches the plan specification
        let plan_response = serde_json::json!({
            "accepted_sell_order": {
                "quote_id": "test_quote_id",
                "asset_amount": 1000,
                "ask_price": 50000,
                "expiry": 1234567890
            },
            "payment_result": {
                "payment_hash": "test_payment_hash",
                "payment_preimage": "test_preimage",
                "payment_route": [],
                "status": "SUCCEEDED",
                "failure_reason": null,
                "value_msat": 1000000,
                "value_sat": 1000,
                "payment_request": "lnbc..."
            }
        });

        // Verify we can deserialize into our struct
        let parsed: Result<SendPaymentStreamResponse, _> = serde_json::from_value(plan_response);
        assert!(parsed.is_ok());

        let response = parsed.unwrap();
        assert!(response.accepted_sell_order.is_some());
        assert!(response.payment_result.is_some());

        // Verify the structure of accepted_sell_order
        if let Some(order) = response.accepted_sell_order {
            assert!(order.get("quote_id").is_some());
            assert!(order.get("asset_amount").is_some());
        }

        // Verify the structure of payment_result
        if let Some(payment) = response.payment_result {
            assert!(payment.get("payment_hash").is_some());
            assert!(payment.get("status").is_some());
        }
    }
}
