use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_ws::{Message as WsMessage, MessageStream, Session};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::connection_manager::WebSocketConnectionManager;
use crate::error::AppError;

const CLIENT_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes
const MESSAGE_TIMEOUT: Duration = Duration::from_secs(30); // 30 seconds for individual messages
const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024; // 10MB

/// Handles WebSocket proxy connections between clients and the tapd backend
pub struct WebSocketProxyHandler {
    connection_manager: Arc<WebSocketConnectionManager>,
    active_proxies: Arc<Mutex<HashMap<Uuid, ProxySession>>>,
}

/// Represents an active proxy session
struct ProxySession {
    #[allow(dead_code)]
    id: Uuid,
    client_id: String,
    backend_endpoint: String,
    backend_conn_id: Uuid,
    created_at: std::time::Instant,
    last_activity_epoch: Arc<AtomicU64>,
    correlation_required: bool,
}

impl WebSocketProxyHandler {
    /// Creates a new WebSocket proxy handler
    pub fn new(connection_manager: Arc<WebSocketConnectionManager>) -> Self {
        Self {
            connection_manager,
            active_proxies: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Handles incoming WebSocket connection requests
    pub async fn handle_websocket(
        &self,
        req: HttpRequest,
        stream: web::Payload,
        backend_endpoint: &str,
        correlation_required: bool,
    ) -> Result<HttpResponse, Error> {
        let session_id = Uuid::new_v4();
        let client_addr = req
            .peer_addr()
            .map(|addr| addr.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        info!(
            "New WebSocket connection from {} for endpoint {}",
            client_addr, backend_endpoint
        );

        // Upgrade to WebSocket
        let (response, session, msg_stream) = actix_ws::handle(&req, stream)?;

        // Create backend connection
        let (backend_conn_id, backend_sink, backend_stream) = self
            .connection_manager
            .connect_to_backend(backend_endpoint)
            .await
            .map_err(|e| {
                error!("Failed to create backend connection: {}", e);
                actix_web::error::ErrorInternalServerError(format!("WebSocket proxy error: {e}"))
            })?;

        // Store proxy session
        let current_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let proxy_session = ProxySession {
            id: session_id,
            client_id: client_addr.clone(),
            backend_endpoint: backend_endpoint.to_string(),
            backend_conn_id,
            created_at: std::time::Instant::now(),
            last_activity_epoch: Arc::new(AtomicU64::new(current_epoch)),
            correlation_required,
        };

        {
            let mut proxies = self.active_proxies.lock().await;
            proxies.insert(session_id, proxy_session);
        }

        // Start bidirectional message forwarding
        let handler = self.clone();
        actix_web::rt::spawn(async move {
            if let Err(e) = handler
                .forward_messages(
                    session_id,
                    session,
                    msg_stream,
                    backend_sink,
                    backend_stream,
                    backend_conn_id,
                    correlation_required,
                )
                .await
            {
                error!("Message forwarding error for session {}: {}", session_id, e);
            }

            // Cleanup on disconnect
            handler.cleanup_session(session_id, backend_conn_id).await;
        });

        Ok(response)
    }

    /// Forwards messages bidirectionally between client and backend
    #[allow(clippy::too_many_arguments)]
    async fn forward_messages(
        &self,
        session_id: Uuid,
        client_session: Session,
        client_stream: MessageStream,
        backend_sink: futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
            TungsteniteMessage,
        >,
        backend_stream: futures_util::stream::SplitStream<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
        >,
        backend_conn_id: Uuid,
        _correlation_required: bool,
    ) -> Result<(), AppError> {
        let client_sink = Arc::new(Mutex::new(client_session));
        let backend_sink = Arc::new(Mutex::new(backend_sink));

        // TODO: Implement correlation logic for tracking request/response pairs
        // when correlation_required is true. This would enable features like
        // request tracing and response matching in complex scenarios.

        // Update activity tracker
        let activity_tracker = {
            let proxies = self.active_proxies.lock().await;
            proxies
                .get(&session_id)
                .map(|p| p.last_activity_epoch.clone())
                .ok_or_else(|| AppError::WebSocketProxyError("Session not found".to_string()))?
        };

        // Spawn task to forward client -> backend
        let client_to_backend = {
            let backend_sink = backend_sink.clone();
            let connection_manager = self.connection_manager.clone();
            let activity_tracker = activity_tracker.clone();

            actix_web::rt::spawn(async move {
                let mut client_stream = client_stream;

                while let Ok(Some(msg)) = timeout(CLIENT_TIMEOUT, client_stream.next()).await {
                    // Update activity atomically
                    let current_epoch = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    activity_tracker.store(current_epoch, Ordering::Relaxed);

                    match msg {
                        Ok(WsMessage::Text(text)) => {
                            debug!("Forwarding text message from client: {} bytes", text.len());

                            // Check message size
                            if text.len() > MAX_MESSAGE_SIZE {
                                error!("Message too large: {} bytes", text.len());
                                break;
                            }

                            let tungstenite_msg = TungsteniteMessage::Text(text.to_string().into());

                            // Send to backend
                            let mut sink = backend_sink.lock().await;
                            if let Err(e) =
                                timeout(MESSAGE_TIMEOUT, sink.send(tungstenite_msg)).await
                            {
                                error!("Failed to send message to backend: {:?}", e);
                                break;
                            }

                            // Update connection activity
                            connection_manager.update_activity(backend_conn_id).await;
                        }
                        Ok(WsMessage::Binary(data)) => {
                            debug!(
                                "Forwarding binary message from client: {} bytes",
                                data.len()
                            );

                            // Check message size
                            if data.len() > MAX_MESSAGE_SIZE {
                                error!("Message too large: {} bytes", data.len());
                                break;
                            }

                            let tungstenite_msg = TungsteniteMessage::Binary(data);

                            // Send to backend
                            let mut sink = backend_sink.lock().await;
                            if let Err(e) =
                                timeout(MESSAGE_TIMEOUT, sink.send(tungstenite_msg)).await
                            {
                                error!("Failed to send message to backend: {:?}", e);
                                break;
                            }

                            // Update connection activity
                            connection_manager.update_activity(backend_conn_id).await;
                        }
                        Ok(WsMessage::Close(reason)) => {
                            info!("Client closing connection: {:?}", reason);
                            let mut sink = backend_sink.lock().await;
                            let _ = sink.send(TungsteniteMessage::Close(None)).await;
                            break;
                        }
                        Ok(WsMessage::Ping(data)) => {
                            let mut sink = backend_sink.lock().await;
                            let _ = sink.send(TungsteniteMessage::Ping(data)).await;
                        }
                        Ok(WsMessage::Pong(data)) => {
                            let mut sink = backend_sink.lock().await;
                            let _ = sink.send(TungsteniteMessage::Pong(data)).await;
                        }
                        Ok(WsMessage::Continuation(_)) | Ok(WsMessage::Nop) => {
                            // Continuation and Nop frames are ignored
                        }
                        Err(e) => {
                            error!("WebSocket error from client: {}", e);
                            break;
                        }
                    }
                }

                debug!(
                    "Client -> Backend forwarding ended for session {}",
                    session_id
                );
            })
        };

        // Spawn task to forward backend -> client
        let backend_to_client = {
            let client_sink = client_sink.clone();
            let connection_manager = self.connection_manager.clone();
            let activity_tracker = activity_tracker.clone();

            actix_web::rt::spawn(async move {
                let mut backend_stream = backend_stream;

                loop {
                    let msg = timeout(CLIENT_TIMEOUT, backend_stream.next()).await;

                    match msg {
                        Ok(Some(Ok(msg))) => {
                            // Update activity atomically
                            let current_epoch = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            activity_tracker.store(current_epoch, Ordering::Relaxed);

                            let client_msg = match msg {
                                TungsteniteMessage::Text(text) => {
                                    debug!(
                                        "Forwarding text message from backend: {} bytes",
                                        text.len()
                                    );
                                    WsMessage::Text(text.to_string().into())
                                }
                                TungsteniteMessage::Binary(data) => {
                                    debug!(
                                        "Forwarding binary message from backend: {} bytes",
                                        data.len()
                                    );
                                    WsMessage::Binary(data)
                                }
                                TungsteniteMessage::Close(frame) => {
                                    info!("Backend closing connection: {:?}", frame);
                                    WsMessage::Close(frame.map(|f| actix_ws::CloseReason {
                                        code: actix_ws::CloseCode::from(u16::from(f.code)),
                                        description: Some(f.reason.to_string()),
                                    }))
                                }
                                TungsteniteMessage::Ping(data) => WsMessage::Ping(data),
                                TungsteniteMessage::Pong(data) => WsMessage::Pong(data),
                                _ => continue,
                            };

                            // Send to client
                            match &client_msg {
                                WsMessage::Text(text) => {
                                    let mut session = client_sink.lock().await;
                                    if let Err(e) =
                                        timeout(MESSAGE_TIMEOUT, session.text(text.clone())).await
                                    {
                                        error!("Failed to send text message to client: {:?}", e);
                                        break;
                                    }
                                }
                                WsMessage::Binary(data) => {
                                    let mut session = client_sink.lock().await;
                                    if let Err(e) =
                                        timeout(MESSAGE_TIMEOUT, session.binary(data.clone())).await
                                    {
                                        error!("Failed to send binary message to client: {:?}", e);
                                        break;
                                    }
                                }
                                WsMessage::Close(_reason) => {
                                    // Just break - the session will be closed when dropped
                                    break;
                                }
                                WsMessage::Ping(data) => {
                                    let mut session = client_sink.lock().await;
                                    if let Err(e) =
                                        timeout(MESSAGE_TIMEOUT, session.ping(data)).await
                                    {
                                        error!("Failed to send ping to client: {:?}", e);
                                        break;
                                    }
                                }
                                WsMessage::Pong(data) => {
                                    let mut session = client_sink.lock().await;
                                    if let Err(e) =
                                        timeout(MESSAGE_TIMEOUT, session.pong(data)).await
                                    {
                                        error!("Failed to send pong to client: {:?}", e);
                                        break;
                                    }
                                }
                                _ => {}
                            }

                            // Update connection activity
                            connection_manager.update_activity(backend_conn_id).await;
                        }
                        Ok(Some(Err(e))) => {
                            error!("WebSocket error from backend: {}", e);
                            break;
                        }
                        Ok(None) => {
                            info!("Backend WebSocket stream ended");
                            break;
                        }
                        Err(_) => {
                            warn!("Backend connection timeout");
                            break;
                        }
                    }
                }

                debug!(
                    "Backend -> Client forwarding ended for session {}",
                    session_id
                );
            })
        };

        // Wait for either direction to complete
        tokio::select! {
            _ = client_to_backend => {
                debug!("Client to backend task completed");
            }
            _ = backend_to_client => {
                debug!("Backend to client task completed");
            }
        }

        Ok(())
    }

    /// Cleans up resources when a proxy session ends
    async fn cleanup_session(&self, session_id: Uuid, backend_conn_id: Uuid) {
        info!("Cleaning up proxy session {}", session_id);

        // Remove from active proxies
        {
            let mut proxies = self.active_proxies.lock().await;
            if let Some(session) = proxies.remove(&session_id) {
                let duration = session.created_at.elapsed();
                info!(
                    "Proxy session {} ended after {:?} for client {}",
                    session_id, duration, session.client_id
                );
            }
        }

        // Remove backend connection
        self.connection_manager
            .remove_connection(backend_conn_id)
            .await;
    }

    /// Gets the number of active proxy sessions
    pub async fn active_session_count(&self) -> usize {
        self.active_proxies.lock().await.len()
    }

    /// Gets information about active sessions
    pub async fn get_active_sessions(&self) -> Vec<SessionInfo> {
        let proxies = self.active_proxies.lock().await;
        let mut sessions = Vec::new();

        for (id, session) in proxies.iter() {
            let last_activity_epoch = session.last_activity_epoch.load(Ordering::Relaxed);
            let last_activity = UNIX_EPOCH + Duration::from_secs(last_activity_epoch);
            let last_activity_instant = std::time::Instant::now()
                - SystemTime::now()
                    .duration_since(last_activity)
                    .unwrap_or_default();
            sessions.push(SessionInfo {
                id: *id,
                client_id: session.client_id.clone(),
                backend_endpoint: session.backend_endpoint.clone(),
                created_at: session.created_at,
                last_activity: last_activity_instant,
                correlation_required: session.correlation_required,
            });
        }

        sessions
    }

    /// Cleans up stale sessions
    pub async fn cleanup_stale_sessions(&self, max_idle: Duration) {
        let current_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut stale_sessions = Vec::new();

        {
            let proxies = self.active_proxies.lock().await;
            for (id, session) in proxies.iter() {
                let last_activity_epoch = session.last_activity_epoch.load(Ordering::Relaxed);
                let idle_duration =
                    Duration::from_secs(current_epoch.saturating_sub(last_activity_epoch));
                if idle_duration > max_idle {
                    stale_sessions.push(*id);
                }
            }
        }

        for session_id in stale_sessions {
            warn!("Cleaning up stale session: {}", session_id);

            // Get backend_conn_id before removing the session
            let backend_conn_id = {
                let proxies = self.active_proxies.lock().await;
                proxies
                    .get(&session_id)
                    .map(|session| session.backend_conn_id)
            };

            // Remove from active proxies
            {
                let mut proxies = self.active_proxies.lock().await;
                proxies.remove(&session_id);
            }

            // Clean up backend connection if found
            if let Some(conn_id) = backend_conn_id {
                self.connection_manager.remove_connection(conn_id).await;
            }
        }
    }
}

impl Clone for WebSocketProxyHandler {
    fn clone(&self) -> Self {
        Self {
            connection_manager: self.connection_manager.clone(),
            active_proxies: self.active_proxies.clone(),
        }
    }
}

/// Information about an active proxy session
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: Uuid,
    pub client_id: String,
    pub backend_endpoint: String,
    pub created_at: std::time::Instant,
    pub last_activity: std::time::Instant,
    pub correlation_required: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{BaseUrl, MacaroonHex};
    use crate::websocket::connection_manager::WebSocketConnectionManager;

    #[tokio::test]
    async fn test_proxy_handler_creation() {
        let manager = Arc::new(WebSocketConnectionManager::new(
            BaseUrl("ws://localhost:8290".to_string()),
            MacaroonHex("test_macaroon".to_string()),
            false,
        ));

        let handler = WebSocketProxyHandler::new(manager);

        assert_eq!(handler.active_session_count().await, 0);
    }

    #[tokio::test]
    async fn test_session_tracking() {
        let manager = Arc::new(WebSocketConnectionManager::new(
            BaseUrl("ws://localhost:8290".to_string()),
            MacaroonHex("test_macaroon".to_string()),
            false,
        ));

        let handler = WebSocketProxyHandler::new(manager);

        // Add a mock session
        let session_id = Uuid::new_v4();
        let backend_conn_id = Uuid::new_v4();
        let current_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let session = ProxySession {
            id: session_id,
            client_id: "test_client".to_string(),
            backend_endpoint: "/test".to_string(),
            backend_conn_id,
            created_at: std::time::Instant::now(),
            last_activity_epoch: Arc::new(AtomicU64::new(current_epoch)),
            correlation_required: false,
        };

        {
            let mut proxies = handler.active_proxies.lock().await;
            proxies.insert(session_id, session);
        }

        assert_eq!(handler.active_session_count().await, 1);

        let sessions = handler.get_active_sessions().await;
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].client_id, "test_client");
        assert_eq!(sessions[0].backend_endpoint, "/test");
    }

