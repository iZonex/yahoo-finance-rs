//! Print the nearest-expiration options chain for a ticker.
//!
//! Run: `cargo run --example options_chain -- AAPL`

use chrono::TimeZone;
use yfinance::{Ticker, YfClient};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let symbol = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "AAPL".to_string());

    let client = YfClient::new()?;
    let t = Ticker::new(&client, &symbol);

    let exp = t.options().await?;
    println!("{}: {} expirations", exp.symbol, exp.expirations.len());

    let chain = t.option_chain(None).await?;
    let exp_str = chain
        .expiration
        .and_then(|s| chrono::Utc.timestamp_opt(s, 0).single())
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "?".into());

    println!(
        "\n=== Calls ({}) — {} contracts ===",
        exp_str,
        chain.calls.len()
    );
    for c in chain.calls.iter().take(10) {
        println!(
            "{:<22} strike={:?} last={:?} bid={:?} ask={:?} OI={:?}",
            c.contract_symbol, c.strike, c.last_price, c.bid, c.ask, c.open_interest
        );
    }

    println!(
        "\n=== Puts ({}) — {} contracts ===",
        exp_str,
        chain.puts.len()
    );
    for p in chain.puts.iter().take(10) {
        println!(
            "{:<22} strike={:?} last={:?} bid={:?} ask={:?} OI={:?}",
            p.contract_symbol, p.strike, p.last_price, p.bid, p.ask, p.open_interest
        );
    }
    Ok(())
}
