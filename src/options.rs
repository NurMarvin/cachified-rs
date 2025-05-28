//! Configuration options for the cachified function
//!
//! This module provides the `CachifiedOptions` struct that configures
//! how the cachified function behaves.

use crate::{Cache, CheckValue, Result};
use std::time::Duration;
use std::future::Future;

/// Configuration options for the cachified function
///
/// This struct contains all the configuration options that control how
/// the cachified function behaves, including cache settings, TTL,
/// stale-while-revalidate, and validation options.
pub struct CachifiedOptions<T, F, C>
where
    T: Clone + Send + Sync + 'static,
    C: Cache<T> + Clone,
{
    /// The cache implementation to use
    pub cache: C,

    /// The cache key to use for storing/retrieving the value
    pub key: String,

    /// Time-to-live for cached values
    pub ttl: Option<Duration>,

    /// Stale-while-revalidate duration
    pub stale_while_revalidate: Option<Duration>,

    /// Whether to force fetching a fresh value, bypassing the cache
    pub force_fresh: bool,

    /// Whether to fall back to cached values when fresh value fetching fails
    pub fallback_to_cache: bool,

    /// Optional validator for cached values
    pub check_value: Option<Box<dyn CheckValue<T> + Send + Sync>>,

    /// Function to get a fresh value when cache miss or validation failure occurs
    pub get_fresh_value: F,
}

/// Builder for `CachifiedOptions` to make construction more ergonomic
pub struct CachifiedOptionsBuilder<T, C>
where
    T: Clone + Send + Sync + 'static,
    C: Cache<T> + Clone,
{
    cache: C,
    key: String,
    ttl: Option<Duration>,
    stale_while_revalidate: Option<Duration>,
    force_fresh: bool,
    fallback_to_cache: bool,
    check_value: Option<Box<dyn CheckValue<T> + Send + Sync>>,
}

impl<T, C> CachifiedOptionsBuilder<T, C>
where
    T: Clone + Send + Sync + 'static,
    C: Cache<T> + Clone,
{
    /// Create a new builder with required parameters
    pub fn new(cache: C, key: impl Into<String>) -> Self {
        Self {
            cache,
            key: key.into(),
            ttl: None,
            stale_while_revalidate: None,
            force_fresh: false,
            fallback_to_cache: false,
            check_value: None,
        }
    }

    /// Set the time-to-live for cached values
    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Set the stale-while-revalidate duration
    pub fn stale_while_revalidate(mut self, duration: Duration) -> Self {
        self.stale_while_revalidate = Some(duration);
        self
    }

    /// Set whether to force fetching fresh values
    pub fn force_fresh(mut self, force: bool) -> Self {
        self.force_fresh = force;
        self
    }

    /// Set whether to fall back to cache on fresh value failure
    pub fn fallback_to_cache(mut self, fallback: bool) -> Self {
        self.fallback_to_cache = fallback;
        self
    }

    /// Set a validator for cached values
    pub fn check_value<V>(mut self, validator: V) -> Self
    where
        V: CheckValue<T> + Send + Sync + 'static,
    {
        self.check_value = Some(Box::new(validator));
        self
    }

    /// Build the final `CachifiedOptions` with the fresh value function
    pub fn get_fresh_value<F, Fut>(self, get_fresh_value: F) -> CachifiedOptions<T, F, C>
    where
        F: Fn() -> Fut + Send + Sync,
        Fut: Future<Output = Result<T>> + Send,
    {
        CachifiedOptions {
            cache: self.cache,
            key: self.key,
            ttl: self.ttl,
            stale_while_revalidate: self.stale_while_revalidate,
            force_fresh: self.force_fresh,
            fallback_to_cache: self.fallback_to_cache,
            check_value: self.check_value,
            get_fresh_value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MokaCache, validation::NonNullValidator};

    #[tokio::test]
    async fn test_cachified_options_builder() {
        let cache = MokaCache::new(100);
        
        let options = CachifiedOptionsBuilder::new(cache, "test-key")
            .ttl(Duration::from_secs(300))
            .stale_while_revalidate(Duration::from_secs(60))
            .force_fresh(false)
            .fallback_to_cache(true)
            .check_value(NonNullValidator)
            .get_fresh_value(|| async { Ok(Some("test".to_string())) });

        assert_eq!(options.key, "test-key");
        assert_eq!(options.ttl, Some(Duration::from_secs(300)));
        assert_eq!(options.stale_while_revalidate, Some(Duration::from_secs(60)));
        assert!(!options.force_fresh);
        assert!(options.fallback_to_cache);
        assert!(options.check_value.is_some());
    }

    #[tokio::test]
    async fn test_cachified_options_builder_minimal() {
        let cache = MokaCache::new(100);
        
        let options = CachifiedOptionsBuilder::new(cache, "test-key")
            .get_fresh_value(|| async { Ok("test".to_string()) });

        assert_eq!(options.key, "test-key");
        assert_eq!(options.ttl, None);
        assert_eq!(options.stale_while_revalidate, None);
        assert!(!options.force_fresh);
        assert!(!options.fallback_to_cache);
        assert!(options.check_value.is_none());
    }
}
