//! Error types for cachified operations.

use thiserror::Error;

#[cfg(feature = "serde")]
use serde_json;

/// Result type alias for cachified operations.
pub type Result<T> = std::result::Result<T, CachifiedError>;

/// Errors that can occur during cachified operations.
#[derive(Error, Debug)]
pub enum CachifiedError {
    /// Error when getting fresh value fails
    #[error("Failed to get fresh value: {0}")]
    FreshValueError(String),
    
    /// Error when cache validation fails
    #[error("Cache validation failed: {0}")]
    ValidationError(String),
    
    /// Error when cache operations fail
    #[error("Cache operation failed: {0}")]
    CacheError(String),
    
    /// Generic error for other failures
    #[error("Cachified error: {0}")]
    Other(String),
}

impl CachifiedError {
    /// Create a new fresh value error
    pub fn fresh_value<S: Into<String>>(msg: S) -> Self {
        CachifiedError::FreshValueError(msg.into())
    }
    
    /// Create a new validation error
    pub fn validation<S: Into<String>>(msg: S) -> Self {
        CachifiedError::ValidationError(msg.into())
    }
    
    /// Create a new cache error
    pub fn cache<S: Into<String>>(msg: S) -> Self {
        CachifiedError::CacheError(msg.into())
    }
    
    /// Create a new generic error
    pub fn other<S: Into<String>>(msg: S) -> Self {
        CachifiedError::Other(msg.into())
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for CachifiedError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        CachifiedError::Other(err.to_string())
    }
}

impl From<String> for CachifiedError {
    fn from(err: String) -> Self {
        CachifiedError::Other(err)
    }
}

impl From<&str> for CachifiedError {
    fn from(err: &str) -> Self {
        CachifiedError::Other(err.to_string())
    }
}

#[cfg(feature = "redis")]
impl From<redis::RedisError> for CachifiedError {
    fn from(err: redis::RedisError) -> Self {
        CachifiedError::CacheError(format!("Redis error: {}", err))
    }
}

#[cfg(feature = "serde")]
impl From<serde_json::Error> for CachifiedError {
    fn from(err: serde_json::Error) -> Self {
        CachifiedError::Other(format!("Serialization error: {}", err))
    }
}
