//! Demonstrates the various `YfClient` builder knobs.
//!
//! ```bash
//! cargo run --example builder_configuration
//! ```

use std::time::Duration;
use yfinance::{ApiPreference, CacheMode, Period, Result, RetryConfig, Ticker, YfClient};

#[tokio::main]
async fn main() -> Result<()> {
    let client = YfClient::builder()
        .timeout(Duration::from_secs(15))
        .retry(RetryConfig::new(5, Duration::from_millis(250)))
        .cache_ttl(Duration::from_secs(30))
        .api_preference(ApiPreference::Auto)
        .build()?;

    let ticker = Ticker::new(&client, "AAPL");
    // First call hits the network.
    let h1 = ticker.history().period(Period::D5).fetch().await?;
    // Second call (within 30 s) is served from cache.
    let h2 = ticker
        .history()
        .period(Period::D5)
        .cache_mode(CacheMode::Use)
        .fetch()
        .await?;
    println!(
        "rows: first={} second={} (same fetch)",
        h1.rows.len(),
        h2.rows.len()
    );

    // Force a refresh — bypass the cached value.
    let h3 = ticker
        .history()
        .period(Period::D5)
        .cache_mode(CacheMode::Refresh)
        .fetch()
        .await?;
    println!("rows after refresh: {}", h3.rows.len());
    Ok(())
}
