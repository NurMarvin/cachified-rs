use cachified::{cachified, CachifiedOptionsBuilder, MokaCache, Cache, CachifiedError, validation::NonEmptyStringValidator};
use std::time::Duration;
use tokio::time::sleep;
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn test_basic_caching() {
    let cache = MokaCache::new(100);
    let call_count = Arc::new(Mutex::new(0));

    // First call should execute the function
    let call_count_clone = call_count.clone();
    let value1: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "test-key")
            .ttl(Duration::from_secs(60))
            .get_fresh_value(move || {
                let call_count = call_count_clone.clone();
                async move {
                    *call_count.lock().unwrap() += 1;
                    Ok("test-value".to_string())
                }
            })
    ).await.unwrap();

    assert_eq!(value1, "test-value");
    assert_eq!(*call_count.lock().unwrap(), 1);

    // Second call should use cache
    let call_count_clone = call_count.clone();
    let value2: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "test-key")
            .ttl(Duration::from_secs(60))
            .get_fresh_value(move || {
                let call_count = call_count_clone.clone();
                async move {
                    *call_count.lock().unwrap() += 1;
                    Ok("different-value".to_string())
                }
            })
    ).await.unwrap();

    assert_eq!(value2, "test-value"); // Should be cached value
    assert_eq!(*call_count.lock().unwrap(), 1); // Function should not have been called again
}

#[tokio::test]
async fn test_ttl_expiration() {
    let cache = MokaCache::new(100);
    let call_count = Arc::new(Mutex::new(0));

    // First call
    let call_count_clone = call_count.clone();
    let _: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "ttl-test")
            .ttl(Duration::from_millis(100))
            .get_fresh_value(move || {
                let call_count = call_count_clone.clone();
                async move {
                    *call_count.lock().unwrap() += 1;
                    Ok("value1".to_string())
                }
            })
    ).await.unwrap();

    assert_eq!(*call_count.lock().unwrap(), 1);

    // Wait for expiration
    sleep(Duration::from_millis(150)).await;

    // Second call should execute function again
    let call_count_clone = call_count.clone();
    let value2: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "ttl-test")
            .ttl(Duration::from_millis(100))
            .get_fresh_value(move || {
                let call_count = call_count_clone.clone();
                async move {
                    *call_count.lock().unwrap() += 1;
                    Ok("value2".to_string())
                }
            })
    ).await.unwrap();

    assert_eq!(value2, "value2");
    assert_eq!(*call_count.lock().unwrap(), 2);
}

#[tokio::test]
async fn test_stale_while_revalidate() {
    let cache = MokaCache::new(100);

    // Populate cache
    let _: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "swr-test")
            .ttl(Duration::from_millis(50))
            .get_fresh_value(|| async {
                Ok("initial-value".to_string())
            })
    ).await.unwrap();

    // Wait for expiration
    sleep(Duration::from_millis(100)).await;

    // This should return stale value while triggering background refresh
    let stale_value: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "swr-test")
            .ttl(Duration::from_millis(50))
            .stale_while_revalidate(Duration::from_secs(60))
            .get_fresh_value(|| async {
                sleep(Duration::from_millis(50)).await; // Simulate slow refresh
                Ok("fresh-value".to_string())
            })
    ).await.unwrap();

    assert_eq!(stale_value, "initial-value"); // Should get stale value immediately

    // Wait for background refresh to complete
    sleep(Duration::from_millis(100)).await;

    // Next call should get the fresh value (cached value should still be valid)
    let fresh_value: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "swr-test")
            .ttl(Duration::from_secs(60)) // Use longer TTL to ensure cache hit
            .get_fresh_value(|| async {
                Ok("another-value".to_string())
            })
    ).await.unwrap();

    assert_eq!(fresh_value, "fresh-value");
}

#[tokio::test]
async fn test_force_fresh() {
    let cache = MokaCache::new(100);

    // Populate cache
    let _: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "force-test")
            .ttl(Duration::from_secs(60))
            .get_fresh_value(|| async {
                Ok("cached-value".to_string())
            })
    ).await.unwrap();

    // Force fresh should ignore cache
    let fresh_value: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "force-test")
            .ttl(Duration::from_secs(60))
            .force_fresh(true)
            .get_fresh_value(|| async {
                Ok("forced-value".to_string())
            })
    ).await.unwrap();

    assert_eq!(fresh_value, "forced-value");
}

#[tokio::test]
async fn test_validation() {
    let cache = MokaCache::new(100);

    // First, put invalid data in cache manually
    cache.set("validation-test", cachified::CacheEntry {
        value: "".to_string(), // Empty string - will fail validation
        metadata: cachified::CacheMetadata {
            created_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap(),
            ttl: Some(Duration::from_secs(300)),
        }
    }).await.unwrap();

    // Try to get it with validation - should fetch fresh value
    let valid_value: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "validation-test")
            .ttl(Duration::from_secs(60))
            .check_value(NonEmptyStringValidator)
            .get_fresh_value(|| async {
                Ok("valid-fresh-value".to_string())
            })
    ).await.unwrap();

    assert_eq!(valid_value, "valid-fresh-value");
}

#[tokio::test]
async fn test_fallback_to_cache() {
    let cache = MokaCache::new(100);

    // Populate cache
    let _: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "fallback-test")
            .ttl(Duration::from_millis(50))
            .get_fresh_value(|| async {
                Ok("cached-value".to_string())
            })
    ).await.unwrap();

    // Wait for expiration
    sleep(Duration::from_millis(100)).await;

    // Try to get fresh value but it fails, should fallback to cache
    let fallback_value: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "fallback-test")
            .ttl(Duration::from_millis(50))
            .fallback_to_cache(true)
            .get_fresh_value(|| async {
                Err(CachifiedError::fresh_value("Simulated failure"))
            })
    ).await.unwrap();

    assert_eq!(fallback_value, "cached-value");
}

#[tokio::test]
async fn test_error_handling() {
    let cache = MokaCache::new(100);

    // Test that errors are properly propagated when no fallback is available
    let result: Result<String, CachifiedError> = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "error-test")
            .ttl(Duration::from_secs(60))
            .get_fresh_value(|| async {
                Err(CachifiedError::fresh_value("Test error"))
            })
    ).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CachifiedError::FreshValueError(msg) => assert_eq!(msg, "Test error"),
        _ => panic!("Wrong error type"),
    }
}

#[tokio::test]
async fn test_different_key_isolation() {
    let cache = MokaCache::new(100);

    // Store values with different keys
    let value1: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "key1")
            .ttl(Duration::from_secs(60))
            .get_fresh_value(|| async {
                Ok("value1".to_string())
            })
    ).await.unwrap();

    let value2: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "key2")
            .ttl(Duration::from_secs(60))
            .get_fresh_value(|| async {
                Ok("value2".to_string())
            })
    ).await.unwrap();

    assert_eq!(value1, "value1");
    assert_eq!(value2, "value2");

    // Verify they don't interfere with each other
    let value1_again: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "key1")
            .ttl(Duration::from_secs(60))
            .get_fresh_value(|| async {
                Ok("different1".to_string())
            })
    ).await.unwrap();

    assert_eq!(value1_again, "value1"); // Should still be cached
}
