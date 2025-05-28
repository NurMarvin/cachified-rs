use cachified::{cachified, CachifiedOptionsBuilder, MokaCache, validation::NonEmptyStringValidator};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache = MokaCache::new(1000);

    println!("=== Basic Caching Example ===");
    
    // Example 1: Basic caching with TTL
    let value: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "user-123")
            .ttl(Duration::from_secs(300)) // 5 minutes
            .get_fresh_value(|| async {
                println!("Fetching fresh value for user-123...");
                // Simulate an expensive operation (database call, API request, etc.)
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok("John Doe".to_string())
            })
    ).await?;
    
    println!("First call result: {}", value);
    
    // Second call should return cached value (no "Fetching..." message)
    let cached_value: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "user-123")
            .ttl(Duration::from_secs(300))
            .get_fresh_value(|| async {
                println!("This shouldn't print - using cache!");
                Ok("Fresh Value".to_string())
            })
    ).await?;
    
    println!("Second call result (from cache): {}", cached_value);
    
    println!("\n=== Stale-While-Revalidate Example ===");
    
    // Example 2: Stale-while-revalidate
    let cache2 = MokaCache::new(100);
    
    // First, populate the cache
    let _: String = cachified(
        CachifiedOptionsBuilder::new(cache2.clone(), "api-data")
            .ttl(Duration::from_millis(100)) // Very short TTL for demo
            .get_fresh_value(|| async {
                Ok("Initial data".to_string())
            })
    ).await?;
    
    // Wait for data to expire
    tokio::time::sleep(Duration::from_millis(150)).await;
    
    // This should serve stale data immediately while refreshing in background
    let stale_value: String = cachified(
        CachifiedOptionsBuilder::new(cache2.clone(), "api-data")
            .ttl(Duration::from_millis(100))
            .stale_while_revalidate(Duration::from_secs(60)) // Allow stale for 1 minute
            .get_fresh_value(|| async {
                println!("Background refresh happening...");
                tokio::time::sleep(Duration::from_millis(50)).await;
                Ok("Refreshed data".to_string())
            })
    ).await?;
    
    println!("SWR result (should be stale): {}", stale_value);
    
    // Give background refresh time to complete
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Next call should return the fresh data from background refresh
    let fresh_value: String = cachified(
        CachifiedOptionsBuilder::new(cache2.clone(), "api-data")
            .ttl(Duration::from_millis(100))
            .stale_while_revalidate(Duration::from_secs(60))
            .get_fresh_value(|| async {
                println!("This shouldn't run - data should be fresh in cache");
                Ok("Unexpected fresh fetch".to_string())
            })
    ).await?;

    println!("Post-refresh result: {}", fresh_value);
    
    println!("\n=== Validation Example ===");
    
    // Example 3: Value validation
    let cache3 = MokaCache::new(100);
    
    // This will work - valid string
    let valid_result: String = cachified(
        CachifiedOptionsBuilder::new(cache3.clone(), "valid-data")
            .ttl(Duration::from_secs(300))
            .check_value(NonEmptyStringValidator)
            .get_fresh_value(|| async {
                Ok("Valid content".to_string())
            })
    ).await?;
    
    println!("Valid result: {}", valid_result);
    
    // Example 4: Force fresh (bypass cache)
    println!("\n=== Force Fresh Example ===");
    
    let fresh_value: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "user-123") // Same key as before
            .ttl(Duration::from_secs(300))
            .force_fresh(true) // This will bypass the cache
            .get_fresh_value(|| async {
                println!("Force fetching fresh value...");
                Ok("Fresh forced value".to_string())
            })
    ).await?;
    
    println!("Force fresh result: {}", fresh_value);
    
    println!("\n=== Fallback to Cache Example ===");
    
    // Example 5: Fallback to cache when fresh value fails
    let fallback_value: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "user-123")
            .ttl(Duration::from_secs(300))
            .fallback_to_cache(true)            .get_fresh_value(|| async {
                // Simulate a failure
                Err(cachified::CachifiedError::fresh_value("Simulated failure"))
            })
    ).await?;
    
    println!("Fallback result (from cache): {}", fallback_value);
    
    Ok(())
}
