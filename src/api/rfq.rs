use super::handle_result;
use crate::error::AppError;
use crate::types::{BaseUrl, MacaroonHex};
use actix_web::{web, HttpRequest, HttpResponse, Result as ActixResult};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{info, instrument};

#[derive(Debug, Serialize, Deserialize)]
pub struct BuyOfferRequest {
    pub asset_specifier: serde_json::Value,
    pub max_units: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuyOrderRequest {
    pub asset_specifier: serde_json::Value,
    pub asset_max_amt: String,
    pub expiry: String,
    pub peer_pub_key: String,
    pub timeout_seconds: u32,
    pub skip_asset_channel_check: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SellOfferRequest {
    pub asset_specifier: serde_json::Value,
    pub max_units: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SellOrderRequest {
    pub asset_specifier: serde_json::Value,
    pub payment_max_amt: String,
    pub expiry: String,
    pub peer_pub_key: String,
    pub timeout_seconds: u32,
    pub skip_asset_channel_check: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryAssetRatesRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_specifier: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_amount: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_amount: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_rate_hint: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_oracle_metadata: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_timestamp: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResolveRequestRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buy_request: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sell_request: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyAcceptQuoteRequest {
    pub accept: serde_json::Value,
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn buy_offer(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: BuyOfferRequest,
    asset_id: &str,
) -> Result<Value, AppError> {
    info!("Creating buy offer for asset ID: {}", asset_id);
    let url = format!("{base_url}/v1/taproot-assets/rfq/buyoffer/asset-id/{asset_id}");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response
        .json::<Value>()
        .await
        .map_err(AppError::RequestError)
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn buy_order(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: BuyOrderRequest,
    asset_id: &str,
) -> Result<Value, AppError> {
    info!("Creating buy order for asset ID: {}", asset_id);
    let url = format!("{base_url}/v1/taproot-assets/rfq/buyorder/asset-id/{asset_id}");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response
        .json::<Value>()
        .await
        .map_err(AppError::RequestError)
}

#[instrument(skip(client, macaroon_hex))]
pub async fn get_notifications(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<Value, AppError> {
    info!("Fetching RFQ notifications");
    let url = format!("{base_url}/v1/taproot-assets/rfq/ntfs");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response
        .json::<Value>()
        .await
        .map_err(AppError::RequestError)
}

#[instrument(skip(client, macaroon_hex))]
pub async fn get_asset_rates(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<Value, AppError> {
    info!("Fetching asset rates");
    let url = format!("{base_url}/v1/taproot-assets/rfq/priceoracle/assetrates");
    let response = client
        .get(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response
        .json::<Value>()
        .await
        .map_err(AppError::RequestError)
}

#[instrument(skip(client, macaroon_hex))]
pub async fn get_peer_quotes(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<Value, AppError> {
    info!("Fetching peer-accepted quotes");
    let url = format!("{base_url}/v1/taproot-assets/rfq/quotes/peeraccepted");
    let response = client
        .get(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response
        .json::<Value>()
        .await
        .map_err(AppError::RequestError)
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn sell_offer(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: SellOfferRequest,
    asset_id: &str,
) -> Result<Value, AppError> {
    info!("Creating sell offer for asset ID: {}", asset_id);
    let url = format!("{base_url}/v1/taproot-assets/rfq/selloffer/asset-id/{asset_id}");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response
        .json::<Value>()
        .await
        .map_err(AppError::RequestError)
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn sell_order(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: SellOrderRequest,
    asset_id: &str,
) -> Result<Value, AppError> {
    info!("Creating sell order for asset ID: {}", asset_id);
    let url = format!("{base_url}/v1/taproot-assets/rfq/sellorder/asset-id/{asset_id}");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response
        .json::<Value>()
        .await
        .map_err(AppError::RequestError)
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn portfoliopilot_asset_rates(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: QueryAssetRatesRequest,
) -> Result<Value, AppError> {
    info!("Querying portfoliopilot asset rates");
    let url = format!("{base_url}/v1/taproot-assets/rfq/portfoliopilot/assetrates");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response
        .json::<Value>()
        .await
        .map_err(AppError::RequestError)
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn portfoliopilot_resolve(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: ResolveRequestRequest,
) -> Result<Value, AppError> {
    info!("Resolving portfoliopilot request");
    let url = format!("{base_url}/v1/taproot-assets/rfq/portfoliopilot/resolve");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response
        .json::<Value>()
        .await
        .map_err(AppError::RequestError)
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn portfoliopilot_verify(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: VerifyAcceptQuoteRequest,
) -> Result<Value, AppError> {
    info!("Verifying portfoliopilot accepted quote");
    let url = format!("{base_url}/v1/taproot-assets/rfq/portfoliopilot/verify");
    let response = client
        .post(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&request)
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response
        .json::<Value>()
        .await
        .map_err(AppError::RequestError)
}

async fn portfoliopilot_asset_rates_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<QueryAssetRatesRequest>,
) -> HttpResponse {
    handle_result(
        portfoliopilot_asset_rates(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
        )
        .await,
    )
}

async fn portfoliopilot_resolve_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<ResolveRequestRequest>,
) -> HttpResponse {
    handle_result(
        portfoliopilot_resolve(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
        )
        .await,
    )
}

async fn portfoliopilot_verify_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<VerifyAcceptQuoteRequest>,
) -> HttpResponse {
    handle_result(
        portfoliopilot_verify(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
        )
        .await,
    )
}

async fn buy_offer_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    path: web::Path<String>,
    req: web::Json<BuyOfferRequest>,
) -> HttpResponse {
    let asset_id = path.into_inner();
    handle_result(
        buy_offer(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
            asset_id.as_str(),
        )
        .await,
    )
}

async fn buy_order_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    path: web::Path<String>,
    req: web::Json<BuyOrderRequest>,
) -> HttpResponse {
    let asset_id = path.into_inner();
    handle_result(
        buy_order(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
            asset_id.as_str(),
        )
        .await,
    )
}

async fn notifications_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    handle_result(
        get_notifications(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
        )
        .await,
    )
}

async fn rfq_events_ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    client: web::Data<Client>,
    config: web::Data<crate::config::Config>,
) -> ActixResult<HttpResponse> {
    info!("Establishing WebSocket connection for RFQ event notifications");

    let (response, session, mut msg_stream) = actix_ws::handle(&req, stream)?;

    let base_url_clone = base_url.0.clone();
    let macaroon_clone = macaroon_hex.0.clone();
    let client_clone = client.get_ref().clone();

    actix_web::rt::spawn(async move {
        use actix_ws::Message;
        use futures_util::StreamExt;
        use std::sync::Arc;
        use tokio::sync::Mutex;
        use tokio::time::{interval, Duration};

        let session = Arc::new(Mutex::new(session));

        // Send initial empty request body to start streaming
        {
            let mut session_lock = session.lock().await;
            if let Err(e) = session_lock.text("{}").await {
                tracing::error!("Failed to send initial message: {}", e);
                return;
            }
        }

        let mut ping_interval = interval(Duration::from_secs(30));

        // Start the polling task
        let poll_session = session.clone();
        let poll_client = client_clone.clone();
        let poll_base_url = base_url_clone.clone();
        let poll_macaroon = macaroon_clone.clone();

        let poll_interval = config.rfq_poll_interval_secs;
        let poll_task = actix_web::rt::spawn(async move {
            poll_rfq_events(
                &poll_client,
                &poll_base_url,
                &poll_macaroon,
                poll_session,
                poll_interval,
            )
            .await;
        });

        loop {
            tokio::select! {
                // Handle incoming messages from client
                msg = msg_stream.next() => {
                    match msg {
                        Some(Ok(Message::Text(_text))) => {
                            // For RFQ notifications, we typically just need to maintain the connection
                            // The streaming is handled by the initial POST request to tapd
                            tracing::debug!("Received client message for RFQ notifications");
                        },
                        Some(Ok(Message::Close(_))) => {
                            tracing::info!("WebSocket connection closed by client");
                            break;
                        },
                        Some(Ok(Message::Ping(bytes))) => {
                            let mut session_lock = session.lock().await;
                            if let Err(e) = session_lock.pong(&bytes).await {
                                tracing::error!("Failed to send pong: {}", e);
                                break;
                            }
                        },
                        Some(Err(e)) => {
                            tracing::error!("WebSocket error: {}", e);
                            break;
                        },
                        None => {
                            tracing::info!("WebSocket stream ended");
                            break;
                        },
                        _ => {}
                    }
                },
                // Send periodic pings to keep connection alive
                _ = ping_interval.tick() => {
                    let mut session_lock = session.lock().await;
                    if let Err(e) = session_lock.ping(b"ping").await {
                        tracing::error!("Failed to send ping: {}", e);
                        break;
                    }
                },
            }
        }

        // Abort the polling task when connection ends
        poll_task.abort();
    });

    Ok(response)
}

async fn poll_rfq_events(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    session: std::sync::Arc<tokio::sync::Mutex<actix_ws::Session>>,
    poll_interval_secs: u64,
) {
    use tokio::time::{sleep, Duration};

    loop {
        match get_notifications(client, base_url, macaroon_hex).await {
            Ok(events) => {
                let event_json =
                    serde_json::to_string(&events).unwrap_or_else(|_| "{}".to_string());
                let mut session_lock = session.lock().await;
                if let Err(e) = session_lock.text(event_json).await {
                    tracing::error!("Failed to send RFQ event: {}", e);
                    break;
                }
            }
            Err(e) => {
                tracing::error!("Failed to fetch RFQ notifications: {}", e);

                let error_msg = serde_json::json!({
                    "error": e.to_string(),
                    "type": "rfq_notification_error"
                });
                let mut session_lock = session.lock().await;
                if let Err(e) = session_lock.text(error_msg.to_string()).await {
                    tracing::error!("Failed to send error message: {}", e);
                    break;
                }
            }
        }

        // Wait before next poll
        sleep(Duration::from_secs(poll_interval_secs)).await;
    }
}

async fn asset_rates_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    handle_result(
        get_asset_rates(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
        )
        .await,
    )
}

async fn peer_quotes_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    handle_result(
        get_peer_quotes(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
        )
        .await,
    )
}

async fn sell_offer_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    path: web::Path<String>,
    req: web::Json<SellOfferRequest>,
) -> HttpResponse {
    let asset_id = path.into_inner();
    handle_result(
        sell_offer(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
            asset_id.as_str(),
        )
        .await,
    )
}

async fn sell_order_handler(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    path: web::Path<String>,
    req: web::Json<SellOrderRequest>,
) -> HttpResponse {
    let asset_id = path.into_inner();
    handle_result(
        sell_order(
            client.as_ref(),
            base_url.0.as_str(),
            macaroon_hex.0.as_str(),
            req.into_inner(),
            asset_id.as_str(),
        )
        .await,
    )
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/rfq/buyoffer/asset-id/{asset_id}").route(web::post().to(buy_offer_handler)),
    )
    .service(
        web::resource("/rfq/buyorder/asset-id/{asset_id}").route(web::post().to(buy_order_handler)),
    )
    .service(
        web::resource("/rfq/ntfs")
            .route(web::get().to(rfq_events_ws_handler))
            .route(web::post().to(notifications_handler)),
    )
    .service(
        web::resource("/rfq/portfoliopilot/assetrates")
            .route(web::post().to(portfoliopilot_asset_rates_handler)),
    )
    .service(
        web::resource("/rfq/portfoliopilot/resolve")
            .route(web::post().to(portfoliopilot_resolve_handler)),
    )
    .service(
        web::resource("/rfq/portfoliopilot/verify")
            .route(web::post().to(portfoliopilot_verify_handler)),
    )
    .service(web::resource("/rfq/priceoracle/assetrates").route(web::get().to(asset_rates_handler)))
    .service(web::resource("/rfq/quotes/peeraccepted").route(web::get().to(peer_quotes_handler)))
    .service(
        web::resource("/rfq/selloffer/asset-id/{asset_id}")
            .route(web::post().to(sell_offer_handler)),
    )
    .service(
        web::resource("/rfq/sellorder/asset-id/{asset_id}")
            .route(web::post().to(sell_order_handler)),
    );
}
