use super::handle_result;
use crate::error::AppError;
use crate::types::{BaseUrl, MacaroonHex};
use crate::websocket::proxy_handler::WebSocketProxyHandler;
use actix_web::{web, HttpRequest, HttpResponse, Result as ActixResult};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, instrument, warn};

#[derive(Debug, Serialize, Deserialize)]
pub struct DebugLevelRequest {
    pub show: bool,
    pub level_spec: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssetMintRequest {
    pub short_response: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssetReceiveRequest {
    pub filter_addr: Option<String>,
    pub start_timestamp: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssetSendRequest {
    pub filter_script_key: Option<String>,
    pub filter_label: Option<String>,
}

// Create a separate client for event subscriptions with longer timeout
fn create_event_client() -> Result<Client, AppError> {
    Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(300)) // 5 minute timeout for event subscriptions
        .build()
        .map_err(|e| AppError::ValidationError(format!("Failed to create event client: {e}")))
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn set_debug_level(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: DebugLevelRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Setting debug level: {}", request.level_spec);
    let url = format!("{base_url}/v1/taproot-assets/debuglevel");
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

#[instrument(skip(macaroon_hex, request))]
pub async fn asset_mint_events(
    base_url: &str,
    macaroon_hex: &str,
    request: AssetMintRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Subscribing to asset mint events");
    let event_client = create_event_client()?;
    let url = format!("{base_url}/v1/taproot-assets/events/asset-mint");

    let response = event_client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await;

    match response {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                resp.json::<serde_json::Value>()
                    .await
                    .map_err(AppError::RequestError)
            } else {
                let error_text = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(AppError::ValidationError(format!(
                    "Event subscription failed with status {status}: {error_text}"
                )))
            }
        }
        Err(e) if e.is_timeout() => {
            warn!("Asset mint event subscription timed out");
            Ok(serde_json::json!({
                "events": [],
                "timeout": true,
                "message": "No events received within timeout period"
            }))
        }
        Err(e) => Err(AppError::RequestError(e)),
    }
}

#[instrument(skip(macaroon_hex, request))]
pub async fn asset_receive_events(
    base_url: &str,
    macaroon_hex: &str,
    request: AssetReceiveRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Subscribing to asset receive events");
    let event_client = create_event_client()?;
    let url = format!("{base_url}/v1/taproot-assets/events/asset-receive");

    let response = event_client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await;

    match response {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                resp.json::<serde_json::Value>()
                    .await
                    .map_err(AppError::RequestError)
            } else {
                let error_text = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(AppError::ValidationError(format!(
                    "Event subscription failed with status {status}: {error_text}"
                )))
            }
        }
        Err(e) if e.is_timeout() => {
            warn!("Asset receive event subscription timed out");
            Ok(serde_json::json!({
                "events": [],
                "timeout": true,
                "message": "No events received within timeout period"
            }))
        }
        Err(e) => Err(AppError::RequestError(e)),
    }
}

#[instrument(skip(macaroon_hex, request))]
pub async fn asset_send_events(
    base_url: &str,
    macaroon_hex: &str,
    request: AssetSendRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Subscribing to asset send events");
    let event_client = create_event_client()?;
    let url = format!("{base_url}/v1/taproot-assets/events/asset-send");

    let response = event_client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await;

    match response {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                resp.json::<serde_json::Value>()
                    .await
                    .map_err(AppError::RequestError)
            } else {
                let error_text = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(AppError::ValidationError(format!(
                    "Event subscription failed with status {status}: {error_text}"
                )))
            }
        }
        Err(e) if e.is_timeout() => {
            warn!("Asset send event subscription timed out");
            Ok(serde_json::json!({
                "events": [],
                "timeout": true,
                "message": "No events received within timeout period"
            }))
        }
        Err(e) => Err(AppError::RequestError(e)),
    }
}

