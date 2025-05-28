//! Cache metadata and entry structures.

use std::time::Duration;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Metadata associated with a cache entry.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CacheMetadata {
    /// When the cache entry was created (Duration since UNIX_EPOCH)
    pub created_time: Duration,
    /// Time-to-live for the cache entry
    pub ttl: Option<Duration>,
}

impl CacheMetadata {
    /// Create new cache metadata with current time
    pub fn new(ttl: Option<Duration>) -> Self {
        Self {
            created_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(Duration::ZERO),
            ttl,
        }
    }
    
    /// Create cache metadata with specific creation time
    pub fn with_time(created_time: Duration, ttl: Option<Duration>) -> Self {
        Self {
            created_time,
            ttl,
        }
    }
    
    /// Check if this cache entry is expired at the given time
    pub fn is_expired(&self, now: Duration) -> bool {
        if let Some(ttl) = self.ttl {
            now >= self.created_time + ttl
        } else {
            false // No TTL means never expires
        }
    }
    
    /// Get the expiration time for this cache entry
    pub fn expires_at(&self) -> Option<Duration> {
        self.ttl.map(|ttl| self.created_time + ttl)
    }
    
    /// Get the age of this cache entry at the given time
    pub fn age(&self, now: Duration) -> Duration {
        now.saturating_sub(self.created_time)
    }
}

/// A cache entry containing both the value and its metadata.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CacheEntry<T> {
    /// The cached value
    pub value: T,
    /// Metadata about the cache entry
    pub metadata: CacheMetadata,
}

impl<T> CacheEntry<T> {
    /// Create a new cache entry with the given value and TTL
    pub fn new(value: T, ttl: Option<Duration>) -> Self {
        Self {
            value,
            metadata: CacheMetadata::new(ttl),
        }
    }
    
    /// Create a new cache entry with specific metadata
    pub fn with_metadata(value: T, metadata: CacheMetadata) -> Self {
        Self {
            value,
            metadata,
        }
    }
    
    /// Check if this cache entry is expired
    pub fn is_expired(&self, now: Duration) -> bool {
        self.metadata.is_expired(now)
    }
    
    /// Get the expiration time for this cache entry
    pub fn expires_at(&self) -> Option<Duration> {
        self.metadata.expires_at()
    }
    
    /// Get the age of this cache entry
    pub fn age(&self, now: Duration) -> Duration {
        self.metadata.age(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_cache_metadata() {
        let ttl = Duration::from_secs(60);
        let metadata = CacheMetadata::new(Some(ttl));
        
        // Should not be expired immediately
        let now = metadata.created_time;
        assert!(!metadata.is_expired(now));
        
        // Should be expired after TTL
        let later = now + ttl + Duration::from_secs(1);
        assert!(metadata.is_expired(later));
        
        // Should expire exactly at created_time + ttl
        let expiry = now + ttl;
        assert!(metadata.is_expired(expiry));
    }
    
    #[test]
    fn test_cache_metadata_no_ttl() {
        let metadata = CacheMetadata::new(None);
        let far_future = metadata.created_time + Duration::from_secs(365 * 24 * 60 * 60);
        
        // Should never expire without TTL
        assert!(!metadata.is_expired(far_future));
    }
    
    #[test]
    fn test_cache_entry() {
        let value = "test_value".to_string();
        let ttl = Duration::from_secs(30);
        let entry = CacheEntry::new(value.clone(), Some(ttl));
        
        assert_eq!(entry.value, value);
        assert_eq!(entry.metadata.ttl, Some(ttl));
        
        // Test age calculation
        let now = entry.metadata.created_time + Duration::from_secs(10);
        assert_eq!(entry.age(now), Duration::from_secs(10));
    }
}
