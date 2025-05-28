//! Cache trait and implementations
//!
//! This module provides the cache abstraction and concrete implementations.
//! The main implementations include Moka (in-memory) and Redis (distributed).

use crate::{CacheEntry, Result};
use async_trait::async_trait;

#[cfg(feature = "moka")]
use moka::future::Cache as MokaFutureCache;
#[cfg(feature = "moka")]
use std::sync::Arc;

#[cfg(feature = "redis")]
use redis::{aio::MultiplexedConnection, AsyncCommands};

/// Cache trait that defines the interface for cache implementations.
///
/// This trait provides async methods for getting and setting cache entries.
/// All cache implementations should implement this trait to be compatible
/// with the cachified function.
#[async_trait]
pub trait Cache<T>: Send + Sync
where
    T: Clone + Send + Sync + 'static,
{
    /// Get a cache entry by key
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key to look up
    ///
    /// # Returns
    ///
    /// Returns `Some(CacheEntry<T>)` if the key exists, `None` otherwise.
    async fn get(&self, key: &str) -> Option<CacheEntry<T>>;

    /// Set a cache entry
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key
    /// * `entry` - The cache entry to store
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if successful, or an error if the operation fails.
    async fn set(&self, key: &str, entry: CacheEntry<T>) -> Result<()>;

    /// Remove a cache entry by key
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key to remove
    async fn remove(&self, key: &str);

    /// Clear all cache entries
    async fn clear(&self);

    /// Get the current number of entries in the cache
    async fn len(&self) -> usize;

    /// Check if the cache is empty
    async fn is_empty(&self) -> bool {
        self.len().await == 0
    }
}

/// Moka-based cache implementation
///
/// This is a high-performance in-memory cache implementation that uses the Moka library
/// for concurrent caching with automatic cleanup.
///
/// # Examples
///
/// ```rust
/// # #[cfg(feature = "moka")]
/// use cachified::MokaCache;
///
/// # #[cfg(feature = "moka")]
/// let cache: MokaCache<String> = MokaCache::new(1000);
/// ```
#[cfg(feature = "moka")]
#[derive(Clone)]
pub struct MokaCache<T> {
    inner: Arc<MokaFutureCache<String, CacheEntry<T>>>,
}

#[cfg(feature = "moka")]
impl<T> MokaCache<T>
where
    T: Clone + Send + Sync + 'static,
{
    /// Create a new MokaCache with the specified maximum capacity
    ///
    /// # Arguments
    ///
    /// * `max_capacity` - Maximum number of entries the cache can hold
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(feature = "moka")]
    /// use cachified::MokaCache;
    ///
    /// # #[cfg(feature = "moka")]
    /// let cache: MokaCache<String> = MokaCache::new(1000);
    /// ```
    pub fn new(max_capacity: u64) -> Self {
        let inner = MokaFutureCache::builder()
            .max_capacity(max_capacity)
            .build();

        Self {
            inner: Arc::new(inner),
        }
    }

    /// Get the underlying Moka cache for advanced operations
    ///
    /// This provides access to additional Moka-specific functionality
    /// that might not be exposed through the Cache trait.
    pub fn inner(&self) -> &MokaFutureCache<String, CacheEntry<T>> {
        &self.inner
    }
}

#[cfg(feature = "moka")]
#[async_trait]
impl<T> Cache<T> for MokaCache<T>
where
    T: Clone + Send + Sync + 'static,
{
    async fn get(&self, key: &str) -> Option<CacheEntry<T>> {
        self.inner.get(key).await
    }

    async fn set(&self, key: &str, entry: CacheEntry<T>) -> Result<()> {
        self.inner.insert(key.to_string(), entry).await;
        Ok(())
    }

    async fn remove(&self, key: &str) {
        self.inner.invalidate(key).await;
    }

    async fn clear(&self) {
        self.inner.invalidate_all();
    }

    async fn len(&self) -> usize {
        self.inner.entry_count() as usize
    }
}

/// Redis-based cache implementation
///
/// This is a distributed cache implementation that uses Redis for
/// storing cache entries. Requires the "redis" feature to be enabled.
///
/// # Examples
///
/// ```rust,no_run
/// # #[cfg(feature = "redis")]
/// use cachified::RedisCache;
///
/// # #[cfg(feature = "redis")]
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let cache: RedisCache<String> = RedisCache::new("redis://localhost:6379").await?;
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "redis")]
#[derive(Clone)]
pub struct RedisCache<T> {
    connection: MultiplexedConnection,
    prefix: String,
    _phantom: std::marker::PhantomData<T>,
}

#[cfg(feature = "redis")]
impl<T> RedisCache<T>
where
    T: Clone + Send + Sync + 'static,
{
    /// Create a new RedisCache with the specified Redis URL
    ///
    /// # Arguments
    ///
    /// * `redis_url` - Redis connection URL (e.g., "redis://localhost:6379")
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(feature = "redis")]
    /// use cachified::RedisCache;
    ///
    /// # #[cfg(feature = "redis")]
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let cache: RedisCache<String> = RedisCache::new("redis://localhost:6379").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url)?;
        let connection = client.get_multiplexed_async_connection().await?;
        
        Ok(Self {
            connection,
            prefix: "cachified:".to_string(),
            _phantom: std::marker::PhantomData,
        })
    }

    /// Create a new RedisCache with a custom key prefix
    ///
    /// # Arguments
    ///
    /// * `redis_url` - Redis connection URL
    /// * `prefix` - Custom prefix for all cache keys
    pub async fn with_prefix(redis_url: &str, prefix: String) -> Result<Self> {
        let client = redis::Client::open(redis_url)?;
        let connection = client.get_multiplexed_async_connection().await?;
        
        Ok(Self {
            connection,
            prefix,
            _phantom: std::marker::PhantomData,
        })
    }

    /// Get the full key with prefix
    fn full_key(&self, key: &str) -> String {
        format!("{}{}", self.prefix, key)
    }
}

