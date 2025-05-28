use cachified::{cachified, CachifiedOptionsBuilder, MokaCache, Cache, validation::{NonNullValidator, NonEmptyStringValidator, validator}};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache = MokaCache::new(1000);

    println!("=== Validation Examples ===");

    // Example 1: NonNullValidator for Option types
    println!("\n1. NonNull Validation:");
    
    let valid_option: Option<String> = cachified(
        CachifiedOptionsBuilder::new(cache.clone(), "maybe-data")
            .ttl(Duration::from_secs(60))
            .check_value(NonNullValidator)
            .get_fresh_value(|| async {
                Ok(Some("Valid data".to_string()))
            })
    ).await?;
    
    println!("Valid option result: {:?}", valid_option);
    
    // Example 2: NonEmptyStringValidator - note we need to work with String not Option<String>
    println!("\n2. NonEmpty String Validation:");
    
    let string_cache = MokaCache::new(100);
    let valid_string: String = cachified(
        CachifiedOptionsBuilder::new(string_cache.clone(), "text-data")
            .ttl(Duration::from_secs(60))
            .check_value(NonEmptyStringValidator)
            .get_fresh_value(|| async {
                Ok("Hello, World!".to_string())
            })
    ).await?;
    
    println!("Valid string result: {}", valid_string);
    
    // Example 3: Custom validator using closure
    println!("\n3. Custom Validation (numbers > 10):");
    
    let number_cache = MokaCache::new(100);
    let valid_number: i32 = cachified(
        CachifiedOptionsBuilder::new(number_cache.clone(), "number-data")
            .ttl(Duration::from_secs(60))
            .check_value(validator(|value: &i32| {
                if *value > 10 {
                    Ok(())
                } else {
                    Err(cachified::CachifiedError::validation("Number must be > 10"))
                }
            }))
            .get_fresh_value(|| async {
                Ok(42)
            })
    ).await?;
    
    println!("Valid number result: {}", valid_number);
    
    // Example 4: Validation failure - should fetch fresh value
    println!("\n4. Validation Failure Recovery:");
    
    // First, let's put invalid data in cache manually (simulating corrupted cache)
    string_cache.set("corrupted-data", cachified::CacheEntry {
        value: "".to_string(), // Empty string - will fail validation
        metadata: cachified::CacheMetadata {
            created_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap(),
            ttl: Some(Duration::from_secs(300)),
        }
    }).await?;
    
    // Now try to get it with validation - should fetch fresh value
    let recovered_value: String = cachified(
        CachifiedOptionsBuilder::new(string_cache.clone(), "corrupted-data")
            .ttl(Duration::from_secs(60))
            .check_value(NonEmptyStringValidator)
            .get_fresh_value(|| async {
                println!("Fetching fresh value due to validation failure...");
                Ok("Fresh valid data".to_string())
            })
    ).await?;
    
    println!("Recovered value: {}", recovered_value);
    
    Ok(())
}