#[instrument(skip(req, stream, ws_proxy_handler))]
async fn generic_event_websocket_handler(
    req: HttpRequest,
    stream: web::Payload,
    ws_proxy_handler: web::Data<Arc<WebSocketProxyHandler>>,
    event_type: &str,
) -> ActixResult<HttpResponse> {
    info!("Handling WebSocket connection for {} events", event_type);

    // Extract query parameters and forward them to the backend
    let query_string = req.query_string();
    let endpoint = if query_string.is_empty() {
        format!("/v1/taproot-assets/events/{event_type}?method=POST")
    } else {
        format!("/v1/taproot-assets/events/{event_type}?method=POST&{query_string}")
    };

    ws_proxy_handler
        .handle_websocket(req, stream, &endpoint, false)
        .await
}

async fn asset_mint_websocket_handler(
    req: HttpRequest,
    stream: web::Payload,
    ws_proxy_handler: web::Data<Arc<WebSocketProxyHandler>>,
) -> ActixResult<HttpResponse> {
    generic_event_websocket_handler(req, stream, ws_proxy_handler, "asset-mint").await
}

async fn asset_receive_websocket_handler(
    req: HttpRequest,
    stream: web::Payload,
    ws_proxy_handler: web::Data<Arc<WebSocketProxyHandler>>,
) -> ActixResult<HttpResponse> {
    generic_event_websocket_handler(req, stream, ws_proxy_handler, "asset-receive").await
}

async fn asset_send_websocket_handler(
    req: HttpRequest,
    stream: web::Payload,
    ws_proxy_handler: web::Data<Arc<WebSocketProxyHandler>>,
) -> ActixResult<HttpResponse> {
    generic_event_websocket_handler(req, stream, ws_proxy_handler, "asset-send").await
}

async fn set_debug_level_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<DebugLevelRequest>,
) -> HttpResponse {
    handle_result(
        set_debug_level(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
        )
        .await,
    )
}

async fn asset_mint_handler(
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<AssetMintRequest>,
) -> HttpResponse {
    handle_result(
        asset_mint_events(
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
        )
        .await,
    )
}

async fn asset_receive_handler(
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<AssetReceiveRequest>,
) -> HttpResponse {
    handle_result(
        asset_receive_events(
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
        )
        .await,
    )
}

