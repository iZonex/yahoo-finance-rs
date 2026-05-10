//! Print 1 month of daily candles for a ticker.
//!
//! Run: `cargo run --example ticker_history -- AAPL`

use yfinance::{Interval, Period, Ticker, YfClient};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let symbol = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "AAPL".to_string());

    let client = YfClient::new()?;
    let ticker = Ticker::new(&client, &symbol);

    let history = ticker
        .history()
        .period(Period::M1)
        .interval(Interval::D1)
        .auto_adjust(true)
        .fetch()
        .await?;

    println!(
        "{} ({} on {}) — {} bars",
        history.symbol,
        history.currency.as_deref().unwrap_or("?"),
        history.exchange_name.as_deref().unwrap_or("?"),
        history.rows.len()
    );
    println!("date         open      high      low       close     volume");
    for row in &history.rows {
        println!(
            "{}  {:>8.2}  {:>8.2}  {:>8.2}  {:>8.2}  {:>12}",
            row.timestamp.format("%Y-%m-%d"),
            row.open,
            row.high,
            row.low,
            row.close,
            row.volume
        );
    }
    Ok(())
}
