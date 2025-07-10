//! WebSocket Connection Manager for proxying connections between clients and tapd backend.
//!
//! This module provides a connection manager that handles WebSocket connections to the tapd
//! backend, including:
//! - Connection pooling and lifecycle management
//! - Automatic macaroon authentication
//! - TLS configuration based on settings
//! - Connection health checking and automatic reconnection
//!
//! # Example
//!
//! ```rust,ignore
//! use taproot_assets_rest_gateway::websocket::connection_manager::WebSocketConnectionManager;
//! use taproot_assets_rest_gateway::types::{BaseUrl, MacaroonHex};
//! use std::sync::Arc;
//!
//! let backend_url = BaseUrl("https://localhost:8089".to_string());
//! let macaroon_hex = MacaroonHex("deadbeef".to_string());
//! let manager = Arc::new(WebSocketConnectionManager::new(backend_url, macaroon_hex, false));
//!
//! // Connect to backend
//! let (conn_id, sink, stream) = manager.connect_to_backend("/v1/taproot-assets/subscribe/send").await?;
//!
//! // Start health monitoring
//! let health_check_handle = manager.clone().start_health_check_task();
//! ```

use crate::error::AppError;
use crate::types::{BaseUrl, MacaroonHex};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::time::interval;
use tokio_tungstenite::{
    connect_async_tls_with_config, tungstenite::protocol::Message, Connector, MaybeTlsStream,
    WebSocketStream,
};
use tracing::{debug, error, info, warn};
use url::Url;
use uuid::Uuid;

/// Default interval for health check monitoring (in seconds)
const DEFAULT_HEALTH_CHECK_INTERVAL_SECS: u64 = 30;

/// Default maximum idle time before considering a connection stale (in seconds)
const DEFAULT_MAX_IDLE_SECS: u64 = 300; // 5 minutes

/// Maximum number of reconnection attempts
const MAX_RECONNECT_ATTEMPTS: u32 = 3;

/// Initial reconnection delay (in seconds)
const INITIAL_RECONNECT_DELAY_SECS: u64 = 1;

/// Maximum reconnection delay (in seconds) - caps exponential backoff
const MAX_RECONNECT_DELAY_SECS: u64 = 60;

/// Timeout for reconnection health checks (in seconds)
const RECONNECT_HEALTH_TIMEOUT_SECS: u64 = 60;

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsSink = futures_util::stream::SplitSink<WsStream, Message>;
type WsStreamSplit = futures_util::stream::SplitStream<WsStream>;

/// WebSocket connection manager for proxying connections to tapd backend
pub struct WebSocketConnectionManager {
    backend_url: String,
    macaroon_hex: String,
    tls_verify: bool,
    connections: Arc<Mutex<HashMap<Uuid, BackendConnection>>>,
}

/// Represents a tracked WebSocket connection to the backend
#[derive(Debug)]
pub struct BackendConnection {
    pub id: Uuid,
    pub endpoint: String,
    pub created_at: Instant,
    pub last_activity: Arc<Mutex<Instant>>,
}

impl Clone for WebSocketConnectionManager {
    fn clone(&self) -> Self {
        Self {
            backend_url: self.backend_url.clone(),
            macaroon_hex: self.macaroon_hex.clone(),
            tls_verify: self.tls_verify,
            connections: self.connections.clone(),
        }
    }
}

