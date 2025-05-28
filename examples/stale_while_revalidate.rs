use cachified::{cachified, CachifiedOptionsBuilder, MokaCache};
use std::time::Duration;
use tokio::time::{sleep, Instant};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache = MokaCache::new(1000);

    println!("=== Stale-While-Revalidate Demo ===");
    
    // First, populate the cache
    println!("1. Initial population:");
    let start = Instant::now();
    
    let initial_value: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "slow-api")
            .ttl(Duration::from_secs(2)) // Short TTL for demo
            .get_fresh_value(|| async {
                println!("   Fetching initial data (slow operation)...");
                sleep(Duration::from_millis(500)).await; // Simulate slow API
                Ok(format!("Data fetched at {:?}", Instant::now()))
            })
    ).await?;
    
    println!("   Result: {}", initial_value);
    println!("   Time taken: {:?}", start.elapsed());
    
    // Wait for cache to expire
    println!("\n2. Waiting for cache to expire...");
    sleep(Duration::from_secs(3)).await;
    
    // Now demonstrate SWR - should return stale data immediately
    println!("\n3. Using stale-while-revalidate:");
    let swr_start = Instant::now();
    
    let swr_value: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "slow-api")
            .ttl(Duration::from_secs(2))
            .stale_while_revalidate(Duration::from_secs(30)) // Allow stale for 30 seconds
            .get_fresh_value(|| async {
                println!("   Background refresh started...");
                sleep(Duration::from_millis(500)).await; // Simulate slow API
                let fresh_data = format!("Fresh data fetched at {:?}", Instant::now());
                println!("   Background refresh completed: {}", fresh_data);
                Ok(fresh_data)
            })
    ).await?;
    
    println!("   SWR Result (stale): {}", swr_value);
    println!("   Time taken: {:?} (should be fast!)", swr_start.elapsed());
    
    // Wait a moment for background refresh to complete
    sleep(Duration::from_millis(600)).await;
    
    // Next call should return the fresh data from background refresh
    println!("\n4. After background refresh:");
    let post_refresh_start = Instant::now();
    
    let fresh_value: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "slow-api")
            .ttl(Duration::from_secs(2))
            .stale_while_revalidate(Duration::from_secs(30))
            .get_fresh_value(|| async {
                println!("   This shouldn't run - data should be fresh in cache");
                Ok("Unexpected fresh fetch".to_string())
            })
    ).await?;
    
    println!("   Result: {}", fresh_value);
    println!("   Time taken: {:?} (should be fast!)", post_refresh_start.elapsed());
    
    // Demonstrate what happens without SWR
    println!("\n5. Without SWR (for comparison):");
    sleep(Duration::from_secs(3)).await; // Wait for expiration again
    
    let no_swr_start = Instant::now();
    let no_swr_value: String = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "slow-api-no-swr")
            .ttl(Duration::from_secs(2))
            // No stale_while_revalidate - will wait for fresh data
            .get_fresh_value(|| async {
                println!("   Fetching fresh data (user waits)...");
                sleep(Duration::from_millis(500)).await;
                Ok(format!("No-SWR data fetched at {:?}", Instant::now()))
            })
    ).await?;
    
    println!("   Result: {}", no_swr_value);
    println!("   Time taken: {:?} (slow - user had to wait)", no_swr_start.elapsed());
    
    Ok(())
}