#[cfg(all(feature = "redis", feature = "serde"))]
#[async_trait]
impl<T> Cache<T> for RedisCache<T>
where
    T: Clone + Send + Sync + 'static + serde::Serialize + serde::de::DeserializeOwned,
{
    async fn get(&self, key: &str) -> Option<CacheEntry<T>> {
        let mut conn = self.connection.clone();
        let full_key = self.full_key(key);
        
        match conn.get::<String, String>(full_key).await {
            Ok(data) => {
                match serde_json::from_str::<CacheEntry<T>>(&data) {
                    Ok(entry) => Some(entry),
                    Err(_) => None,
                }
            }
            Err(_) => None,
        }
    }

    async fn set(&self, key: &str, entry: CacheEntry<T>) -> Result<()> {
        let mut conn = self.connection.clone();
        let full_key = self.full_key(key);
        
        let data = serde_json::to_string(&entry)?;
        
        // Set with TTL if specified
        if let Some(ttl) = entry.metadata.ttl {
            let expire_seconds = ttl.as_secs();
            if expire_seconds > 0 {
                conn.set_ex::<String, String, ()>(full_key, data, expire_seconds).await?;
            } else {
                conn.set::<String, String, ()>(full_key, data).await?;
            }
        } else {
            conn.set::<String, String, ()>(full_key, data).await?;
        }
        
        Ok(())
    }

    async fn remove(&self, key: &str) {
        let mut conn = self.connection.clone();
        let full_key = self.full_key(key);
        let _ = conn.del::<String, ()>(full_key).await;
    }

    async fn clear(&self) {
        let mut conn = self.connection.clone();
        let pattern = format!("{}*", self.prefix);
        
        // Get all keys matching the pattern
        if let Ok(keys) = conn.keys::<String, Vec<String>>(pattern).await {
            if !keys.is_empty() {
                let _ = conn.del::<Vec<String>, ()>(keys).await;
            }
        }
    }

    async fn len(&self) -> usize {
        let mut conn = self.connection.clone();
        let pattern = format!("{}*", self.prefix);
        
        match conn.keys::<String, Vec<String>>(pattern).await {
            Ok(keys) => keys.len(),
            Err(_) => 0,
        }
    }
}

#[cfg(all(feature = "redis", not(feature = "serde")))]
compile_error!("Redis cache requires the 'serde' feature to be enabled for serialization support");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::CacheMetadata;
    use std::time::Duration;

    fn create_test_entry() -> CacheEntry<String> {
        CacheEntry {
            value: "test-value".to_string(),
            metadata: CacheMetadata {
                created_time: Duration::from_secs(1000),
                ttl: Some(Duration::from_secs(300)),
            },
        }
    }

    #[cfg(feature = "moka")]
    mod moka_tests {
        use super::*;

        #[tokio::test]
        async fn test_moka_cache_basic_operations() {
            let cache: MokaCache<String> = MokaCache::new(100);
            let entry = create_test_entry();

            // Test set and get
            cache.set("test-key", entry.clone()).await.unwrap();
            let retrieved = cache.get("test-key").await;
            assert!(retrieved.is_some());
            assert_eq!(retrieved.unwrap().value, "test-value");

            // Test remove
            cache.remove("test-key").await;
            assert!(cache.get("test-key").await.is_none());
        }

        #[tokio::test]
        async fn test_moka_cache_clear() {
            let cache: MokaCache<String> = MokaCache::new(100);
            let entry = create_test_entry();

            // Add multiple entries
            cache.set("key1", entry.clone()).await.unwrap();
            cache.set("key2", entry.clone()).await.unwrap();
            cache.set("key3", entry).await.unwrap();

            // Verify entries exist
            assert!(cache.get("key1").await.is_some());
            assert!(cache.get("key2").await.is_some());
            assert!(cache.get("key3").await.is_some());

            // Clear all
            cache.clear().await;
            
            // Verify entries are gone
            assert!(cache.get("key1").await.is_none());
            assert!(cache.get("key2").await.is_none());
            assert!(cache.get("key3").await.is_none());
        }

        #[tokio::test]
        async fn test_cache_clone() {
            let cache: MokaCache<String> = MokaCache::new(100);
            let cache_clone = cache.clone();
            let entry = create_test_entry();

            // Set in original cache
            cache.set("test-key", entry.clone()).await.unwrap();

            // Should be accessible from clone
            let retrieved = cache_clone.get("test-key").await;
            assert!(retrieved.is_some());
            assert_eq!(retrieved.unwrap().value, "test-value");
        }
    }

    #[cfg(all(feature = "redis", feature = "serde"))]
    mod redis_tests {
        use super::*;

        // Note: These tests require a running Redis instance
        // They are ignored by default to avoid failing CI/CD
        
        #[tokio::test]
        #[ignore = "requires running Redis instance"]
        async fn test_redis_cache_basic_operations() {
            let cache: RedisCache<String> = RedisCache::new("redis://localhost:6379")
                .await
                .expect("Failed to connect to Redis");
            
            let entry = create_test_entry();

            // Test set and get
            cache.set("test-key", entry.clone()).await.unwrap();
            let retrieved = cache.get("test-key").await;
            assert!(retrieved.is_some());
            assert_eq!(retrieved.unwrap().value, "test-value");

            // Test remove
            cache.remove("test-key").await;
            assert!(cache.get("test-key").await.is_none());
        }
    }
}
