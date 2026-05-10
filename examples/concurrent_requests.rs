//! Fetch quote, history, and profile concurrently for one symbol.
//!
//! ```bash
//! cargo run --example concurrent_requests -- AAPL
//! ```

use yfinance::{Interval, Period, Result, Ticker, YfClient};

#[tokio::main]
async fn main() -> Result<()> {
    let symbol = std::env::args().nth(1).unwrap_or_else(|| "AAPL".into());
    let client = YfClient::new()?;
    let ticker = Ticker::new(&client, &symbol);

    let (quote, history, profile) = tokio::join!(
        ticker.quote(),
        ticker
            .history()
            .period(Period::M1)
            .interval(Interval::D1)
            .fetch(),
        ticker.profile(),
    );

    let quote = quote?;
    let history = history?;
    let profile = profile?;

    println!("{} ({})", profile.name(), quote.symbol);
    if let Some(p) = quote.price {
        println!("  price={p:.2}");
    }
    println!("  bars={}", history.rows.len());
    Ok(())
}
