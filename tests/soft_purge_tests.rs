use cachified::{cachified, soft_purge, CachifiedOptionsBuilder, MokaCache, SoftPurgeOptions, Cache, CacheEntry, CacheMetadata};
use std::time::Duration;
use tokio::time::sleep;
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn test_soft_purge_basic() {
    let cache = MokaCache::new(100);

    // First, populate the cache
    let call_count = Arc::new(Mutex::new(0));
    let call_count_clone = call_count.clone();
    
    let _: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "soft-purge-test")
            .ttl(Duration::from_secs(300)) // 5 minutes
            .get_fresh_value(move || {
                let call_count = call_count_clone.clone();
                async move {
                    *call_count.lock().unwrap() += 1;
                    Ok("original-value".to_string())
                }
            })
    ).await.unwrap();

    assert_eq!(*call_count.lock().unwrap(), 1);
    
    // Verify the cache entry exists and is not expired
    let entry = cache.get("soft-purge-test").await;
    assert!(entry.is_some());
    let entry = entry.unwrap();
    assert_eq!(entry.value, "original-value");
    assert!(!entry.is_expired(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()));

    // Soft purge the cache entry
    soft_purge(&cache, SoftPurgeOptions::new("soft-purge-test")).await.unwrap();

    // Verify the cache entry still exists but is now expired
    let entry = cache.get("soft-purge-test").await;
    assert!(entry.is_some());
    let entry = entry.unwrap();
    assert_eq!(entry.value, "original-value");
    assert!(entry.is_expired(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()));

    // Now when we call cachified with stale-while-revalidate, it should return the stale value
    // and trigger a background refresh
    let call_count_clone = call_count.clone();
    let stale_value: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "soft-purge-test")
            .ttl(Duration::from_secs(300))
            .stale_while_revalidate(Duration::from_secs(60))
            .get_fresh_value(move || {
                let call_count = call_count_clone.clone();
                async move {
                    sleep(Duration::from_millis(50)).await; // Simulate slow refresh
                    *call_count.lock().unwrap() += 1;
                    Ok("refreshed-value".to_string())
                }
            })
    ).await.unwrap();

    // Should return stale value immediately
    assert_eq!(stale_value, "original-value");
    
    // Wait for background refresh to complete
    sleep(Duration::from_millis(100)).await;

    // Fresh value function should have been called
    assert_eq!(*call_count.lock().unwrap(), 2);

    // Next call should return the refreshed value
    let call_count_clone = call_count.clone();
    let fresh_value: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "soft-purge-test")
            .ttl(Duration::from_secs(300))
            .get_fresh_value(move || {
                let call_count = call_count_clone.clone();
                async move {
                    *call_count.lock().unwrap() += 1;
                    Ok("should-not-be-called".to_string())
                }
            })
    ).await.unwrap();

    assert_eq!(fresh_value, "refreshed-value");
    assert_eq!(*call_count.lock().unwrap(), 2); // Should not have been called again
}

#[tokio::test]
async fn test_soft_purge_nonexistent_key() {
    let cache: MokaCache<String> = MokaCache::new(100);

    // Soft purging a non-existent key should succeed without error
    let result = soft_purge(&cache, SoftPurgeOptions::new("nonexistent-key")).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_soft_purge_already_expired() {
    let cache: MokaCache<String> = MokaCache::new(100);

    // Manually create an already expired entry
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    
    let expired_entry = CacheEntry {
        value: "expired-value".to_string(),
        metadata: CacheMetadata {
            created_time: now - Duration::from_secs(100),
            ttl: Some(Duration::from_secs(50)), // Expired 50 seconds ago
        },
    };
    
    cache.set("expired-test", expired_entry).await.unwrap();

    // Soft purge the already expired entry
    soft_purge(&cache, SoftPurgeOptions::new("expired-test")).await.unwrap();

    // Verify the entry still exists and is expired
    let entry = cache.get("expired-test").await;
    assert!(entry.is_some());
    let entry = entry.unwrap();
    assert_eq!(entry.value, "expired-value");
    
    // The entry should still be expired after soft purge
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    assert!(entry.is_expired(current_time));
    
    // The created_time should have been updated to approximately now for proper SWR behavior
    assert!(entry.metadata.created_time >= now - Duration::from_secs(1));
}

#[tokio::test] 
async fn test_soft_purge_options_builder() {
    // Test that SoftPurgeOptions builder pattern works correctly
    let options = SoftPurgeOptions::new("test-key")
        .stale_while_revalidate(Duration::from_secs(120));
        
    assert_eq!(options.key, "test-key");
    assert_eq!(options.stale_while_revalidate, Some(Duration::from_secs(120)));
    
    // Test default values
    let default_options = SoftPurgeOptions::new("another-key");
    assert_eq!(default_options.key, "another-key");
    assert_eq!(default_options.stale_while_revalidate, None);
}
