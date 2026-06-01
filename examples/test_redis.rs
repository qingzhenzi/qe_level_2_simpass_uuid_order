use std::time::Duration;

#[tokio::main]
async fn main() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://:redis_a4XQ4x@156.233.234.125:6379".into());

    println!("Testing Redis connection to: {}", redis_url);
    println!("(set REDIS_URL env var to override)");
    println!();

    let client = match redis::Client::open(redis_url.as_str()) {
        Ok(c) => {
            println!("[OK] Client::open succeeded");
            c
        }
        Err(e) => {
            println!("[FAIL] Client::open failed: {}", e);
            return;
        }
    };

    // First try: get_multiplexed_async_connection
    println!("--- Attempt 1: get_multiplexed_async_connection (8s timeout) ---");
    let result = tokio::time::timeout(
        Duration::from_secs(8),
        client.get_multiplexed_async_connection(),
    ).await;

    match result {
        Ok(Ok(mut conn)) => {
            println!("[OK] get_multiplexed_async_connection succeeded");

            let pong: String = redis::cmd("PING")
                .query_async(&mut conn)
                .await
                .unwrap_or_else(|e| format!("ERROR: {}", e));
            println!("[OK] PING -> {:?}", pong);

            println!();
            println!("========================================");
            println!("Redis connection test: PASSED");
            println!("========================================");
            return;
        }
        Ok(Err(e)) => {
            println!("[FAIL] Connection failed: {}", e);
            println!("  Kind: {:?}", e.kind());
            let err_str = e.to_string();
            if err_str.contains("REFUSED") || err_str.contains("refused") || err_str.contains("Connection refused") {
                println!("  => Redis server not running on 156.233.234.125:6379");
            } else if err_str.contains("AUTH") || err_str.contains("NOAUTH") || err_str.contains("authentication") {
                println!("  => Wrong Redis password");
            } else if err_str.contains("timed out") || err_str.contains("TimedOut") {
                println!("  => Connection timed out (firewall/network?)");
            }
        }
        Err(_) => {
            println!("[FAIL] Timed out after 8 seconds");
        }
    }

    // Second try: ConnectionManager
    println!();
    println!("--- Attempt 2: ConnectionManager (10s timeout) ---");
    let client2 = redis::Client::open(redis_url.as_str()).unwrap();
    let result2 = tokio::time::timeout(
        Duration::from_secs(10),
        redis::aio::ConnectionManager::new(client2),
    ).await;

    match result2 {
        Ok(Ok(_)) => println!("[OK] ConnectionManager succeeded"),
        Ok(Err(e)) => println!("[FAIL] ConnectionManager failed: {}", e),
        Err(_) => println!("[FAIL] ConnectionManager timed out after 10s"),
    }

    println!();
    println!("========================================");
    println!("All connection attempts completed");
    println!("Redis is NOT reachable at: {}", redis_url);
    println!("Please check:");
    println!("  1. Is Redis server running on 156.233.234.125:6379?");
    println!("  2. Is the password 'redis_a4XQ4x' correct?");
    println!("  3. Is there a firewall blocking port 6379?");
    println!("========================================");
}
