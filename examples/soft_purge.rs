use cachified::{cachified, soft_purge, CachifiedOptionsBuilder, MokaCache, SoftPurgeOptions, Cache};
use std::time::Duration;
use tokio::time::{sleep, Instant};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache = MokaCache::new(1000);

    println!("=== Soft Purge Demo ===");
    
    // Example 1: Basic soft purge usage
    println!("\n1. Initial cache population:");
    let start = Instant::now();
    
    let initial_value: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "user-data")
            .ttl(Duration::from_secs(300)) // 5 minutes
            .get_fresh_value(|| async {
                println!("   Fetching fresh data (slow operation)...");
                sleep(Duration::from_millis(500)).await; // Simulate slow API
                Ok(format!("Data fetched at {:?}", Instant::now()))
            })
    ).await?;
    
    println!("   Result: {}", initial_value);
    println!("   Time taken: {:?}", start.elapsed());
    
    // Example 2: Verify cache is working
    println!("\n2. Subsequent call (should use cache):");
    let cache_start = Instant::now();
    
    let cached_value: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "user-data")
            .ttl(Duration::from_secs(300))
            .get_fresh_value(|| async {
                println!("   This shouldn't run - using cache!");
                Ok("Fresh Value".to_string())
            })
    ).await?;
    
    println!("   Result: {}", cached_value);
    println!("   Time taken: {:?} (should be very fast)", cache_start.elapsed());
    
    // Example 3: Soft purge the data
    println!("\n3. Soft purging the cache entry:");
    soft_purge(&cache, SoftPurgeOptions::new("user-data")).await?;
    println!("   Cache entry has been soft purged (marked as expired)");
    
    // Example 4: Using stale-while-revalidate after soft purge
    println!("\n4. Calling cachified with stale-while-revalidate after soft purge:");
    let swr_start = Instant::now();
    
    let swr_value: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "user-data")
            .ttl(Duration::from_secs(300))
            .stale_while_revalidate(Duration::from_secs(60)) // Allow stale for 1 minute
            .get_fresh_value(|| async {
                println!("   Background refresh started...");
                sleep(Duration::from_millis(500)).await; // Simulate slow API
                let fresh_data = format!("Fresh data after soft purge at {:?}", Instant::now());
                println!("   Background refresh completed: {}", fresh_data);
                Ok(fresh_data)
            })
    ).await?;
    
    println!("   SWR Result (stale): {}", swr_value);
    println!("   Time taken: {:?} (should be fast - returns stale immediately)", swr_start.elapsed());
    
    // Wait for background refresh to complete
    sleep(Duration::from_millis(600)).await;
    
    // Example 5: Subsequent call should return fresh data
    println!("\n5. After background refresh:");
    let post_refresh_start = Instant::now();
    
    let fresh_value: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "user-data")
            .ttl(Duration::from_secs(300))
            .get_fresh_value(|| async {
                println!("   This shouldn't run - data should be fresh in cache");
                Ok("Unexpected fresh fetch".to_string())
            })
    ).await?;
    
    println!("   Result: {}", fresh_value);
    println!("   Time taken: {:?} (should be fast - using refreshed cache)", post_refresh_start.elapsed());
    
    // Example 6: Soft purge with custom stale duration
    println!("\n6. Soft purge with custom stale-while-revalidate duration:");
    soft_purge(&cache, 
        SoftPurgeOptions::new("user-data")
            .stale_while_revalidate(Duration::from_secs(30))
    ).await?;
    println!("   Cache entry soft purged with 30-second stale window");
    
    // Example 7: Comparison with hard purge (remove)
    println!("\n7. Comparison - Hard purge (complete removal):");
    cache.remove("user-data").await;
    println!("   Cache entry completely removed");
    
    let hard_purge_start = Instant::now();
    let after_hard_purge: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "user-data")
            .ttl(Duration::from_secs(300))
            .get_fresh_value(|| async {
                println!("   Fetching fresh data after hard purge (user waits)...");
                sleep(Duration::from_millis(500)).await;
                Ok(format!("Data after hard purge at {:?}", Instant::now()))
            })
    ).await?;
    
    println!("   Result: {}", after_hard_purge);
    println!("   Time taken: {:?} (slow - user had to wait)", hard_purge_start.elapsed());
    
    Ok(())
}
