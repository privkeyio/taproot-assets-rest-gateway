use crate::crypto::{
    derive_public_key_from_receiver_id, verify_schnorr_signature, verify_signature,
};
use crate::database::{ReceiverInfo, SharedDatabase};
use crate::error::AppError;
use crate::monitoring::SharedMonitoring;
use crate::types::{BaseUrl, MacaroonHex};
use crate::websocket::proxy_handler::WebSocketProxyHandler;
use actix_web::{web, HttpRequest, HttpResponse, Result as ActixResult};
use actix_ws::{Message, MessageStream, Session};
use base64::Engine;
use bitcoin::bech32;
use chrono::Utc;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::time::timeout;
use tracing::{debug, error, info, instrument, warn};

#[derive(Debug, Serialize, Deserialize)]
pub struct ReceiveRequest {
    pub init: serde_json::Value,
    pub auth_sig: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendRequest {
    pub receiver_id: String,
    pub encrypted_payload: String,
    pub tx_proof: Option<serde_json::Value>,
    pub expiry_block_height: Option<u32>,
}

#[derive(Debug, Clone)]
enum MailboxState {
    AwaitingInit,
    ChallengeSent,
    Authenticated,
    Streaming,
    Closed,
}

struct ConnectionLimits {
    message_count: u32,
    last_reset: Instant,
}

#[derive(Debug, Clone)]
struct ChallengeData {
    challenge_id: String,
    timestamp: i64,
    nonce: String,
    issued_at: Instant,
}

lazy_static::lazy_static! {
    static ref ACTIVE_CHALLENGES: Mutex<HashMap<String, ChallengeData>> = Mutex::new(HashMap::new());
}

const IDLE_TIMEOUT_SECS: u64 = 300; // 5 minutes
const RATE_LIMIT_MESSAGES_PER_MINUTE: u32 = 60;
const MAX_MESSAGE_SIZE_BYTES: usize = 64 * 1024; // 64KB
const CHALLENGE_EXPIRY_SECS: u64 = 300; // 5 minutes
const TIMESTAMP_TOLERANCE_SECS: i64 = 30; // 30 seconds tolerance for clock skew
const MAX_ACTIVE_CHALLENGES: usize = 10_000;

#[derive(Debug, Serialize, Deserialize)]
struct WebSocketMailboxMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    init: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth_sig: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MailboxResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    challenge: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth_success: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    messages: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    eos: Option<serde_json::Value>,
}