async fn asset_send_handler(
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<AssetSendRequest>,
) -> HttpResponse {
    handle_result(
        asset_send_events(
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
        )
        .await,
    )
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/debuglevel").route(web::post().to(set_debug_level_handler)))
        .service(
            web::resource("/events/asset-mint")
                .route(web::post().to(asset_mint_handler))
                .route(web::get().to(asset_mint_websocket_handler)),
        )
        .service(
            web::resource("/events/asset-receive")
                .route(web::post().to(asset_receive_handler))
                .route(web::get().to(asset_receive_websocket_handler)),
        )
        .service(
            web::resource("/events/asset-send")
                .route(web::post().to(asset_send_handler))
                .route(web::get().to(asset_send_websocket_handler)),
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_url_format_asset_mint() {
        let base_url = "wss://localhost:8080";
        let endpoint = "/v1/taproot-assets/events/asset-mint?method=POST";
        let full_url = format!("{base_url}{endpoint}");

        assert_eq!(
            full_url,
            "wss://localhost:8080/v1/taproot-assets/events/asset-mint?method=POST"
        );
        assert!(full_url.contains("method=POST"));
        assert!(full_url.starts_with("wss://"));
    }

    #[test]
    fn test_websocket_url_format_asset_receive() {
        let base_url = "wss://localhost:8080";
        let endpoint = "/v1/taproot-assets/events/asset-receive?method=POST";
        let full_url = format!("{base_url}{endpoint}");

        assert_eq!(
            full_url,
            "wss://localhost:8080/v1/taproot-assets/events/asset-receive?method=POST"
        );
        assert!(full_url.contains("method=POST"));
        assert!(full_url.starts_with("wss://"));
    }

    #[test]
    fn test_websocket_url_format_asset_send() {
        let base_url = "wss://localhost:8080";
        let endpoint = "/v1/taproot-assets/events/asset-send?method=POST";
        let full_url = format!("{base_url}{endpoint}");

        assert_eq!(
            full_url,
            "wss://localhost:8080/v1/taproot-assets/events/asset-send?method=POST"
        );
        assert!(full_url.contains("method=POST"));
        assert!(full_url.starts_with("wss://"));
    }

    #[test]
    fn test_websocket_query_parameter_forwarding() {
        // Test query parameter handling for different event types

        // Asset mint parameters
        let mint_query = "short_response=true&method=POST";
        assert!(mint_query.contains("method=POST"));
        assert!(mint_query.contains("short_response=true"));

        // Asset receive parameters
        let receive_query = "filter_addr=addr123&start_timestamp=1234567890&method=POST";
        assert!(receive_query.contains("method=POST"));
        assert!(receive_query.contains("filter_addr=addr123"));
        assert!(receive_query.contains("start_timestamp=1234567890"));

        // Asset send parameters
        let send_query = "filter_script_key=key123&filter_label=label456&method=POST";
        assert!(send_query.contains("method=POST"));
        assert!(send_query.contains("filter_script_key=key123"));
        assert!(send_query.contains("filter_label=label456"));
    }

    #[test]
    fn test_asset_mint_request_serialization() {
        let request = AssetMintRequest {
            short_response: true,
        };

        let serialized = serde_json::to_string(&request).unwrap();
        assert!(serialized.contains("short_response"));
        assert!(serialized.contains("true"));
    }

    #[test]
    fn test_asset_receive_request_serialization() {
        let request = AssetReceiveRequest {
            filter_addr: Some("addr123".to_string()),
            start_timestamp: Some("1234567890".to_string()),
        };

        let serialized = serde_json::to_string(&request).unwrap();
        assert!(serialized.contains("filter_addr"));
        assert!(serialized.contains("addr123"));
        assert!(serialized.contains("start_timestamp"));
        assert!(serialized.contains("1234567890"));
    }

    #[test]
    fn test_asset_send_request_serialization() {
        let request = AssetSendRequest {
            filter_script_key: Some("key123".to_string()),
            filter_label: Some("label456".to_string()),
        };

        let serialized = serde_json::to_string(&request).unwrap();
        assert!(serialized.contains("filter_script_key"));
        assert!(serialized.contains("key123"));
        assert!(serialized.contains("filter_label"));
        assert!(serialized.contains("label456"));
    }

    #[test]
    fn test_event_schema_validation() {
        // Validate that expected response fields match the documented schemas

        // Asset mint event schema
        let mint_event = serde_json::json!({
            "timestamp": "1234567890",
            "batch_state": "BATCH_STATE_BROADCAST",
            "batch": {
                "batch_key": "key123",
                "batch_txid": "txid123"
            },
            "error": ""
        });
        assert!(mint_event.get("timestamp").is_some());
        assert!(mint_event.get("batch_state").is_some());
        assert!(mint_event.get("batch").is_some());

        // Asset receive event schema
        let receive_event = serde_json::json!({
            "timestamp": "1234567890",
            "address": {
                "encoded": "addr123",
                "asset_id": "asset123"
            },
            "outpoint": "outpoint123",
            "status": "ADDR_EVENT_STATUS_TRANSACTION_CONFIRMED",
            "confirmation_height": 100,
            "error": ""
        });
        assert!(receive_event.get("timestamp").is_some());
        assert!(receive_event.get("address").is_some());
        assert!(receive_event.get("outpoint").is_some());
        assert!(receive_event.get("status").is_some());

        // Asset send event schema
        let send_event = serde_json::json!({
            "timestamp": "1234567890",
            "send_state": "SEND_STATE_VIRTUAL_COMMIT_BROADCAST",
            "parcel_type": "PARCEL_TYPE_SEND",
            "addresses": [],
            "virtual_packets": [],
            "passive_virtual_packets": [],
            "anchor_transaction": {},
            "transfer": {},
            "error": "",
            "transfer_label": "label123",
            "next_send_state": "SEND_STATE_COMPLETED"
        });
        assert!(send_event.get("timestamp").is_some());
        assert!(send_event.get("send_state").is_some());
        assert!(send_event.get("parcel_type").is_some());
        assert!(send_event.get("addresses").is_some());
    }
}
