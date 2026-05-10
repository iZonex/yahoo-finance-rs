//! Convert yfinance results to Polars `DataFrame`s.
//!
//! Run with the `dataframe` feature enabled:
//! ```bash
//! cargo run --example polars_dataframes --features dataframe -- AAPL
//! ```

use yfinance::{dataframe::ToDataFrame, Interval, Period, Result, Ticker, YfClient};

#[tokio::main]
async fn main() -> Result<()> {
    let symbol = std::env::args().nth(1).unwrap_or_else(|| "AAPL".into());
    let client = YfClient::new()?;
    let ticker = Ticker::new(&client, &symbol);

    let history = ticker
        .history()
        .period(Period::M3)
        .interval(Interval::D1)
        .auto_adjust(true)
        .fetch()
        .await?;

    let df = history.to_dataframe().expect("history to_dataframe");
    println!(
        "=== {symbol} history ({}, {}) ===",
        df.shape().0,
        df.shape().1
    );
    println!("{df}");

    if let Ok(rows) = ticker.recommendations().await {
        let df = rows.to_dataframe().expect("recommendations to_dataframe");
        println!("\n=== {symbol} recommendations ===");
        println!("{df}");
    }

    if let Ok(arts) = ticker.news().count(5).fetch().await {
        let df = arts.to_dataframe().expect("news to_dataframe");
        println!("\n=== {symbol} news ===");
        println!("{df}");
    }

    Ok(())
}