    #[tokio::test]
    async fn test_cleanup_stale_sessions() {
        let manager = Arc::new(WebSocketConnectionManager::new(
            BaseUrl("ws://localhost:8290".to_string()),
            MacaroonHex("test_macaroon".to_string()),
            false,
        ));

        let handler = WebSocketProxyHandler::new(manager);

        // Add a stale session
        let session_id = Uuid::new_v4();
        let backend_conn_id = Uuid::new_v4();
        let old_time = std::time::Instant::now() - Duration::from_secs(3600); // 1 hour ago
        let old_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            - 3600; // 1 hour ago in epoch seconds
        let session = ProxySession {
            id: session_id,
            client_id: "stale_client".to_string(),
            backend_endpoint: "/test".to_string(),
            backend_conn_id,
            created_at: old_time,
            last_activity_epoch: Arc::new(AtomicU64::new(old_epoch)),
            correlation_required: false,
        };

        {
            let mut proxies = handler.active_proxies.lock().await;
            proxies.insert(session_id, session);
        }

        assert_eq!(handler.active_session_count().await, 1);

        // Cleanup sessions older than 30 minutes
        handler
            .cleanup_stale_sessions(Duration::from_secs(1800))
            .await;

        assert_eq!(handler.active_session_count().await, 0);
    }
}
