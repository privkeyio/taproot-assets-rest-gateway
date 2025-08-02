use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// WebSocket connection metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketMetrics {
    pub active_connections: usize,
    pub total_connections: u64,
    pub total_messages_sent: u64,
    pub total_messages_received: u64,
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
    pub average_connection_duration: Duration,
    pub longest_connection_duration: Duration,
    pub failed_connections: u64,
    pub auth_failures: u64,
    pub rate_limit_hits: u64,
}

impl Default for WebSocketMetrics {
    fn default() -> Self {
        Self {
            active_connections: 0,
            total_connections: 0,
            total_messages_sent: 0,
            total_messages_received: 0,
            total_bytes_sent: 0,
            total_bytes_received: 0,
            average_connection_duration: Duration::ZERO,
            longest_connection_duration: Duration::ZERO,
            failed_connections: 0,
            auth_failures: 0,
            rate_limit_hits: 0,
        }
    }
}

/// Individual connection info
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub id: String,
    pub receiver_id: Option<String>,
    pub remote_addr: String,
    pub connected_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub state: String,
}

/// Monitoring service for WebSocket connections
pub struct MonitoringService {
    metrics: Arc<RwLock<WebSocketMetrics>>,
    connections: Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    connection_durations: Arc<RwLock<Vec<Duration>>>,
}

impl Default for MonitoringService {
    fn default() -> Self {
        Self::new()
    }
}

impl MonitoringService {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(WebSocketMetrics::default())),
            connections: Arc::new(RwLock::new(HashMap::new())),
            connection_durations: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Record a new connection
    pub async fn record_connection(&self, connection_id: String, remote_addr: String) {
        let mut metrics = self.metrics.write().await;
        metrics.active_connections += 1;
        metrics.total_connections += 1;

        let connection_info = ConnectionInfo {
            id: connection_id.clone(),
            receiver_id: None,
            remote_addr,
            connected_at: Utc::now(),
            last_activity: Utc::now(),
            messages_sent: 0,
            messages_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
            state: "connected".to_string(),
        };

        let mut connections = self.connections.write().await;
        connections.insert(connection_id, connection_info);

        debug!(
            "Recorded new connection. Active: {}",
            metrics.active_connections
        );
    }

    /// Update connection with receiver ID after authentication
    pub async fn update_receiver_id(&self, connection_id: &str, receiver_id: String) {
        let mut connections = self.connections.write().await;
        if let Some(conn) = connections.get_mut(connection_id) {
            conn.receiver_id = Some(receiver_id);
            conn.state = "authenticated".to_string();
        }
    }

    /// Record a message sent
    pub async fn record_message_sent(&self, connection_id: &str, size: usize) {
        let mut metrics = self.metrics.write().await;
        metrics.total_messages_sent += 1;
        metrics.total_bytes_sent += size as u64;

        let mut connections = self.connections.write().await;
        if let Some(conn) = connections.get_mut(connection_id) {
            conn.messages_sent += 1;
            conn.bytes_sent += size as u64;
            conn.last_activity = Utc::now();
        }
    }

    /// Record a message received
    pub async fn record_message_received(&self, connection_id: &str, size: usize) {
        let mut metrics = self.metrics.write().await;
        metrics.total_messages_received += 1;
        metrics.total_bytes_received += size as u64;

        let mut connections = self.connections.write().await;
        if let Some(conn) = connections.get_mut(connection_id) {
            conn.messages_received += 1;
            conn.bytes_received += size as u64;
            conn.last_activity = Utc::now();
        }
    }

    /// Record connection closure
    pub async fn record_connection_closed(&self, connection_id: &str) {
        let mut connections = self.connections.write().await;
        if let Some(conn) = connections.remove(connection_id) {
            let duration = Utc::now()
                .signed_duration_since(conn.connected_at)
                .to_std()
                .unwrap_or(Duration::ZERO);

            let mut durations = self.connection_durations.write().await;
            durations.push(duration);

            // Keep only last 1000 durations for average calculation
            if durations.len() > 1000 {
                durations.remove(0);
            }

            let mut metrics = self.metrics.write().await;
            metrics.active_connections = metrics.active_connections.saturating_sub(1);

            // Update average and longest duration
            if !durations.is_empty() {
                let total_duration: Duration = durations.iter().sum();
                metrics.average_connection_duration = total_duration / durations.len() as u32;
            }

            if duration > metrics.longest_connection_duration {
                metrics.longest_connection_duration = duration;
            }

            info!(
                "Connection closed. ID: {}, Duration: {:?}, Messages sent/received: {}/{}",
                connection_id, duration, conn.messages_sent, conn.messages_received
            );
        }
    }

    /// Record authentication failure
    pub async fn record_auth_failure(&self, connection_id: &str) {
        let mut metrics = self.metrics.write().await;
        metrics.auth_failures += 1;

        let mut connections = self.connections.write().await;
        if let Some(conn) = connections.get_mut(connection_id) {
            conn.state = "auth_failed".to_string();
        }
    }

    /// Record rate limit hit
    pub async fn record_rate_limit_hit(&self, connection_id: &str) {
        let mut metrics = self.metrics.write().await;
        metrics.rate_limit_hits += 1;

        let mut connections = self.connections.write().await;
        if let Some(conn) = connections.get_mut(connection_id) {
            conn.state = "rate_limited".to_string();
        }
    }

    /// Record failed connection
    pub async fn record_failed_connection(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.failed_connections += 1;
    }

    /// Get current metrics
    pub async fn get_metrics(&self) -> WebSocketMetrics {
        self.metrics.read().await.clone()
    }

    /// Get active connections
    pub async fn get_active_connections(&self) -> Vec<ConnectionInfo> {
        self.connections.read().await.values().cloned().collect()
    }

    /// Get connection info by ID
    pub async fn get_connection_info(&self, connection_id: &str) -> Option<ConnectionInfo> {
        self.connections.read().await.get(connection_id).cloned()
    }

    /// Clean up stale connections (connections inactive for more than 10 minutes)
    pub async fn cleanup_stale_connections(&self) {
        let now = Utc::now();
        let stale_threshold = chrono::Duration::minutes(10);

        let mut connections = self.connections.write().await;
        let mut stale_ids = Vec::new();

        for (id, conn) in connections.iter() {
            if now.signed_duration_since(conn.last_activity) > stale_threshold {
                stale_ids.push(id.clone());
            }
        }

        for id in stale_ids {
            if let Some(conn) = connections.remove(&id) {
                info!(
                    "Cleaned up stale connection: {} (inactive since {})",
                    id, conn.last_activity
                );

                let mut metrics = self.metrics.write().await;
                metrics.active_connections = metrics.active_connections.saturating_sub(1);
            }
        }
    }
}