impl WebSocketConnectionManager {
    pub fn new(backend_url: BaseUrl, macaroon_hex: MacaroonHex, tls_verify: bool) -> Self {
        Self {
            backend_url: backend_url.0,
            macaroon_hex: macaroon_hex.0,
            tls_verify,
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Establish a WebSocket connection to the tapd backend
    pub async fn connect_to_backend(
        &self,
        endpoint: &str,
    ) -> Result<(Uuid, WsSink, WsStreamSplit), AppError> {
        // Convert https to wss URL
        let ws_url = self
            .backend_url
            .replace("https://", "wss://")
            .replace("http://", "ws://");
        let url = format!("{ws_url}{endpoint}");
        debug!("Connecting to backend WebSocket: {}", url);

        // Extract host from URL using proper URL parsing
        let host = Url::parse(&self.backend_url)
            .map_err(|e| AppError::WebSocketProxyError(format!("Invalid backend URL: {e}")))?
            .host_str()
            .unwrap_or("localhost")
            .to_string();

        // Build request with macaroon authentication
        let request = tokio_tungstenite::tungstenite::http::Request::builder()
            .uri(&url)
            .header("Grpc-Metadata-macaroon", &self.macaroon_hex)
            .header("Sec-WebSocket-Protocol", "Grpc-Metadata-macaroon")
            .header("Host", host)
            .header("User-Agent", "taproot-assets-rest-gateway/0.0.1")
            .body(())
            .map_err(|e| AppError::WebSocketProxyError(format!("Failed to build request: {e}")))?;

        // Configure TLS
        let connector = if self.tls_verify {
            Connector::NativeTls(
                native_tls::TlsConnector::new()
                    .map_err(|e| AppError::WebSocketError(format!("TLS error: {e}")))?,
            )
        } else {
            Connector::NativeTls(
                native_tls::TlsConnector::builder()
                    .danger_accept_invalid_certs(true)
                    .danger_accept_invalid_hostnames(true)
                    .build()
                    .map_err(|e| AppError::WebSocketError(format!("TLS error: {e}")))?,
            )
        };

        // Connect to the backend
        let (ws_stream, _response) =
            connect_async_tls_with_config(request, None, false, Some(connector))
                .await
                .map_err(|e| AppError::WebSocketProxyError(format!("Failed to connect: {e}")))?;

        info!("Successfully connected to backend WebSocket: {endpoint}");

        // Split the stream for bidirectional communication
        let (sink, stream) = ws_stream.split();

        // Create connection tracking
        let connection_id = Uuid::new_v4();
        let connection = BackendConnection {
            id: connection_id,
            endpoint: endpoint.to_string(),
            created_at: Instant::now(),
            last_activity: Arc::new(Mutex::new(Instant::now())),
        };

        // Store connection info (without the sink - caller owns it)
        let mut connections = self.connections.lock().await;
        connections.insert(connection_id, connection);

        Ok((connection_id, sink, stream))
    }

    /// Remove a connection from the pool
    pub async fn remove_connection(&self, connection_id: Uuid) -> Option<BackendConnection> {
        let mut connections = self.connections.lock().await;
        connections.remove(&connection_id)
    }

    /// Get all active connection IDs
    pub async fn get_connection_ids(&self) -> Vec<Uuid> {
        let connections = self.connections.lock().await;
        connections.keys().copied().collect()
    }

    /// Get connection info
    pub async fn get_connection_info(&self, connection_id: Uuid) -> Option<ConnectionInfo> {
        let connections = self.connections.lock().await;
        connections.get(&connection_id).map(|conn| ConnectionInfo {
            id: conn.id,
            endpoint: conn.endpoint.clone(),
            created_at: conn.created_at,
            last_activity: conn.last_activity.clone(),
        })
    }

    /// Update last activity timestamp for a connection
    pub async fn update_activity(&self, connection_id: Uuid) {
        let connections = self.connections.lock().await;
        if let Some(conn) = connections.get(&connection_id) {
            let mut last_activity = conn.last_activity.lock().await;
            *last_activity = Instant::now();
        }
    }

    /// Clean up stale connections (connections inactive for more than the specified duration)
    pub async fn cleanup_stale_connections(&self, max_idle_secs: u64) -> Vec<Uuid> {
        let mut connections = self.connections.lock().await;
        let now = Instant::now();
        let mut removed = Vec::new();

        let mut to_remove = Vec::new();
        for (id, conn) in connections.iter() {
            let last_activity = conn.last_activity.blocking_lock();
            let idle_duration = now.duration_since(*last_activity);

            if idle_duration.as_secs() > max_idle_secs {
                warn!(
                    "Removing stale connection {} (idle for {:?})",
                    id, idle_duration
                );
                to_remove.push(*id);
            }
        }

        for id in &to_remove {
            connections.remove(id);
            removed.push(*id);
        }

        removed
    }

    /// Reconnect a specific connection with retry logic
    pub async fn reconnect(
        &self,
        connection_id: Uuid,
    ) -> Result<(WsSink, WsStreamSplit), AppError> {
        let endpoint = {
            let connections = self.connections.lock().await;
            connections
                .get(&connection_id)
                .map(|conn| conn.endpoint.clone())
                .ok_or_else(|| AppError::WebSocketError("Connection not found".to_string()))?
        };

        // Remove the old connection
        self.remove_connection(connection_id).await;

        // Try to reconnect with exponential backoff
        let mut retry_count = 0;
        let mut delay = Duration::from_secs(INITIAL_RECONNECT_DELAY_SECS);

        loop {
            match self.connect_to_backend(&endpoint).await {
                Ok((new_id, sink, stream)) => {
                    info!(
                        "Reconnected {} -> {} for endpoint {}",
                        connection_id, new_id, endpoint
                    );
                    return Ok((sink, stream));
                }
                Err(e) => {
                    retry_count += 1;
                    if retry_count >= MAX_RECONNECT_ATTEMPTS {
                        error!(
                            "Failed to reconnect after {} attempts: {}",
                            MAX_RECONNECT_ATTEMPTS, e
                        );
                        return Err(e);
                    }

                    warn!(
                        "Reconnection attempt {} failed, retrying in {:?}: {}",
                        retry_count, delay, e
                    );
                    tokio::time::sleep(delay).await;
                    delay = std::cmp::min(delay * 2, Duration::from_secs(MAX_RECONNECT_DELAY_SECS));
                    // Exponential backoff with cap
                }
            }
        }
    }

    /// Attempt to reconnect all failed connections
    pub async fn reconnect_all_failed(&self) -> Vec<(Uuid, Result<(), AppError>)> {
        let connection_ids = self.get_connection_ids().await;
        let mut results = Vec::new();

        for conn_id in connection_ids {
            // Check if connection is unhealthy (idle for more than the timeout threshold)
            if !self
                .is_connection_healthy(conn_id, RECONNECT_HEALTH_TIMEOUT_SECS)
                .await
            {
                let result = match self.reconnect(conn_id).await {
                    Ok(_) => Ok(()),
                    Err(e) => Err(e),
                };
                results.push((conn_id, result));
            }
        }

        results
    }

    /// Get the total number of active connections
    pub async fn connection_count(&self) -> usize {
        let connections = self.connections.lock().await;
        connections.len()
    }

    /// Update connection activity timestamp
    ///
    /// Note: Actual WebSocket ping/pong must be handled by the caller who owns the sink.
    /// This method only updates the activity tracking for connection health monitoring.
    pub async fn mark_connection_active(&self, connection_id: Uuid) -> Result<(), AppError> {
        let connections = self.connections.lock().await;

        if connections.contains_key(&connection_id) {
            drop(connections);
            self.update_activity(connection_id).await;
            Ok(())
        } else {
            Err(AppError::WebSocketError("Connection not found".to_string()))
        }
    }

    /// Start a background task to monitor connection health
    pub fn start_health_check_task(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(DEFAULT_HEALTH_CHECK_INTERVAL_SECS));

            loop {
                interval.tick().await;

                // Get stale connections
                let stale_connections = self.cleanup_stale_connections(DEFAULT_MAX_IDLE_SECS).await;

                if !stale_connections.is_empty() {
                    info!(
                        "Cleaned up {} stale WebSocket connections",
                        stale_connections.len()
                    );
                }

                // Log current connection count
                let count = self.connection_count().await;
                if count > 0 {
                    debug!("Active WebSocket connections: {}", count);
                }
            }
        })
    }

    /// Check if a connection is healthy based on its last activity
    pub async fn is_connection_healthy(&self, connection_id: Uuid, max_idle_secs: u64) -> bool {
        if let Some(info) = self.get_connection_info(connection_id).await {
            let idle_duration = info.idle_duration().await;
            idle_duration.as_secs() <= max_idle_secs
        } else {
            false
        }
    }

    /// Gracefully shutdown all connections
    pub async fn shutdown_all(&self) -> Vec<Uuid> {
        let mut connections = self.connections.lock().await;
        let ids: Vec<Uuid> = connections.keys().copied().collect();

        info!("Shutting down {} WebSocket connections", ids.len());
        connections.clear();

        ids
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub id: Uuid,
    pub endpoint: String,
    pub created_at: Instant,
    pub last_activity: Arc<Mutex<Instant>>,
}