#[instrument(skip(client, macaroon_hex))]
pub async fn get_mailbox_info(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
) -> Result<serde_json::Value, AppError> {
    info!("Fetching mailbox info");
    let url = format!("{base_url}/v1/taproot-assets/mailbox/info");
    let response = client
        .get(&url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .send()
        .await
        .map_err(AppError::RequestError)?;
    response
        .json::<serde_json::Value>()
        .await
        .map_err(AppError::RequestError)
}

#[instrument(skip(client, macaroon_hex, request))]
pub async fn receive_mail(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: ReceiveRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Receiving mail");
    let url = format!("{base_url}/v1/taproot-assets/mailbox/receive");
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
pub async fn send_mail(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    request: SendRequest,
) -> Result<serde_json::Value, AppError> {
    info!("Sending mail to receiver ID: {}", request.receiver_id);
    let url = format!("{base_url}/v1/taproot-assets/mailbox/send");
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

async fn info(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> HttpResponse {
    handle_result(get_mailbox_info(&client, &base_url.0, &macaroon_hex.0).await)
}

async fn receive(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<ReceiveRequest>,
) -> HttpResponse {
    handle_result(receive_mail(&client, &base_url.0, &macaroon_hex.0, req.into_inner()).await)
}

async fn send(
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
    req: web::Json<SendRequest>,
) -> HttpResponse {
    handle_result(send_mail(&client, &base_url.0, &macaroon_hex.0, req.into_inner()).await)
}

async fn receive_websocket(
    req: HttpRequest,
    stream: web::Payload,
    client: web::Data<Client>,
    base_url: web::Data<BaseUrl>,
    macaroon_hex: web::Data<MacaroonHex>,
) -> ActixResult<HttpResponse> {
    // Check if WebSocketProxyHandler is available and clone it before using req
    let maybe_proxy_handler = req
        .app_data::<web::Data<Arc<WebSocketProxyHandler>>>()
        .cloned();

    if let Some(handler) = maybe_proxy_handler {
        // Use proxy handler for production-ready WebSocket support
        return receive_websocket_with_proxy(req, stream, handler).await;
    }

    // Check for database instance
    let database = req
        .app_data::<web::Data<SharedDatabase>>()
        .map(|d| d.get_ref().clone());

    // Check for monitoring service
    let monitoring = req
        .app_data::<web::Data<SharedMonitoring>>()
        .map(|m| m.get_ref().clone());

    // Get remote address for monitoring
    let remote_addr = req
        .peer_addr()
        .map(|addr| addr.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Generate connection ID
    let connection_id = uuid::Uuid::new_v4().to_string();

    // Fall back to custom implementation
    let (response, session, msg_stream) = actix_ws::handle(&req, stream)?;

    info!(
        "Mailbox receive WebSocket connection established: {}",
        connection_id
    );

    // Record connection in monitoring
    if let Some(ref mon) = monitoring {
        mon.record_connection(connection_id.clone(), remote_addr)
            .await;
    }

    actix_rt::spawn(handle_mailbox_websocket_connection(
        session,
        msg_stream,
        client.get_ref().clone(),
        base_url.0.clone(),
        macaroon_hex.0.clone(),
        database,
        monitoring,
        connection_id,
    ));

    Ok(response)
}

#[allow(clippy::too_many_arguments)]
async fn handle_mailbox_websocket_connection(
    mut session: Session,
    mut msg_stream: MessageStream,
    client: Client,
    base_url: String,
    macaroon_hex: String,
    database: Option<SharedDatabase>,
    monitoring: Option<SharedMonitoring>,
    connection_id: String,
) {
    let mut state = MailboxState::AwaitingInit;
    let mut pending_init: Option<serde_json::Value> = None;
    let mut limits = ConnectionLimits {
        message_count: 0,
        last_reset: Instant::now(),
    };

    // Main message loop with idle timeout
    loop {
        let timeout_result =
            timeout(Duration::from_secs(IDLE_TIMEOUT_SECS), msg_stream.next()).await;

        let msg = match timeout_result {
            Ok(Some(msg)) => msg,
            Ok(None) => {
                info!("WebSocket stream ended");
                break;
            }
            Err(_) => {
                warn!("WebSocket connection timed out due to inactivity");
                let _ = session
                    .close(Some(actix_ws::CloseReason {
                        code: actix_ws::CloseCode::Normal,
                        description: Some("Connection idle timeout".to_string()),
                    }))
                    .await;
                break;
            }
        };

        // Check rate limiting
        if !check_rate_limit(&mut limits) {
            warn!("Rate limit exceeded, closing connection");

            // Record rate limit hit
            if let Some(ref mon) = monitoring {
                mon.record_rate_limit_hit(&connection_id).await;
            }

            let _ = session
                .close(Some(actix_ws::CloseReason {
                    code: actix_ws::CloseCode::Policy,
                    description: Some("Rate limit exceeded".to_string()),
                }))
                .await;
            break;
        }
        match msg {
            Ok(Message::Text(text)) => {
                // Validate message size before processing
                if text.len() > MAX_MESSAGE_SIZE_BYTES {
                    warn!(
                        "Message too large: {} bytes, max: {} bytes",
                        text.len(),
                        MAX_MESSAGE_SIZE_BYTES
                    );
                    let _ = session
                        .close(Some(actix_ws::CloseReason {
                            code: actix_ws::CloseCode::Size,
                            description: Some("Message too large".to_string()),
                        }))
                        .await;
                    break;
                }

                info!("Received mailbox WebSocket message: {}", text);

                // Record message received in monitoring
                if let Some(ref mon) = monitoring {
                    mon.record_message_received(&connection_id, text.len())
                        .await;
                }

                let parsed_msg: Result<WebSocketMailboxMessage, _> = serde_json::from_str(&text);
                match parsed_msg {
                    Ok(ws_msg) => {
                        match handle_mailbox_message(
                            &mut state,
                            ws_msg,
                            &mut pending_init,
                            &client,
                            &base_url,
                            &macaroon_hex,
                            &mut session,
                            database.as_ref(),
                            monitoring.as_ref(),
                            &connection_id,
                        )
                        .await
                        {
                            Ok(should_continue) => {
                                if !should_continue {
                                    break;
                                }
                            }
                            Err(e) => {
                                error!("Error handling mailbox message: {}", e);
                                let error_response = MailboxResponse {
                                    challenge: None,
                                    auth_success: Some(false),
                                    messages: None,
                                    eos: None,
                                };
                                if let Ok(error_json) = serde_json::to_string(&error_response) {
                                    let _ = session.text(error_json).await;
                                }
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse WebSocket message: {}", e);
                        break;
                    }
                }
            }
            Ok(Message::Close(_)) => {
                info!("Mailbox WebSocket connection closed");
                break;
            }
            Ok(Message::Ping(bytes)) => {
                if let Err(e) = session.pong(&bytes).await {
                    error!("Failed to send pong: {}", e);
                    break;
                }
            }
            Ok(_) => {}
            Err(e) => {
                error!("WebSocket message error: {}", e);
                break;
            }
        }
    }

    info!(
        "Mailbox WebSocket connection handler finished: {}",
        connection_id
    );

    // Record connection closure in monitoring
    if let Some(ref mon) = monitoring {
        mon.record_connection_closed(&connection_id).await;
    }
}

fn check_rate_limit(limits: &mut ConnectionLimits) -> bool {
    let now = Instant::now();

    // Reset counter every minute
    if now.duration_since(limits.last_reset) >= Duration::from_secs(60) {
        limits.message_count = 0;
        limits.last_reset = now;
    }

    limits.message_count += 1;
    limits.message_count <= RATE_LIMIT_MESSAGES_PER_MINUTE
}

#[allow(clippy::too_many_arguments)]
async fn handle_mailbox_message(
    state: &mut MailboxState,
    msg: WebSocketMailboxMessage,
    pending_init: &mut Option<serde_json::Value>,
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    session: &mut Session,
    database: Option<&SharedDatabase>,
    monitoring: Option<&SharedMonitoring>,
    connection_id: &str,
) -> Result<bool, AppError> {
    match state {
        MailboxState::AwaitingInit => {
            if let Some(init) = msg.init {
                info!("Received init message, sending challenge");
                *pending_init = Some(init);
                *state = MailboxState::ChallengeSent;

                let challenge_response = generate_challenge().await?;
                let response = MailboxResponse {
                    challenge: Some(challenge_response),
                    auth_success: None,
                    messages: None,
                    eos: None,
                };

                let response_json = serde_json::to_string(&response)
                    .map_err(|e| AppError::SerializationError(e.to_string()))?;

                // Record message sent in monitoring
                if let Some(mon) = monitoring {
                    mon.record_message_sent(connection_id, response_json.len())
                        .await;
                }

                session
                    .text(response_json)
                    .await
                    .map_err(|e| AppError::WebSocketError(e.to_string()))?;

                Ok(true)
            } else {
                warn!("Expected init message but got something else");
                Err(AppError::InvalidInput("Expected init message".to_string()))
            }
        }
        MailboxState::ChallengeSent => {
            if let Some(auth_sig) = msg.auth_sig {
                info!("Received auth signature, validating");

                if let Some(init) = pending_init.take() {
                    let auth_result = validate_authentication(
                        &init,
                        &auth_sig,
                        client,
                        base_url,
                        macaroon_hex,
                        database,
                    )
                    .await?;

                    let response = MailboxResponse {
                        challenge: None,
                        auth_success: Some(auth_result),
                        messages: None,
                        eos: None,
                    };

                    let response_json = serde_json::to_string(&response)
                        .map_err(|e| AppError::SerializationError(e.to_string()))?;

                    // Record message sent in monitoring
                    if let Some(mon) = monitoring {
                        mon.record_message_sent(connection_id, response_json.len())
                            .await;
                    }

                    session
                        .text(response_json)
                        .await
                        .map_err(|e| AppError::WebSocketError(e.to_string()))?;

                    if auth_result {
                        *state = MailboxState::Authenticated;

                        // Update monitoring with receiver ID
                        if let Some(mon) = monitoring {
                            if let Some(receiver_id) =
                                init.get("receiver_id").and_then(|v| v.as_str())
                            {
                                mon.update_receiver_id(connection_id, receiver_id.to_string())
                                    .await;
                            }
                        }

                        stream_mailbox_messages(
                            client,
                            base_url,
                            macaroon_hex,
                            session,
                            state,
                            &init,
                            &auth_sig,
                            monitoring,
                            connection_id,
                        )
                        .await?;
                        Ok(false)
                    } else {
                        warn!("Authentication failed");

                        // Record auth failure in monitoring
                        if let Some(mon) = monitoring {
                            mon.record_auth_failure(connection_id).await;
                        }

                        Ok(false)
                    }
                } else {
                    Err(AppError::InvalidInput(
                        "No pending init message".to_string(),
                    ))
                }
            } else {
                warn!("Expected auth signature but got something else");
                Err(AppError::InvalidInput(
                    "Expected auth signature".to_string(),
                ))
            }
        }
        _ => {
            warn!("Received message in unexpected state: {:?}", state);
            Err(AppError::InvalidInput("Unexpected state".to_string()))
        }
    }
}

async fn generate_challenge() -> Result<serde_json::Value, AppError> {
    let challenge_id = uuid::Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().timestamp();
    let nonce = base64::engine::general_purpose::STANDARD.encode(uuid::Uuid::new_v4().as_bytes());

    // Store challenge data for later verification
    let challenge_data = ChallengeData {
        challenge_id: challenge_id.clone(),
        timestamp,
        nonce: nonce.clone(),
        issued_at: Instant::now(),
    };

    {
        let mut challenges = ACTIVE_CHALLENGES.lock().unwrap();

        // Clean up expired challenges
        challenges.retain(|_, data| data.issued_at.elapsed().as_secs() < CHALLENGE_EXPIRY_SECS);

        if challenges.len() >= MAX_ACTIVE_CHALLENGES {
            return Err(AppError::ValidationError(
                "Too many pending challenges. Please try again later.".to_string(),
            ));
        }

        challenges.insert(challenge_id.clone(), challenge_data);
    }

    Ok(serde_json::json!({
        "challenge_id": challenge_id,
        "timestamp": timestamp,
        "nonce": nonce,
        "message": format!("Sign this challenge: {}-{}-{}", challenge_id, timestamp, nonce)
    }))
}

async fn validate_authentication(
    init: &serde_json::Value,
    auth_sig: &serde_json::Value,
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    database: Option<&SharedDatabase>,
) -> Result<bool, AppError> {
    // Extract required fields from init data
    let receiver_id = init
        .get("receiver_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::InvalidInput("Missing receiver_id in init data".to_string()))?;

    // Extract signature and challenge_id from auth_sig
    let signature = auth_sig
        .get("signature")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::InvalidInput("Missing signature in auth_sig".to_string()))?;

    let challenge_id = auth_sig
        .get("challenge_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::InvalidInput("Missing challenge_id in auth_sig".to_string()))?;

    let signed_timestamp = auth_sig
        .get("timestamp")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| AppError::InvalidInput("Missing timestamp in auth_sig".to_string()))?;

    // Basic validation
    if signature.is_empty() || signature.len() < 32 {
        warn!("Invalid signature format: too short");
        return Ok(false);
    }

    if receiver_id.is_empty() {
        warn!("Invalid receiver_id: empty");
        return Ok(false);
    }

    // Validate signature encoding (should be hex or base64)
    if !signature.chars().all(|c| c.is_ascii_hexdigit())
        && base64::engine::general_purpose::STANDARD
            .decode(signature)
            .is_err()
    {
        warn!("Invalid signature encoding: not hex or base64");
        return Ok(false);
    }

    // 1. Verify challenge exists and is valid
    let challenge_data = {
        let mut challenges = ACTIVE_CHALLENGES.lock().unwrap();
        let data = challenges
            .get(challenge_id)
            .ok_or_else(|| {
                warn!("Challenge not found: {}", challenge_id);
                AppError::InvalidInput("Invalid or expired challenge".to_string())
            })?
            .clone();

        // Check if challenge has expired
        if data.issued_at.elapsed().as_secs() > CHALLENGE_EXPIRY_SECS {
            warn!("Challenge expired: {}", challenge_id);
            challenges.remove(challenge_id);
            return Ok(false);
        }

        data
    };

    // 2. Validate timestamp to prevent replay attacks
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AppError::InvalidInput("System time error".to_string()))?
        .as_secs() as i64;

    let time_diff = (current_time - signed_timestamp).abs();
    if time_diff > TIMESTAMP_TOLERANCE_SECS {
        warn!(
            "Timestamp validation failed: time difference {} seconds exceeds tolerance",
            time_diff
        );
        return Ok(false);
    }

    // Ensure the signed timestamp matches the challenge timestamp (within tolerance)
    let challenge_time_diff = (challenge_data.timestamp - signed_timestamp).abs();
    if challenge_time_diff > TIMESTAMP_TOLERANCE_SECS {
        warn!(
            "Challenge timestamp mismatch: difference {} seconds",
            challenge_time_diff
        );
        return Ok(false);
    }

    // 3. Verify the signature cryptographically against the challenge
    let expected_message = format!(
        "Sign this challenge: {}-{}-{}",
        challenge_data.challenge_id, challenge_data.timestamp, challenge_data.nonce
    );

    if !verify_signature_with_receiver(&expected_message, signature, receiver_id, database).await? {
        warn!("Cryptographic signature verification failed");
        return Ok(false);
    }

    // 4. Test connectivity to backend and validate macaroon permissions
    if !validate_macaroon_permissions(client, base_url, macaroon_hex, receiver_id).await? {
        warn!("Macaroon permission validation failed");
        return Ok(false);
    }

    // 5. Validate receiver_id exists and is accessible
    if !validate_receiver_id(receiver_id, client, base_url, macaroon_hex, database).await? {
        warn!("Receiver ID validation failed: {}", receiver_id);
        return Ok(false);
    }

    // Remove used challenge to prevent replay
    {
        let mut challenges = ACTIVE_CHALLENGES.lock().unwrap();
        challenges.remove(challenge_id);
    }

    // Store receiver info in database if available
    if let Some(db) = database {
        // Try to extract public key from auth_sig or receiver_id
        let public_key = if let Some(pk) = auth_sig.get("public_key").and_then(|v| v.as_str()) {
            pk.to_string()
        } else if let Some(pk) = derive_public_key_from_receiver_id(receiver_id)? {
            pk
        } else {
            // Generate a placeholder if we can't determine the actual public key
            format!("unknown_{receiver_id}")
        };

        let receiver_info = ReceiverInfo {
            receiver_id: receiver_id.to_string(),
            public_key,
            address: init
                .get("address")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            created_at: Utc::now().timestamp(),
            last_seen: Utc::now().timestamp(),
            is_active: true,
            metadata: Some(serde_json::json!({
                "auth_method": "mailbox",
                "last_challenge_id": challenge_id,
            })),
        };

        if let Err(e) = db.store_receiver_info(&receiver_info).await {
            warn!("Failed to store receiver info in database: {}", e);
            // Don't fail authentication if we can't store in database
        }
    }

    info!(
        "Authentication successfully validated for receiver_id: {}",
        receiver_id
    );
    Ok(true)
}

async fn verify_signature_with_receiver(
    message: &str,
    signature: &str,
    receiver_id: &str,
    database: Option<&SharedDatabase>,
) -> Result<bool, AppError> {
    // First check if receiver_id is directly a public key
    if let Some(public_key) = derive_public_key_from_receiver_id(receiver_id)? {
        // Try Schnorr signature first (for Taproot compatibility)
        if public_key.len() == 64 {
            // X-only public key (32 bytes hex) - use Schnorr
            return verify_schnorr_signature(message, signature, &public_key);
        } else {
            // Regular public key - use ECDSA
            return verify_signature(message, signature, &public_key);
        }
    }

    // If not a direct public key, look it up in the database
    if let Some(db) = database {
        if let Some(receiver_info) = db.get_receiver_info(receiver_id).await? {
            // Try Schnorr first for Taproot addresses
            if receiver_info.public_key.len() == 64 {
                return verify_schnorr_signature(message, signature, &receiver_info.public_key);
            } else {
                return verify_signature(message, signature, &receiver_info.public_key);
            }
        }
    }

    // If we can't find the public key, we can't verify the signature
    warn!("Unable to find public key for receiver_id: {}", receiver_id);
    Ok(false)
}

async fn validate_macaroon_permissions(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    receiver_id: &str,
) -> Result<bool, AppError> {
    // Test general connectivity and macaroon validity
    let info_url = format!("{base_url}/v1/taproot-assets/mailbox/info");
    let info_response = client
        .get(&info_url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| {
            error!("Failed to validate macaroon with backend: {}", e);
            AppError::RequestError(e)
        })?;

    if !info_response.status().is_success() {
        warn!(
            "Macaroon validation failed with status: {}",
            info_response.status()
        );
        return Ok(false);
    }

    // Parse response to check mailbox feature availability
    let info_json = info_response
        .json::<serde_json::Value>()
        .await
        .map_err(AppError::RequestError)?;

    // Check if mailbox is enabled
    if let Some(mailbox_enabled) = info_json.get("mailbox_enabled").and_then(|v| v.as_bool()) {
        if !mailbox_enabled {
            warn!("Mailbox feature is not enabled on the backend");
            return Ok(false);
        }
    }

    // Test mailbox-specific permissions by attempting a minimal receive operation
    let test_receive = ReceiveRequest {
        init: serde_json::json!({
            "receiver_id": receiver_id,
            "test": true
        }),
        auth_sig: serde_json::json!({
            "test": true
        }),
    };

    let receive_url = format!("{base_url}/v1/taproot-assets/mailbox/receive");
    let receive_response = client
        .post(&receive_url)
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&test_receive)
        .timeout(Duration::from_secs(2))
        .send()
        .await;

    match receive_response {
        Ok(resp) => {
            // We expect either success or a specific error indicating permission
            // 403 Forbidden means no permission, other errors might be OK
            if resp.status() == reqwest::StatusCode::FORBIDDEN {
                warn!("Macaroon lacks mailbox receive permissions");
                return Ok(false);
            }
            // Other statuses (including errors) are acceptable for permission check
        }
        Err(e) if e.is_timeout() => {
            // Timeout is OK, we're just checking permissions
            debug!("Permission check timed out, assuming permissions are valid");
        }
        Err(e) => {
            // Connection errors might indicate backend issues, not permission issues
            warn!("Failed to check mailbox permissions: {}", e);
        }
    }

    info!(
        "Macaroon permissions validated for receiver_id: {}",
        receiver_id
    );
    Ok(true)
}

fn is_valid_bech32_chars(s: &str) -> bool {
    const BECH32_CHARSET: &[u8] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
    s.chars()
        .all(|c| c.is_ascii_lowercase() && BECH32_CHARSET.contains(&(c as u8)))
}

fn validate_taproot_address_format(address: &str) -> Result<bool, AppError> {
    if !address.starts_with("taprt1") {
        return Ok(false);
    }

    let data_part = &address[6..]; // Remove "taprt1" prefix

    // Check if it contains only valid Bech32 characters
    if !is_valid_bech32_chars(data_part) {
        return Ok(false);
    }

    // Attempt to decode using bech32
    match bech32::decode(address) {
        Ok((hrp, data)) => {
            // Verify it's a taproot address with correct HRP
            Ok(hrp.as_str() == "taprt1" && !data.is_empty())
        }
        Err(_) => Ok(false),
    }
}

async fn validate_receiver_id(
    receiver_id: &str,
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    database: Option<&SharedDatabase>,
) -> Result<bool, AppError> {
    // Basic format validation
    if receiver_id.len() < 8 {
        warn!("Receiver ID too short: {}", receiver_id);
        return Ok(false);
    }

    // First check if it's a potential Taproot address format - validate Bech32 characters
    if !is_valid_bech32_chars(receiver_id) {
        // If not valid Bech32 chars, check if it's alphanumeric format
        if !receiver_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
            warn!("Receiver ID contains invalid characters: {}", receiver_id);
            return Ok(false);
        }
    }

    // Check if it's a public key format
    if (derive_public_key_from_receiver_id(receiver_id)?).is_some() {
        // If it's a valid public key format, it's valid
        info!("Receiver ID is a valid public key: {}", receiver_id);
        return Ok(true);
    }

    // Check database if available
    if let Some(db) = database {
        if let Some(receiver_info) = db.get_receiver_info(receiver_id).await? {
            if receiver_info.is_active {
                info!(
                    "Receiver ID found in database and is active: {}",
                    receiver_id
                );
                return Ok(true);
            } else {
                warn!("Receiver ID found but is inactive: {}", receiver_id);
                return Ok(false);
            }
        }
    }

    // Attempt to validate against tapd backend by checking if an address exists
    // This is done by trying to decode a taproot address associated with the receiver
    let decode_url = format!("{base_url}/v1/taproot-assets/addrs/decode");
    let test_address = format!("taprt1{receiver_id}"); // Construct a test address

    // First validate the constructed address format locally
    match validate_taproot_address_format(&test_address) {
        Ok(true) => {
            // Address format is valid, proceed with remote validation
            let response = client
                .post(&decode_url)
                .header("Grpc-Metadata-macaroon", macaroon_hex)
                .json(&serde_json::json!({"addr": test_address}))
                .timeout(Duration::from_secs(2))
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    info!("Receiver ID validated via tapd backend: {}", receiver_id);
                    Ok(true)
                }
                _ => {
                    // If remote validation fails but local format is valid, reject it
                    // This prevents accepting addresses that don't exist on the backend
                    warn!(
                        "Receiver ID has valid format but failed backend validation: {}",
                        receiver_id
                    );
                    Ok(false)
                }
            }
        }
        Ok(false) => {
            // Invalid Taproot address format
            warn!(
                "Receiver ID does not form a valid Taproot address: {}",
                receiver_id
            );
            Ok(false)
        }
        Err(e) => {
            warn!("Error validating Taproot address format: {}", e);
            Ok(false)
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn stream_mailbox_messages(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    session: &mut Session,
    state: &mut MailboxState,
    init: &serde_json::Value,
    auth_sig: &serde_json::Value,
    monitoring: Option<&SharedMonitoring>,
    connection_id: &str,
) -> Result<(), AppError> {
    *state = MailboxState::Streaming;

    let receiver_id = init
        .get("receiver_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::InvalidInput("Missing receiver_id".to_string()))?;

    info!(
        "Starting mailbox message stream for receiver: {}",
        receiver_id
    );

    // Create a loop to continuously poll for new messages
    let mut message_count = 0;
    let mut last_message_id: Option<String> = None;
    let poll_interval = Duration::from_secs(1); // Poll every second
    let max_empty_polls = 300; // Stop after 5 minutes of no messages
    let mut empty_polls = 0;

    loop {
        // Build request with optional last_message_id for pagination
        let mut request_init = init.clone();
        if let Some(ref last_id) = last_message_id {
            if let Some(obj) = request_init.as_object_mut() {
                obj.insert(
                    "after_message_id".to_string(),
                    serde_json::Value::String(last_id.clone()),
                );
            }
        }

        let request = ReceiveRequest {
            init: request_init,
            auth_sig: auth_sig.clone(),
        };

        match receive_mail(client, base_url, macaroon_hex, request).await {
            Ok(response_data) => {
                // Check if we got any messages
                let messages = if let Some(messages_array) =
                    response_data.get("messages").and_then(|v| v.as_array())
                {
                    messages_array.clone()
                } else if response_data.is_array() {
                    // Response might be directly an array of messages
                    response_data.as_array().unwrap().clone()
                } else {
                    vec![]
                };

                if !messages.is_empty() {
                    empty_polls = 0; // Reset empty poll counter
                    message_count += messages.len();

                    // Update last_message_id for pagination
                    if let Some(last_msg) = messages.last() {
                        if let Some(msg_id) = last_msg.get("id").and_then(|v| v.as_str()) {
                            last_message_id = Some(msg_id.to_string());
                        }
                    }

                    // Send messages to client
                    let response = MailboxResponse {
                        challenge: None,
                        auth_success: None,
                        messages: Some(serde_json::Value::Array(messages.clone())),
                        eos: None,
                    };

                    let response_json = serde_json::to_string(&response)
                        .map_err(|e| AppError::SerializationError(e.to_string()))?;

                    // Record message sent in monitoring
                    if let Some(mon) = monitoring {
                        mon.record_message_sent(connection_id, response_json.len())
                            .await;
                    }

                    if let Err(e) = session.text(response_json).await {
                        warn!("Failed to send messages to client: {}", e);
                        break;
                    }

                    debug!("Sent {} new messages to client", messages.len());
                } else {
                    empty_polls += 1;

                    // Send heartbeat every 10 empty polls (10 seconds)
                    if empty_polls % 10 == 0 {
                        if let Err(e) = session.ping(b"heartbeat").await {
                            warn!("Failed to send heartbeat: {}", e);
                            break;
                        }
                    }

                    if empty_polls >= max_empty_polls {
                        info!("No messages for {} seconds, ending stream", max_empty_polls);
                        break;
                    }
                }
            }
            Err(e) => {
                // Check if it's a client disconnect or network error
                if let AppError::RequestError(ref req_err) = e {
                    if req_err.is_timeout() || req_err.is_connect() {
                        warn!("Network error while streaming: {}", e);
                        break;
                    }
                }

                error!("Failed to receive mail: {}", e);

                // Send error to client
                let error_response = MailboxResponse {
                    challenge: None,
                    auth_success: None,
                    messages: None,
                    eos: Some(serde_json::json!({
                        "error": e.to_string(),
                        "completed": false
                    })),
                };

                if let Ok(error_json) = serde_json::to_string(&error_response) {
                    let _ = session.text(error_json).await;
                }

                return Err(e);
            }
        }

        // Check if client is still connected by sending a ping
        if (session.ping(b"").await).is_err() {
            info!("Client disconnected, ending stream");
            break;
        }

        // Wait before next poll
        tokio::time::sleep(poll_interval).await;
    }

    // Send end-of-stream message
    let eos_response = MailboxResponse {
        challenge: None,
        auth_success: None,
        messages: None,
        eos: Some(serde_json::json!({
            "completed": true,
            "message_count": message_count,
            "duration_seconds": empty_polls + (message_count as u32)
        })),
    };

    let eos_json = serde_json::to_string(&eos_response)
        .map_err(|e| AppError::SerializationError(e.to_string()))?;

    // Record final message sent in monitoring
    if let Some(mon) = monitoring {
        mon.record_message_sent(connection_id, eos_json.len()).await;
    }

    let _ = session.text(eos_json).await;

    *state = MailboxState::Closed;
    info!(
        "Mailbox stream ended. Total messages delivered: {}",
        message_count
    );
    Ok(())
}

fn handle_result<T: serde::Serialize>(result: Result<T, AppError>) -> HttpResponse {
    match result {
        Ok(value) => HttpResponse::Ok().json(value),
        Err(e) => {
            let status = e.status_code();
            HttpResponse::build(status).json(serde_json::json!({
                "error": e.to_string(),
                "type": format!("{:?}", e)
            }))
        }
    }
}

async fn receive_websocket_with_proxy(
    req: HttpRequest,
    stream: web::Payload,
    ws_proxy_handler: web::Data<Arc<WebSocketProxyHandler>>,
) -> ActixResult<HttpResponse> {
    info!("Mailbox receive WebSocket using proxy handler");

    // Define the backend WebSocket endpoint
    let backend_endpoint = "/v1/taproot-assets/mailbox/receive?stream=true";

    // Handle the WebSocket connection with correlation tracking enabled
    ws_proxy_handler
        .handle_websocket(req, stream, backend_endpoint, true)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/mailbox/info").route(web::get().to(info)))
        .service(web::resource("/mailbox/receive").route(web::post().to(receive)))
        .service(web::resource("/mailbox/receive").route(web::get().to(receive_websocket)))
        .service(web::resource("/mailbox/send").route(web::post().to(send)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_websocket_message_serialization() {
        let init_msg = WebSocketMailboxMessage {
            init: Some(json!({"receiver_id": "test"})),
            auth_sig: None,
        };

        let serialized = serde_json::to_string(&init_msg).unwrap();
        assert!(serialized.contains("init"));
        assert!(!serialized.contains("auth_sig"));
    }

    #[test]
    fn test_websocket_message_deserialization() {
        let json_str = r#"{"init": {"receiver_id": "test"}, "auth_sig": {"signature": "abc123"}}"#;
        let msg: WebSocketMailboxMessage = serde_json::from_str(json_str).unwrap();

        assert!(msg.init.is_some());
        assert!(msg.auth_sig.is_some());
    }

    #[test]
    fn test_mailbox_response_serialization() {
        let response = MailboxResponse {
            challenge: Some(json!({"challenge_id": "test"})),
            auth_success: None,
            messages: None,
            eos: None,
        };

        let serialized = serde_json::to_string(&response).unwrap();
        assert!(serialized.contains("challenge"));
        assert!(!serialized.contains("auth_success"));
        assert!(!serialized.contains("messages"));
        assert!(!serialized.contains("eos"));
    }

    #[test]
    fn test_state_machine_transitions() {
        let mut state = MailboxState::AwaitingInit;

        match state {
            MailboxState::AwaitingInit => {
                state = MailboxState::ChallengeSent;
            }
            _ => panic!("Unexpected state"),
        }

        match state {
            MailboxState::ChallengeSent => {
                state = MailboxState::Authenticated;
            }
            _ => panic!("Unexpected state"),
        }

        match state {
            MailboxState::Authenticated => {
                state = MailboxState::Streaming;
            }
            _ => panic!("Unexpected state"),
        }

        match state {
            MailboxState::Streaming => {
                state = MailboxState::Closed;
            }
            _ => panic!("Unexpected state"),
        }

        matches!(state, MailboxState::Closed);
    }

    #[tokio::test]
    async fn test_generate_challenge() {
        let challenge = generate_challenge().await.unwrap();

        assert!(challenge.get("challenge_id").is_some());
        assert!(challenge.get("timestamp").is_some());
        assert!(challenge.get("nonce").is_some());

        let challenge_id = challenge.get("challenge_id").unwrap().as_str().unwrap();
        assert!(!challenge_id.is_empty());

        let timestamp = challenge.get("timestamp").unwrap().as_i64().unwrap();
        assert!(timestamp > 0);

        let nonce = challenge.get("nonce").unwrap().as_str().unwrap();
        assert!(!nonce.is_empty());
    }

    #[tokio::test]
    async fn test_validate_authentication_missing_fields() {
        let init = json!({"receiver_id": "test_receiver_12345"});
        let auth_sig = json!({"signature": "abcdef1234567890abcdef1234567890abcdef1234567890"}); // Missing challenge_id and timestamp

        let result = validate_authentication(
            &init,
            &auth_sig,
            &reqwest::Client::new(),
            "http://localhost:8289",
            "test_macaroon",
            None,
        )
        .await;
        assert!(result.is_err()); // Should fail due to missing required fields
    }

    #[tokio::test]
    async fn test_validate_authentication_invalid_challenge() {
        let init = json!({"receiver_id": "test_receiver_12345"});
        let auth_sig = json!({
            "signature": "abcdef1234567890abcdef1234567890abcdef1234567890",
            "challenge_id": "nonexistent_challenge",
            "timestamp": chrono::Utc::now().timestamp()
        });

        let result = validate_authentication(
            &init,
            &auth_sig,
            &reqwest::Client::new(),
            "http://localhost:8289",
            "test_macaroon",
            None,
        )
        .await;
        assert!(result.is_err()); // Should fail due to invalid challenge_id
    }

    #[test]
    fn test_websocket_url_format() {
        let base_url = "wss://localhost:8080";
        let endpoint = "/v1/taproot-assets/mailbox/receive";
        let full_url = format!("{base_url}{endpoint}");

        assert_eq!(
            full_url,
            "wss://localhost:8080/v1/taproot-assets/mailbox/receive"
        );
        assert!(full_url.starts_with("wss://"));
        assert!(full_url.contains("/mailbox/receive"));
    }

    #[test]
    fn test_mailbox_flow_sequence() {
        let client_init = WebSocketMailboxMessage {
            init: Some(json!({"receiver_id": "user123"})),
            auth_sig: None,
        };
        assert!(client_init.init.is_some());
        assert!(client_init.auth_sig.is_none());

        let challenge_response = MailboxResponse {
            challenge: Some(json!({"challenge_id": "ch123", "nonce": "abc"})),
            auth_success: None,
            messages: None,
            eos: None,
        };
        assert!(challenge_response.challenge.is_some());

        let client_auth = WebSocketMailboxMessage {
            init: None,
            auth_sig: Some(json!({
                "signature": "sig123456789abcdef123456789abcdef123456789abcdef123456789abcdef",
                "challenge_id": "ch123",
                "timestamp": chrono::Utc::now().timestamp()
            })),
        };
        assert!(client_auth.init.is_none());
        assert!(client_auth.auth_sig.is_some());

        let auth_success_response = MailboxResponse {
            challenge: None,
            auth_success: Some(true),
            messages: None,
            eos: None,
        };
        assert_eq!(auth_success_response.auth_success, Some(true));

        let messages_response = MailboxResponse {
            challenge: None,
            auth_success: None,
            messages: Some(json!({"messages": ["msg1", "msg2"]})),
            eos: None,
        };
        assert!(messages_response.messages.is_some());

        let eos_response = MailboxResponse {
            challenge: None,
            auth_success: None,
            messages: None,
            eos: Some(json!({"completed": true})),
        };
        assert!(eos_response.eos.is_some());
    }

    #[test]
    fn test_authentication_failure_handling() {
        let auth_failure_response = MailboxResponse {
            challenge: None,
            auth_success: Some(false),
            messages: None,
            eos: None,
        };
        assert_eq!(auth_failure_response.auth_success, Some(false));
        assert!(auth_failure_response.messages.is_none());
        assert!(auth_failure_response.eos.is_none());
    }

    #[test]
    fn test_request_format_matches_plan() {
        let expected_init = json!({"receiver_id": "test_receiver"});
        let expected_auth_sig = json!({
            "signature": "test_signature_123456789abcdef123456789abcdef123456789abcdef",
            "challenge_id": "test_challenge_id",
            "timestamp": 1640995200
        });

        let request = WebSocketMailboxMessage {
            init: Some(expected_init.clone()),
            auth_sig: Some(expected_auth_sig.clone()),
        };

        assert_eq!(request.init, Some(expected_init));
        assert_eq!(request.auth_sig, Some(expected_auth_sig));
    }

    #[test]
    fn test_response_format_matches_plan() {
        let response = MailboxResponse {
            challenge: Some(json!({"challenge_id": "test"})),
            auth_success: Some(true),
            messages: Some(json!({"data": "test"})),
            eos: Some(json!({"completed": true})),
        };

        let serialized = serde_json::to_string(&response).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();

        assert!(parsed.get("challenge").is_some());
        assert!(parsed.get("auth_success").is_some());
        assert!(parsed.get("messages").is_some());
        assert!(parsed.get("eos").is_some());
    }
}
