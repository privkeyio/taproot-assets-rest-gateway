#![allow(dead_code)]
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_ws::{Message as WsMessage, MessageStream, Session};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
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
const CORRELATION_TIMEOUT: Duration = Duration::from_secs(60); // 1 minute for correlation timeout
const CORRELATION_CLEANUP_INTERVAL: Duration = Duration::from_secs(30); // 30 seconds cleanup interval

/// Represents a pending request awaiting correlation with its response
#[derive(Debug, Clone)]
struct PendingRequest {
    #[allow(dead_code)]
    correlation_id: String,
    original_message: String,
    sent_at: Instant,
    #[allow(dead_code)]
    client_session_id: Uuid,
}

/// Tracks correlation state for a WebSocket session
#[derive(Debug)]
struct CorrelationTracker {
    pending_requests: HashMap<String, PendingRequest>,
    next_correlation_id: AtomicU64,
    session_id: Uuid,
}

impl CorrelationTracker {
    fn new(session_id: Uuid) -> Self {
        Self {
            pending_requests: HashMap::new(),
            next_correlation_id: AtomicU64::new(1),
            session_id,
        }
    }

    fn generate_correlation_id(&self) -> String {
        let id = self.next_correlation_id.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        format!("corr_{}_{}_{}", self.session_id, id, timestamp)
    }

    fn add_pending_request(&mut self, correlation_id: String, original_message: String) {
        let request = PendingRequest {
            correlation_id: correlation_id.clone(),
            original_message,
            sent_at: Instant::now(),
            client_session_id: self.session_id,
        };
        debug!(
            "Added pending request with correlation ID: {}",
            correlation_id
        );
        self.pending_requests.insert(correlation_id, request);
    }

    fn remove_pending_request(&mut self, correlation_id: &str) -> Option<PendingRequest> {
        let request = self.pending_requests.remove(correlation_id);
        if request.is_some() {
            debug!("Matched response with correlation ID: {}", correlation_id);
        }
        request
    }

    fn cleanup_expired_requests(&mut self) -> Vec<PendingRequest> {
        let now = Instant::now();
        let mut expired = Vec::new();

        self.pending_requests.retain(|correlation_id, request| {
            if now.duration_since(request.sent_at) > CORRELATION_TIMEOUT {
                warn!("Correlation timeout for request: {}", correlation_id);
                expired.push(request.clone());
                false
            } else {
                true
            }
        });

        expired
    }

    fn pending_count(&self) -> usize {
        self.pending_requests.len()
    }
}

/// Message processing utilities for correlation tracking
struct MessageProcessor;

impl MessageProcessor {
    /// Attempts to inject a correlation ID into a JSON message
    fn inject_correlation_id(
        message: &str,
        correlation_id: &str,
    ) -> Result<String, serde_json::Error> {
        // Try to parse as JSON
        match serde_json::from_str::<Value>(message) {
            Ok(mut json) => {
                // Inject correlation ID into the message
                if let Some(obj) = json.as_object_mut() {
                    obj.insert("_correlation_id".to_string(), json!(correlation_id));
                    debug!(
                        "Injected correlation ID {} into JSON message",
                        correlation_id
                    );
                } else {
                    // If it's not an object, wrap it
                    json = json!({
                        "_correlation_id": correlation_id,
                        "_original_message": json
                    });
                    debug!(
                        "Wrapped non-object JSON with correlation ID {}",
                        correlation_id
                    );
                }
                serde_json::to_string(&json)
            }
            Err(_) => {
                // Not valid JSON, try to wrap as text message
                let wrapped = json!({
                    "_correlation_id": correlation_id,
                    "_original_text": message,
                    "_wrapped": true
                });
                serde_json::to_string(&wrapped)
            }
        }
    }

