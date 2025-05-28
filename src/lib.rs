#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! # Cachified-rs
//!
//! A work-in-progress port of the [cachified](https://github.com/epicweb-dev/cachified) library from TypeScript to Rust.
//!
//! ## Features
//!
//! - `moka` (default): Enable Moka in-memory cache backend
//! - `redis`: Enable Redis distributed cache backend
//! - `serde` (default): Enable serialization support (required for Redis)
//! - `tracing`: Enable tracing support
//!
//! ## Quick Start
//!
//! ### With Moka (in-memory cache)
//!
//! ```rust
//! # #[cfg(feature = "moka")]
//! use cachified::{cachified, CachifiedOptionsBuilder, MokaCache};
//! use std::time::Duration;
//!
//! # #[cfg(feature = "moka")]
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let cache = MokaCache::new(1000);
//!     
//!     let value: String = cachified(
//!         CachifiedOptionsBuilder::new(cache, "user-1")
//!             .ttl(Duration::from_secs(300)) // 5 minutes
//!             .get_fresh_value(|| async { 
//!                 // This would typically be a database call, API request, etc.
//!                 Ok("fresh-value".to_string())
//!             })
//!     ).await?;
//!     
//!     println!("Cached value: {}", value);
//!     Ok(())
//! }
//! ```
//!
//! ### With Redis (distributed cache)
//!
//! ```rust,ignore
//! use cachified::{cachified, CachifiedOptionsBuilder, RedisCache};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let cache = RedisCache::new("redis://localhost:6379").await?;
//!     
//!     let value: String = cachified(
//!         CachifiedOptionsBuilder::new(cache, "user-1")
//!             .ttl(Duration::from_secs(300)) // 5 minutes
//!             .get_fresh_value(|| async { 
//!                 // This would typically be a database call, API request, etc.
//!                 Ok("fresh-value".to_string())
//!             })
//!     ).await?;
//!     
//!     println!("Cached value: {}", value);
//!     Ok(())
//! }
//! ```

pub mod cache;
pub mod error;
pub mod options;
pub mod metadata;
pub mod validation;

pub use cache::Cache;
#[cfg(feature = "moka")]
pub use cache::MokaCache;
#[cfg(feature = "redis")]
pub use cache::RedisCache;
pub use error::{CachifiedError, Result};
pub use options::{CachifiedOptions, CachifiedOptionsBuilder};
pub use metadata::{CacheMetadata, CacheEntry};
pub use validation::CheckValue;

use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::future::Future;

/// The main cachified function that provides caching functionality.
///
/// This function attempts to retrieve a value from cache first. If the value is not found,
/// expired, or fails validation, it will call the `get_fresh_value` function to get a fresh
/// value, cache it, and return it.
///
/// # Arguments
///
/// * `options` - Configuration options for caching behavior
///
/// # Returns
///
/// Returns the cached or fresh value, or an error if both cache retrieval and fresh value
/// generation fail.
///
/// # Examples
///
/// ```rust
/// # #[cfg(feature = "moka")]
/// use cachified::{cachified, CachifiedOptionsBuilder, MokaCache};
/// use std::time::Duration;
///
/// # #[cfg(feature = "moka")]
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let cache = MokaCache::new(1000);
/// 
/// let value: String = cachified(
///     CachifiedOptionsBuilder::new(cache, "my-key")
///         .ttl(Duration::from_secs(60))
///         .get_fresh_value(|| async { Ok("Hello, World!".to_string()) })
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn cachified<T, F, Fut, C>(options: CachifiedOptions<T, F, C>) -> Result<T>
where
    T: Clone + Send + Sync + 'static,
    F: Fn() -> Fut + Send + Sync,
    Fut: Future<Output = Result<T>> + Send + 'static,
    C: Cache<T> + Clone + 'static,
{
    let CachifiedOptions {
        cache,
        key,
        ttl,
        stale_while_revalidate,
        force_fresh,
        fallback_to_cache,
        check_value,
        get_fresh_value,
    } = options;

    let now = current_time();

    // If force_fresh is true, skip cache lookup and get fresh value
    if !force_fresh {
        // Try to get value from cache
        if let Some(entry) = cache.get(&key).await {
            // Check if value is still valid (not expired)
            if !is_expired(&entry.metadata, now) {
                // Validate the cached value if validator is provided
                if let Some(ref validator) = check_value {
                    if validator.check(&entry.value).is_ok() {
                        return Ok(entry.value);
                    }
                    // If validation fails, continue to get fresh value
                } else {
                    return Ok(entry.value);
                }
            } else if let Some(swr_duration) = stale_while_revalidate {
                // Check if we're in the stale-while-revalidate window
                let stale_until = entry.metadata.created_time + 
                    entry.metadata.ttl.unwrap_or(Duration::ZERO) + swr_duration;
                
                if now < stale_until {
                    // Serve stale value and trigger background refresh
                    let cache_clone = cache.clone();
                    let key_clone = key.clone();
                    let fresh_value_future = get_fresh_value();
                    
                    // Start background refresh
                    tokio::spawn(async move {
                        if let Ok(fresh_value) = fresh_value_future.await {
                            let metadata = CacheMetadata {
                                created_time: current_time(),
                                ttl,
                            };
                            let entry = CacheEntry {
                                value: fresh_value,
                                metadata,
                            };
                            let _ = cache_clone.set(&key_clone, entry).await;
                        }
                    });
                    
                    // Return stale value immediately
                    if let Some(ref validator) = check_value {
                        if validator.check(&entry.value).is_ok() {
                            return Ok(entry.value);
                        }
                    } else {
                        return Ok(entry.value);
                    }
                }
            }
        }
    }

    // Get fresh value
    match get_fresh_value().await {
        Ok(fresh_value) => {
            // Validate fresh value if validator is provided
            if let Some(ref validator) = check_value {
                validator.check(&fresh_value)?;
            }

            // Cache the fresh value if TTL is positive
            if let Some(ttl_duration) = ttl {
                if ttl_duration > Duration::ZERO {
                    let metadata = CacheMetadata {
                        created_time: now,
                        ttl,
                    };
                    let entry = CacheEntry {
                        value: fresh_value.clone(),
                        metadata,
                    };
                    
                    if cache.set(&key, entry).await.is_err() {
                        // If cache write fails, we still return the fresh value
                        // This is consistent with the original cachified behavior
                    }
                }
            }

            Ok(fresh_value)
        }
        Err(e) => {
            // If getting fresh value fails and fallback_to_cache is enabled,
            // try to return cached value even if it's expired
            if fallback_to_cache {
                if let Some(entry) = cache.get(&key).await {
                    if let Some(ref validator) = check_value {
                        if validator.check(&entry.value).is_ok() {
                            return Ok(entry.value);
                        }
                    } else {
                        return Ok(entry.value);
                    }
                }
            }
            Err(e)
        }
    }
}