impl ConnectionInfo {
    pub async fn idle_duration(&self) -> std::time::Duration {
        let last_activity = self.last_activity.lock().await;
        Instant::now().duration_since(*last_activity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn create_test_manager() -> WebSocketConnectionManager {
        let backend_url = BaseUrl("https://localhost:8089".to_string());
        let macaroon_hex = MacaroonHex("deadbeef".to_string());
        WebSocketConnectionManager::new(backend_url, macaroon_hex, false)
    }

    #[tokio::test]
    async fn test_connection_manager_creation() {
        let manager = create_test_manager();
        assert_eq!(manager.connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_connection_tracking() {
        let manager = create_test_manager();

        // Initial count should be 0
        assert_eq!(manager.connection_count().await, 0);

        // Get connection IDs (should be empty)
        let ids = manager.get_connection_ids().await;
        assert!(ids.is_empty());
    }

    #[tokio::test]
    async fn test_connection_info() {
        let manager = create_test_manager();
        let fake_id = Uuid::new_v4();

        // Should return None for non-existent connection
        let info = manager.get_connection_info(fake_id).await;
        assert!(info.is_none());
    }

    #[tokio::test]
    async fn test_remove_nonexistent_connection() {
        let manager = create_test_manager();
        let fake_id = Uuid::new_v4();

        // Should return None when removing non-existent connection
        let removed = manager.remove_connection(fake_id).await;
        assert!(removed.is_none());
    }

    #[tokio::test]
    async fn test_update_activity() {
        let manager = create_test_manager();
        let fake_id = Uuid::new_v4();

        // Should not panic when updating non-existent connection
        manager.update_activity(fake_id).await;
    }

    #[tokio::test]
    async fn test_is_connection_healthy() {
        let manager = create_test_manager();
        let fake_id = Uuid::new_v4();

        // Non-existent connection should be unhealthy
        assert!(!manager.is_connection_healthy(fake_id, 60).await);
    }

    #[tokio::test]
    async fn test_cleanup_stale_connections() {
        let manager = create_test_manager();

        // With no connections, cleanup should return empty vec
        let cleaned = manager.cleanup_stale_connections(60).await;
        assert!(cleaned.is_empty());
    }

    #[tokio::test]
    async fn test_mark_connection_active_nonexistent() {
        let manager = create_test_manager();
        let fake_id = Uuid::new_v4();

        // Should return error for non-existent connection
        let result = manager.mark_connection_active(fake_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_reconnect_nonexistent_connection() {
        let manager = create_test_manager();
        let fake_id = Uuid::new_v4();

        // Should return error for non-existent connection
        let result = manager.reconnect(fake_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_reconnect_all_failed_empty() {
        let manager = create_test_manager();

        // With no connections, should return empty vec
        let results = manager.reconnect_all_failed().await;
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_connection_info_idle_duration() {
        let info = ConnectionInfo {
            id: Uuid::new_v4(),
            endpoint: "/test".to_string(),
            created_at: Instant::now(),
            last_activity: Arc::new(Mutex::new(Instant::now())),
        };

        // Sleep briefly to ensure some idle time
        tokio::time::sleep(Duration::from_millis(10)).await;

        let idle = info.idle_duration().await;
        assert!(idle.as_millis() >= 10);
    }

    #[tokio::test]
    async fn test_health_check_task() {
        let manager = Arc::new(create_test_manager());

        // Start health check task
        let handle = manager.clone().start_health_check_task();

        // Let it run briefly
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Cancel the task
        handle.abort();

        // Should complete without panic
        let _ = handle.await;
    }

    #[tokio::test]
    async fn test_shutdown_all() {
        let manager = create_test_manager();

        // Initially no connections
        assert_eq!(manager.connection_count().await, 0);

        // Shutdown should return empty vec
        let shutdown_ids = manager.shutdown_all().await;
        assert!(shutdown_ids.is_empty());

        // Still no connections after shutdown
        assert_eq!(manager.connection_count().await, 0);
    }
}