    /// Attempts to extract correlation ID from a message
    fn extract_correlation_id(message: &str) -> Option<String> {
        match serde_json::from_str::<Value>(message) {
            Ok(json) => {
                if let Some(obj) = json.as_object() {
                    // Check for correlation ID field
                    if let Some(corr_id) = obj.get("_correlation_id") {
                        if let Some(id_str) = corr_id.as_str() {
                            debug!("Extracted correlation ID {} from response", id_str);
                            return Some(id_str.to_string());
                        }
                    }

                    // Also check common gRPC/API response patterns
                    if let Some(corr_id) =
                        obj.get("correlation_id").or_else(|| obj.get("request_id"))
                    {
                        if let Some(id_str) = corr_id.as_str() {
                            debug!("Extracted correlation ID {} from response field", id_str);
                            return Some(id_str.to_string());
                        }
                    }
                }
                None
            }
            Err(_) => None,
        }
    }

    /// Checks if a message appears to be a request (heuristic)
    fn is_request_message(message: &str) -> bool {
        match serde_json::from_str::<Value>(message) {
            Ok(json) => {
                if let Some(obj) = json.as_object() {
                    // Common patterns for requests
                    obj.contains_key("method") ||
                    obj.contains_key("command") ||
                    obj.contains_key("action") ||
                    obj.contains_key("request") ||
                    // gRPC-style patterns
                    message.contains("Request") ||
                    // API endpoint patterns
                    obj.contains_key("endpoint") ||
                    obj.contains_key("path")
                } else {
                    false
                }
            }
            Err(_) => {
                // For non-JSON messages, use simple heuristics
                message.contains("request") || message.contains("cmd") || message.contains("call")
            }
        }
    }

