use crate::error::AppError;
use redis::aio::ConnectionManager;
use redis::{AsyncCommands, RedisError};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use sqlx::{migrate::MigrateDatabase, Sqlite};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

#[derive(Clone)]
pub struct Database {
    sqlite_pool: Option<SqlitePool>,
    redis_conn: Option<ConnectionManager>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReceiverInfo {
    pub receiver_id: String,
    pub public_key: String,
    pub address: Option<String>,
    pub created_at: i64,
    pub last_seen: i64,
    pub is_active: bool,
    pub metadata: Option<serde_json::Value>,
}

impl Database {
    /// Creates a new database instance with optional SQLite and Redis connections
    pub async fn new(sqlite_path: Option<&str>, redis_url: Option<&str>) -> Result<Self, AppError> {
        let mut db = Database {
            sqlite_pool: None,
            redis_conn: None,
        };

        // Initialize SQLite if path provided
        if let Some(path) = sqlite_path {
            db.sqlite_pool = Some(Self::init_sqlite(path).await?);
        }

        // Initialize Redis if URL provided
        if let Some(url) = redis_url {
            db.redis_conn = Some(Self::init_redis(url).await?);
        }

        Ok(db)
    }

    /// Initialize SQLite connection and run migrations
    async fn init_sqlite(database_url: &str) -> Result<SqlitePool, AppError> {
        // Create database if it doesn't exist
        if !Sqlite::database_exists(database_url)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to check database: {e}")))?
        {
            Sqlite::create_database(database_url).await.map_err(|e| {
                AppError::DatabaseError(format!("Failed to create database: {e}"))
            })?;
            info!("Created SQLite database at: {}", database_url);
        }

        // Create connection pool
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(3))
            .connect(database_url)
            .await
            .map_err(|e| {
                AppError::DatabaseError(format!("Failed to connect to database: {e}"))
            })?;

        // Run migrations
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS receivers (
                receiver_id TEXT PRIMARY KEY,
                public_key TEXT NOT NULL,
                address TEXT,
                created_at INTEGER NOT NULL,
                last_seen INTEGER NOT NULL,
                is_active INTEGER NOT NULL DEFAULT 1,
                metadata TEXT,
                UNIQUE(public_key)
            );
            
            CREATE INDEX IF NOT EXISTS idx_receivers_public_key ON receivers(public_key);
            CREATE INDEX IF NOT EXISTS idx_receivers_address ON receivers(address);
            CREATE INDEX IF NOT EXISTS idx_receivers_is_active ON receivers(is_active);
            "#,
        )
        .execute(&pool)
        .await
        .map_err(|e| AppError::DatabaseError(format!("Failed to run migrations: {e}")))?;

