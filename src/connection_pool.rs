use crate::error::AppError;
use reqwest::{Client, ClientBuilder};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, info, warn};

/// Configuration for the connection pool
#[derive(Clone)]
pub struct PoolConfig {
    pub max_connections: usize,
    pub connection_timeout: Duration,
    pub request_timeout: Duration,
    pub idle_timeout: Option<Duration>,
    pub pool_timeout: Duration,
    pub tls_verify: bool,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 100,
            connection_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            idle_timeout: Some(Duration::from_secs(90)),
            pool_timeout: Duration::from_secs(5),
            tls_verify: true,
        }
    }
}

/// A connection pool manager for HTTP clients
pub struct ConnectionPool {
    client: Client,
    semaphore: Arc<Semaphore>,
    config: PoolConfig,
    stats: Arc<RwLock<PoolStats>>,
}

#[derive(Default, Clone, Debug)]
pub struct PoolStats {
    pub total_requests: u64,
    pub active_connections: usize,
    pub failed_requests: u64,
    pub timeout_errors: u64,
}

impl ConnectionPool {
    /// Creates a new connection pool with the given configuration
    pub fn new(config: PoolConfig) -> Result<Self, AppError> {
        let mut client_builder = ClientBuilder::new()
            .connect_timeout(config.connection_timeout)
            .timeout(config.request_timeout)
            .pool_max_idle_per_host(config.max_connections)
            .tcp_keepalive(Duration::from_secs(60));

        if let Some(idle_timeout) = config.idle_timeout {
            client_builder = client_builder.pool_idle_timeout(idle_timeout);
        }

        if !config.tls_verify {
            warn!("TLS verification disabled - use only for development!");
            client_builder = client_builder.danger_accept_invalid_certs(true);
        }

        let client = client_builder
            .build()
            .map_err(|e| AppError::InvalidInput(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self {
            client,
            semaphore: Arc::new(Semaphore::new(config.max_connections)),
            config,
            stats: Arc::new(RwLock::new(PoolStats::default())),
        })
    }

    /// Get a client from the pool
    pub async fn get_client(&self) -> Result<PooledClient, AppError> {
        // Try to acquire a permit with timeout
        let permit = match tokio::time::timeout(
            self.config.pool_timeout,
            self.semaphore.clone().acquire_owned(),
        )
        .await
        {
            Ok(Ok(permit)) => permit,
            Ok(Err(_)) => {
                return Err(AppError::InvalidInput(
                    "Failed to acquire semaphore permit".to_string(),
                ));
            }
            Err(_) => {
                let mut stats = self.stats.write().await;
                stats.timeout_errors += 1;
                return Err(AppError::InvalidInput(
                    "Connection pool timeout - all connections busy".to_string(),
                ));
            }
        };

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.total_requests += 1;
            stats.active_connections += 1;
        }

        debug!(
            "Acquired connection from pool (active: {})",
            self.semaphore.available_permits()
        );

        Ok(PooledClient {
            client: self.client.clone(),
            _permit: permit,
            stats: self.stats.clone(),
        })
    }

    /// Get pool statistics
    pub async fn get_stats(&self) -> PoolStats {
        self.stats.read().await.clone()
    }

    /// Get the underlying reqwest client (for compatibility)
    pub fn client(&self) -> &Client {
        &self.client
    }
}

/// A client borrowed from the connection pool
#[derive(Debug)]
pub struct PooledClient {
    client: Client,
    _permit: tokio::sync::OwnedSemaphorePermit,
    stats: Arc<RwLock<PoolStats>>,
}

impl PooledClient {
    /// Get the underlying reqwest client
    pub fn client(&self) -> &Client {
        &self.client
    }
}

impl Drop for PooledClient {
    fn drop(&mut self) {
        // Update stats when connection is returned - use a safe approach that won't panic
        let stats = self.stats.clone();

        // Try to update stats using try_write to avoid blocking or panicking
        if let Ok(mut stats_guard) = stats.try_write() {
            stats_guard.active_connections = stats_guard.active_connections.saturating_sub(1);
            debug!("Returned connection to pool");
            return;
        }

        // If we can't acquire the lock immediately, spawn a non-blocking task
        // This is safer than blocking in Drop which can cause deadlocks or panics
        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::spawn(async move {
                let mut stats_guard = stats.write().await;
                stats_guard.active_connections = stats_guard.active_connections.saturating_sub(1);
                debug!("Returned connection to pool (async)");
            });
        } else {
            warn!("Cannot update connection stats: not in tokio runtime context");
        }
    }
}

impl std::ops::Deref for PooledClient {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

/// Create a shared connection pool instance
pub fn create_connection_pool(config: PoolConfig) -> Result<Arc<ConnectionPool>, AppError> {
    let pool = ConnectionPool::new(config)?;
    info!(
        "Created connection pool with {} max connections",
        pool.config.max_connections
    );
    Ok(Arc::new(pool))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_pool_creation() {
        let config = PoolConfig {
            max_connections: 10,
            ..Default::default()
        };

        let pool = ConnectionPool::new(config).unwrap();
        assert_eq!(pool.semaphore.available_permits(), 10);
    }

    #[tokio::test]
    async fn test_acquire_and_release() {
        let config = PoolConfig {
            max_connections: 2,
            ..Default::default()
        };

        let pool = Arc::new(ConnectionPool::new(config).unwrap());

        // Acquire first connection
        let client1 = pool.get_client().await.unwrap();
        assert_eq!(pool.semaphore.available_permits(), 1);

        // Acquire second connection
        let _client2 = pool.get_client().await.unwrap();
        assert_eq!(pool.semaphore.available_permits(), 0);

        // Drop first connection
        drop(client1);
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Should be able to acquire again
        let _client3 = pool.get_client().await.unwrap();
    }

    #[tokio::test]
    async fn test_pool_timeout() {
        let config = PoolConfig {
            max_connections: 1,
            pool_timeout: Duration::from_millis(100),
            ..Default::default()
        };

        let pool = Arc::new(ConnectionPool::new(config).unwrap());

        // Acquire the only connection
        let _client1 = pool.get_client().await.unwrap();

        // Try to acquire another - should timeout
        let result = pool.get_client().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timeout"));
    }
}