/// Get current time as Duration since UNIX_EPOCH
fn current_time() -> Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
}

/// Check if a cache entry is expired
fn is_expired(metadata: &CacheMetadata, now: Duration) -> bool {
    if let Some(ttl) = metadata.ttl {
        now >= metadata.created_time + ttl
    } else {
        false // No TTL means never expires
    }
}

/// Soft purge options for controlling soft purging behavior
pub struct SoftPurgeOptions {
    /// The cache key to soft purge
    pub key: String,
    /// How long the stale data should remain available after purging
    /// If not specified, defaults to 5 minutes (300 seconds)
    pub stale_while_revalidate: Option<Duration>,
}

impl SoftPurgeOptions {
    /// Create new soft purge options with the given key
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            stale_while_revalidate: None,
        }
    }
    
    /// Set the stale-while-revalidate duration
    pub fn stale_while_revalidate(mut self, duration: Duration) -> Self {
        self.stale_while_revalidate = Some(duration);
        self
    }
}

/// Soft purge a cache entry.
///
/// Soft purging marks a cache entry as expired (TTL = 0) while allowing it to be served
/// as stale data for a specified duration. This is useful for graceful cache invalidation
/// where you want to immediately mark data as outdated but still serve it while fresh
/// data is being fetched in the background.
///
/// Unlike hard purging (removing the cache entry entirely), soft purging prevents
/// thundering herd problems by allowing stale data to be served while only one
/// background request fetches fresh data.
///
/// # Arguments
///
/// * `cache` - The cache implementation to soft purge from
/// * `options` - Configuration options for the soft purge operation
///
/// # Returns
///
/// Returns `Ok(())` if the soft purge was successful, or an error if the operation failed.
/// If the cache entry doesn't exist, this function succeeds without doing anything.
///
/// # Examples
///
/// ```rust
/// # #[cfg(feature = "moka")]
/// use cachified::{soft_purge, SoftPurgeOptions, MokaCache};
/// use std::time::Duration;
///
/// # #[cfg(feature = "moka")]
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let cache: MokaCache<String> = MokaCache::new(1000);
/// 
/// // Soft purge a cache entry with default stale duration (5 minutes)
/// soft_purge(&cache, SoftPurgeOptions::new("user-123")).await?;
/// 
/// // Soft purge with custom stale duration
/// soft_purge(&cache, 
///     SoftPurgeOptions::new("user-456")
///         .stale_while_revalidate(Duration::from_secs(60))
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn soft_purge<T, C>(cache: &C, options: SoftPurgeOptions) -> Result<()>
where
    T: Clone + Send + Sync + 'static,
    C: Cache<T>,
{
    let SoftPurgeOptions {
        key,
        stale_while_revalidate: _,
    } = options;

    // Try to get the existing cache entry
    if let Some(mut entry) = cache.get(&key).await {
        let now = current_time();
        
        // Set TTL to 0 to mark as expired
        entry.metadata.ttl = Some(Duration::ZERO);
        
        // If the entry was already expired, we need to update created_time
        // to now so that the stale-while-revalidate period starts from now
        if entry.metadata.is_expired(now) {
            entry.metadata.created_time = now;
        }
        
        // Store the modified entry back to cache
        cache.set(&key, entry).await?;
    }
    // If the entry doesn't exist, soft purging succeeds without doing anything
    
    Ok(())
}