    /// Checks if a message appears to be a response (heuristic)
    #[allow(dead_code)]
    fn is_response_message(message: &str) -> bool {
        match serde_json::from_str::<Value>(message) {
            Ok(json) => {
                if let Some(obj) = json.as_object() {
                    // Common patterns for responses
                    obj.contains_key("result") ||
                    obj.contains_key("response") ||
                    obj.contains_key("data") ||
                    obj.contains_key("error") ||
                    obj.contains_key("status") ||
                    // gRPC-style patterns
                    message.contains("Response") ||
                    message.contains("Reply")
                } else {
                    false
                }
            }
            Err(_) => {
                // For non-JSON messages, use simple heuristics
                message.contains("response")
                    || message.contains("result")
                    || message.contains("reply")
            }
        }
    }
}

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
    correlation_tracker: Option<Arc<Mutex<CorrelationTracker>>>,
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
        let correlation_tracker = if correlation_required {
            Some(Arc::new(Mutex::new(CorrelationTracker::new(session_id))))
        } else {
            None
        };

        let proxy_session = ProxySession {
            id: session_id,
            client_id: client_addr.clone(),
            backend_endpoint: backend_endpoint.to_string(),
            backend_conn_id,
            created_at: std::time::Instant::now(),
            last_activity_epoch: Arc::new(AtomicU64::new(current_epoch)),
            correlation_required,
            correlation_tracker,
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

        // Get correlation tracker if enabled
        let correlation_tracker = if _correlation_required {
            let proxies = self.active_proxies.lock().await;
            proxies
                .get(&session_id)
                .and_then(|p| p.correlation_tracker.clone())
        } else {
            None
        };

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
            let correlation_tracker_clone = correlation_tracker.clone();

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

                            // Handle correlation tracking if enabled
                            let final_message = if let Some(ref tracker) = correlation_tracker_clone
                            {
                                let text_str = text.to_string();

                                // Check if this is a request message that needs correlation
                                if MessageProcessor::is_request_message(&text_str) {
                                    let mut tracker_guard = tracker.lock().await;
                                    let correlation_id = tracker_guard.generate_correlation_id();

                                    match MessageProcessor::inject_correlation_id(
                                        &text_str,
                                        &correlation_id,
                                    ) {
                                        Ok(modified_message) => {
                                            tracker_guard.add_pending_request(
                                                correlation_id.clone(),
                                                text_str,
                                            );
                                            debug!(
                                                "Added correlation tracking for request: {}",
                                                correlation_id
                                            );
                                            modified_message
                                        }
                                        Err(e) => {
                                            warn!("Failed to inject correlation ID: {}, sending original", e);
                                            text_str
                                        }
                                    }
                                } else {
                                    text_str
                                }
                            } else {
                                text.to_string()
                            };

                            let tungstenite_msg = TungsteniteMessage::Text(final_message.into());

                            // Send to backend
                            let mut sink = backend_sink.lock().await;
                            if let Err(e) =
                                timeout(MESSAGE_TIMEOUT, sink.send(tungstenite_msg)).await
                            {
                                error!("Failed to send message to backend: {:?}", e);
                                // Close backend connection on send failure
                                let _ = sink.close().await;
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
                                // Close backend connection on send failure
                                let _ = sink.close().await;
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
            let correlation_tracker_clone = correlation_tracker.clone();

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

                                    // Handle correlation tracking if enabled
                                    let final_text =
                                        if let Some(ref tracker) = correlation_tracker_clone {
                                            let text_str = text.to_string();

                                            // Check if this is a response with correlation ID
                                            if let Some(correlation_id) =
                                                MessageProcessor::extract_correlation_id(&text_str)
                                            {
                                                let mut tracker_guard = tracker.lock().await;
                                                if let Some(original_request) = tracker_guard
                                                    .remove_pending_request(&correlation_id)
                                                {
                                                    info!(
                                                    "Matched response to request {} (took {:?})",
                                                    correlation_id,
                                                    original_request.sent_at.elapsed()
                                                );
                                                    // Could add request/response logging here
                                                    debug!(
                                                        "Original request: {}",
                                                        original_request.original_message
                                                    );
                                                    debug!("Response: {}", text_str);
                                                }
                                            }
                                            text_str
                                        } else {
                                            text.to_string()
                                        };

                                    WsMessage::Text(final_text.into())
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

        // Start correlation cleanup task if tracking is enabled
        let cleanup_task = if let Some(ref tracker) = correlation_tracker {
            let tracker_clone = tracker.clone();
            Some(actix_web::rt::spawn(async move {
                let mut interval = tokio::time::interval(CORRELATION_CLEANUP_INTERVAL);
                loop {
                    interval.tick().await;
                    let mut tracker_guard = tracker_clone.lock().await;
                    let expired = tracker_guard.cleanup_expired_requests();
                    if !expired.is_empty() {
                        warn!("Cleaned up {} expired correlation requests", expired.len());
                    }
                    let pending_count = tracker_guard.pending_count();
                    if pending_count > 0 {
                        debug!("Pending correlation requests: {}", pending_count);
                    }
                }
            }))
        } else {
            None
        };

        // Wait for either direction to complete
        tokio::select! {
            _ = client_to_backend => {
                debug!("Client to backend task completed");
                // Ensure backend connection is closed on task completion
                let mut backend = backend_sink.lock().await;
                let _ = backend.close().await;
            }
            _ = backend_to_client => {
                debug!("Backend to client task completed");
                // Ensure backend connection is closed on task completion
                let mut backend = backend_sink.lock().await;
                let _ = backend.close().await;
            }
        }

        // Cancel cleanup task if it was running
        if let Some(task) = cleanup_task {
            task.abort();
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
            correlation_tracker: None,
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
            correlation_tracker: None,
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

    #[tokio::test]
    async fn test_correlation_tracker() {
        let session_id = Uuid::new_v4();
        let mut tracker = CorrelationTracker::new(session_id);

        // Test correlation ID generation
        let id1 = tracker.generate_correlation_id();
        let id2 = tracker.generate_correlation_id();
        assert_ne!(id1, id2);
        assert!(id1.contains(&session_id.to_string()));

        // Test adding and removing pending requests
        tracker.add_pending_request(id1.clone(), "test message".to_string());
        assert_eq!(tracker.pending_count(), 1);

        let removed = tracker.remove_pending_request(&id1);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().original_message, "test message");
        assert_eq!(tracker.pending_count(), 0);

        // Test removing non-existent request
        let removed = tracker.remove_pending_request("non-existent");
        assert!(removed.is_none());
    }

    #[tokio::test]
    async fn test_message_processor_json_injection() {
        // Test injecting correlation ID into JSON object
        let json_message = r#"{"method": "test", "params": {"key": "value"}}"#;
        let correlation_id = "test-corr-123";

        let result = MessageProcessor::inject_correlation_id(json_message, correlation_id);
        assert!(result.is_ok());

        let modified = result.unwrap();
        assert!(modified.contains("_correlation_id"));
        assert!(modified.contains(correlation_id));

        // Test extracting correlation ID
        let extracted = MessageProcessor::extract_correlation_id(&modified);
        assert_eq!(extracted, Some(correlation_id.to_string()));
    }

    #[tokio::test]
    async fn test_message_processor_non_json() {
        // Test with non-JSON message
        let text_message = "This is not JSON";
        let correlation_id = "test-corr-456";

        let result = MessageProcessor::inject_correlation_id(text_message, correlation_id);
        assert!(result.is_ok());

        let modified = result.unwrap();
        assert!(modified.contains("_correlation_id"));
        assert!(modified.contains(correlation_id));
        assert!(modified.contains("_original_text"));
    }

    #[tokio::test]
    async fn test_message_type_detection() {
        // Test request detection
        let request_json = r#"{"method": "get_info", "params": {}}"#;
        assert!(MessageProcessor::is_request_message(request_json));

        let request_text = "send request to server";
        assert!(MessageProcessor::is_request_message(request_text));

        // Test response detection
        let response_json = r#"{"result": {"status": "ok"}, "error": null}"#;
        assert!(MessageProcessor::is_response_message(response_json));

        let response_text = "response from server";
        assert!(MessageProcessor::is_response_message(response_text));

        // Test non-matching message
        let other_message = r#"{"notification": "update"}"#;
        assert!(!MessageProcessor::is_request_message(other_message));
        assert!(!MessageProcessor::is_response_message(other_message));
    }

    #[tokio::test]
    async fn test_correlation_timeout_cleanup() {
        let session_id = Uuid::new_v4();
        let mut tracker = CorrelationTracker::new(session_id);

        // Add a request that should be expired
        let correlation_id = tracker.generate_correlation_id();
        tracker.add_pending_request(correlation_id.clone(), "test message".to_string());

        // Manually set the sent_at time to be in the past
        if let Some(request) = tracker.pending_requests.get_mut(&correlation_id) {
            request.sent_at = Instant::now() - Duration::from_secs(120); // 2 minutes ago
        }

        assert_eq!(tracker.pending_count(), 1);

        // Run cleanup
        let expired = tracker.cleanup_expired_requests();
        assert_eq!(expired.len(), 1);
        assert_eq!(tracker.pending_count(), 0);
        assert_eq!(expired[0].correlation_id, correlation_id);
    }

    #[tokio::test]
    async fn test_proxy_session_with_correlation() {
        let manager = Arc::new(WebSocketConnectionManager::new(
            BaseUrl("ws://localhost:8290".to_string()),
            MacaroonHex("test_macaroon".to_string()),
            false,
        ));

        let _handler = WebSocketProxyHandler::new(manager);

        // Test session with correlation enabled
        let session_id = Uuid::new_v4();
        let backend_conn_id = Uuid::new_v4();
        let current_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let correlation_tracker = Some(Arc::new(Mutex::new(CorrelationTracker::new(session_id))));

        let session = ProxySession {
            id: session_id,
            client_id: "test_client".to_string(),
            backend_endpoint: "/test".to_string(),
            backend_conn_id,
            created_at: std::time::Instant::now(),
            last_activity_epoch: Arc::new(AtomicU64::new(current_epoch)),
            correlation_required: true,
            correlation_tracker,
        };

        // Verify correlation tracker is present
        assert!(session.correlation_tracker.is_some());
        assert_eq!(session.correlation_required, true);

        // Test correlation tracker functionality
        if let Some(ref tracker) = session.correlation_tracker {
            let mut tracker_guard = tracker.lock().await;
            let corr_id = tracker_guard.generate_correlation_id();
            tracker_guard.add_pending_request(corr_id.clone(), "test".to_string());
            assert_eq!(tracker_guard.pending_count(), 1);
        }
    }
}
