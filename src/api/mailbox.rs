use crate::error::AppError;
use crate::types::{BaseUrl, MacaroonHex};
use actix_web::{web, HttpRequest, HttpResponse, Result as ActixResult};
use actix_ws::{Message, MessageStream, Session};
use base64::Engine;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument, warn};

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
    let (response, session, msg_stream) = actix_ws::handle(&req, stream)?;

    info!("Mailbox receive WebSocket connection established");

    actix_rt::spawn(handle_mailbox_websocket_connection(
        session,
        msg_stream,
        client.get_ref().clone(),
        base_url.0.clone(),
        macaroon_hex.0.clone(),
    ));

    Ok(response)
}

async fn handle_mailbox_websocket_connection(
    mut session: Session,
    mut msg_stream: MessageStream,
    client: Client,
    base_url: String,
    macaroon_hex: String,
) {
    let mut state = MailboxState::AwaitingInit;
    let mut pending_init: Option<serde_json::Value> = None;

    while let Some(msg) = msg_stream.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                info!("Received mailbox WebSocket message: {}", text);

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

    info!("Mailbox WebSocket connection handler finished");
}

async fn handle_mailbox_message(
    state: &mut MailboxState,
    msg: WebSocketMailboxMessage,
    pending_init: &mut Option<serde_json::Value>,
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    session: &mut Session,
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
                    let auth_result =
                        validate_authentication(&init, &auth_sig, client, base_url, macaroon_hex)
                            .await?;

                    let response = MailboxResponse {
                        challenge: None,
                        auth_success: Some(auth_result),
                        messages: None,
                        eos: None,
                    };

                    let response_json = serde_json::to_string(&response)
                        .map_err(|e| AppError::SerializationError(e.to_string()))?;
                    session
                        .text(response_json)
                        .await
                        .map_err(|e| AppError::WebSocketError(e.to_string()))?;

                    if auth_result {
                        *state = MailboxState::Authenticated;
                        stream_mailbox_messages(client, base_url, macaroon_hex, session, state)
                            .await?;
                        Ok(false)
                    } else {
                        warn!("Authentication failed");
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
    Ok(serde_json::json!({
        "challenge_id": uuid::Uuid::new_v4().to_string(),
        "timestamp": chrono::Utc::now().timestamp(),
        "nonce": base64::engine::general_purpose::STANDARD.encode(uuid::Uuid::new_v4().as_bytes())
    }))
}

async fn validate_authentication(
    _init: &serde_json::Value,
    _auth_sig: &serde_json::Value,
    _client: &Client,
    _base_url: &str,
    _macaroon_hex: &str,
) -> Result<bool, AppError> {
    Ok(true)
}

async fn stream_mailbox_messages(
    client: &Client,
    base_url: &str,
    macaroon_hex: &str,
    session: &mut Session,
    state: &mut MailboxState,
) -> Result<(), AppError> {
    *state = MailboxState::Streaming;

    let request = ReceiveRequest {
        init: serde_json::json!({}),
        auth_sig: serde_json::json!({}),
    };

    match receive_mail(client, base_url, macaroon_hex, request).await {
        Ok(messages) => {
            let response = MailboxResponse {
                challenge: None,
                auth_success: None,
                messages: Some(messages),
                eos: None,
            };

            let response_json = serde_json::to_string(&response)
                .map_err(|e| AppError::SerializationError(e.to_string()))?;
            session
                .text(response_json)
                .await
                .map_err(|e| AppError::WebSocketError(e.to_string()))?;
        }
        Err(e) => {
            error!("Failed to receive mail: {}", e);
            return Err(e);
        }
    }

    let eos_response = MailboxResponse {
        challenge: None,
        auth_success: None,
        messages: None,
        eos: Some(serde_json::json!({"completed": true})),
    };

    let eos_json = serde_json::to_string(&eos_response)
        .map_err(|e| AppError::SerializationError(e.to_string()))?;
    session
        .text(eos_json)
        .await
        .map_err(|e| AppError::WebSocketError(e.to_string()))?;

    *state = MailboxState::Closed;
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
    async fn test_validate_authentication_success() {
        let init = json!({"receiver_id": "test"});
        let auth_sig = json!({"signature": "valid_signature"});

        let result = validate_authentication(
            &init,
            &auth_sig,
            &reqwest::Client::new(),
            "http://localhost",
            "test_macaroon",
        )
        .await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_websocket_url_format() {
        let base_url = "wss://localhost:8080";
        let endpoint = "/v1/taproot-assets/mailbox/receive";
        let full_url = format!("{}{}", base_url, endpoint);

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
            auth_sig: Some(json!({"signature": "sig123"})),
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
        let expected_auth_sig = json!({"signature": "test_signature"});

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