        info!("SQLite database initialized successfully");
        Ok(pool)
    }

    /// Initialize Redis connection
    async fn init_redis(redis_url: &str) -> Result<ConnectionManager, AppError> {
        let client = redis::Client::open(redis_url).map_err(|e| {
            AppError::DatabaseError(format!("Failed to create Redis client: {e}"))
        })?;

        let conn_manager = ConnectionManager::new(client)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to connect to Redis: {e}")))?;

        info!("Redis connection established successfully");
        Ok(conn_manager)
    }

    /// Store receiver info in the database
    pub async fn store_receiver_info(&self, info: &ReceiverInfo) -> Result<(), AppError> {
        // Store in SQLite first if available - this is the persistent store
        if let Some(pool) = &self.sqlite_pool {
            self.store_receiver_sqlite(pool, info).await?;
        } else if self.redis_conn.is_none() {
            return Err(AppError::DatabaseError(
                "No database backend available".to_string(),
            ));
        }

        // Only update Redis cache after SQLite succeeds
        if let Some(redis_conn) = &self.redis_conn {
            if let Err(e) = self.store_receiver_redis(redis_conn.clone(), info).await {
                warn!("Failed to store in Redis cache: {}", e);
                // Note: We don't fail the operation if Redis fails since SQLite succeeded
            }
        }

        Ok(())
    }

    /// Store receiver info in Redis with TTL
    async fn store_receiver_redis(
        &self,
        mut conn: ConnectionManager,
        info: &ReceiverInfo,
    ) -> Result<(), RedisError> {
        let key = format!("receiver:{}", info.receiver_id);
        let value = serde_json::to_string(info).map_err(|e| {
            RedisError::from((
                redis::ErrorKind::IoError,
                "Serialization error",
                e.to_string(),
            ))
        })?;

        // Store with 1 hour TTL
        conn.set_ex::<_, _, ()>(&key, value, 3600).await?;

        // Also store reverse lookup by public key
        let pubkey_key = format!("pubkey:{}", info.public_key);
        conn.set_ex::<_, _, ()>(&pubkey_key, &info.receiver_id, 3600)
            .await?;

        Ok(())
    }

    /// Store receiver info in SQLite
    async fn store_receiver_sqlite(
        &self,
        pool: &SqlitePool,
        info: &ReceiverInfo,
    ) -> Result<(), AppError> {
        let metadata_json = info
            .metadata
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| AppError::SerializationError(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO receivers (receiver_id, public_key, address, created_at, last_seen, is_active, metadata)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(receiver_id) DO UPDATE SET
                last_seen = excluded.last_seen,
                is_active = excluded.is_active,
                metadata = excluded.metadata
            "#,
        )
        .bind(&info.receiver_id)
        .bind(&info.public_key)
        .bind(&info.address)
        .bind(info.created_at)
        .bind(info.last_seen)
        .bind(info.is_active as i32)
        .bind(metadata_json)
        .execute(pool)
        .await
        .map_err(|e| AppError::DatabaseError(format!("Failed to store receiver: {e}")))?;

        Ok(())
    }

    /// Get receiver info by receiver ID
    pub async fn get_receiver_info(
        &self,
        receiver_id: &str,
    ) -> Result<Option<ReceiverInfo>, AppError> {
        // Try Redis cache first
        if let Some(redis_conn) = &self.redis_conn {
            if let Ok(Some(info)) = self
                .get_receiver_redis(redis_conn.clone(), receiver_id)
                .await
            {
                return Ok(Some(info));
            }
        }

        // Fall back to SQLite
        if let Some(pool) = &self.sqlite_pool {
            self.get_receiver_sqlite(pool, receiver_id).await
        } else {
            Ok(None)
        }
    }

    /// Get receiver info from Redis
    async fn get_receiver_redis(
        &self,
        mut conn: ConnectionManager,
        receiver_id: &str,
    ) -> Result<Option<ReceiverInfo>, RedisError> {
        let key = format!("receiver:{receiver_id}");
        let value: Option<String> = conn.get(&key).await?;

        if let Some(json) = value {
            let info: ReceiverInfo = serde_json::from_str(&json).map_err(|e| {
                RedisError::from((
                    redis::ErrorKind::IoError,
                    "Deserialization error",
                    e.to_string(),
                ))
            })?;
            Ok(Some(info))
        } else {
            Ok(None)
        }
    }

    /// Get receiver info from SQLite
    async fn get_receiver_sqlite(
        &self,
        pool: &SqlitePool,
        receiver_id: &str,
    ) -> Result<Option<ReceiverInfo>, AppError> {
        let row = sqlx::query_as::<
            _,
            (
                String,
                String,
                Option<String>,
                i64,
                i64,
                i32,
                Option<String>,
            ),
        >(
            r#"
            SELECT receiver_id, public_key, address, created_at, last_seen, is_active, metadata
            FROM receivers
            WHERE receiver_id = ? AND is_active = 1
            "#,
        )
        .bind(receiver_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::DatabaseError(format!("Failed to query receiver: {e}")))?;

        if let Some((
            receiver_id,
            public_key,
            address,
            created_at,
            last_seen,
            is_active,
            metadata_json,
        )) = row
        {
            let metadata = metadata_json
                .map(|json| serde_json::from_str(&json))
                .transpose()
                .map_err(|e| AppError::SerializationError(e.to_string()))?;

            Ok(Some(ReceiverInfo {
                receiver_id,
                public_key,
                address,
                created_at,
                last_seen,
                is_active: is_active != 0,
                metadata,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get receiver ID by public key
    pub async fn get_receiver_by_public_key(
        &self,
        public_key: &str,
    ) -> Result<Option<String>, AppError> {
        // Try Redis cache first
        if let Some(redis_conn) = &self.redis_conn {
            let mut conn = redis_conn.clone();
            let key = format!("pubkey:{public_key}");
            if let Ok(Some(receiver_id)) = conn.get::<_, Option<String>>(&key).await {
                return Ok(Some(receiver_id));
            }
        }

        // Fall back to SQLite
        if let Some(pool) = &self.sqlite_pool {
            let row = sqlx::query_scalar::<_, String>(
                "SELECT receiver_id FROM receivers WHERE public_key = ? AND is_active = 1",
            )
            .bind(public_key)
            .fetch_optional(pool)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to query by public key: {e}")))?;

            Ok(row)
        } else {
            Ok(None)
        }
    }

    /// Mark receiver as inactive
    pub async fn deactivate_receiver(&self, receiver_id: &str) -> Result<(), AppError> {
        if let Some(pool) = &self.sqlite_pool {
            sqlx::query("UPDATE receivers SET is_active = 0 WHERE receiver_id = ?")
                .bind(receiver_id)
                .execute(pool)
                .await
                .map_err(|e| {
                    AppError::DatabaseError(format!("Failed to deactivate receiver: {e}"))
                })?;
        }

        // Remove from Redis cache
        if let Some(redis_conn) = &self.redis_conn {
            let mut conn = redis_conn.clone();
            let key = format!("receiver:{receiver_id}");
            let _: Result<(), _> = conn.del(&key).await;
        }

        Ok(())
    }
}

/// Global database instance wrapped in Arc for thread-safe sharing
pub type SharedDatabase = Arc<Database>;

/// Initialize the global database instance
pub async fn init_database(
    sqlite_path: Option<&str>,
    redis_url: Option<&str>,
) -> Result<SharedDatabase, AppError> {
    let db = Database::new(sqlite_path, redis_url).await?;
    Ok(Arc::new(db))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn test_receiver_info_serialization() {
        let info = ReceiverInfo {
            receiver_id: "test_receiver_123".to_string(),
            public_key: "02a1b2c3d4e5f6".to_string(),
            address: Some("taprt1abc...".to_string()),
            created_at: Utc::now().timestamp(),
            last_seen: Utc::now().timestamp(),
            is_active: true,
            metadata: Some(serde_json::json!({"type": "mailbox"})),
        };

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: ReceiverInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(info.receiver_id, deserialized.receiver_id);
        assert_eq!(info.public_key, deserialized.public_key);
    }
}