/// Global monitoring instance
pub type SharedMonitoring = Arc<MonitoringService>;

/// Create a shared monitoring service
pub fn create_monitoring_service() -> SharedMonitoring {
    Arc::new(MonitoringService::new())
}

/// Periodic cleanup task
pub async fn run_cleanup_task(monitoring: SharedMonitoring) {
    let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes

    loop {
        interval.tick().await;
        monitoring.cleanup_stale_connections().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_lifecycle() {
        let monitoring = MonitoringService::new();
        let conn_id = "test_conn_1";

        // Record new connection
        monitoring
            .record_connection(conn_id.to_string(), "127.0.0.1:12345".to_string())
            .await;
        let metrics = monitoring.get_metrics().await;
        assert_eq!(metrics.active_connections, 1);
        assert_eq!(metrics.total_connections, 1);

        // Update with receiver ID
        monitoring
            .update_receiver_id(conn_id, "receiver_123".to_string())
            .await;
        let conn_info = monitoring.get_connection_info(conn_id).await.unwrap();
        assert_eq!(conn_info.receiver_id, Some("receiver_123".to_string()));
        assert_eq!(conn_info.state, "authenticated");

        // Record messages
        monitoring.record_message_sent(conn_id, 100).await;
        monitoring.record_message_received(conn_id, 200).await;

        let metrics = monitoring.get_metrics().await;
        assert_eq!(metrics.total_messages_sent, 1);
        assert_eq!(metrics.total_messages_received, 1);
        assert_eq!(metrics.total_bytes_sent, 100);
        assert_eq!(metrics.total_bytes_received, 200);

        // Close connection
        monitoring.record_connection_closed(conn_id).await;
        let metrics = monitoring.get_metrics().await;
        assert_eq!(metrics.active_connections, 0);
    }

    #[tokio::test]
    async fn test_multiple_connections() {
        let monitoring = MonitoringService::new();

        // Create multiple connections
        for i in 0..5 {
            monitoring
                .record_connection(format!("conn_{}", i), format!("127.0.0.1:{}", 12345 + i))
                .await;
        }

        let metrics = monitoring.get_metrics().await;
        assert_eq!(metrics.active_connections, 5);
        assert_eq!(metrics.total_connections, 5);

        // Close some connections
        monitoring.record_connection_closed("conn_1").await;
        monitoring.record_connection_closed("conn_3").await;

        let metrics = monitoring.get_metrics().await;
        assert_eq!(metrics.active_connections, 3);
    }
}
