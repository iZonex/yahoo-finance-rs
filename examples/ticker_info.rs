//! Print company info for a ticker.
//!
//! Run: `cargo run --example ticker_info -- MSFT`

use yfinance::{Ticker, YfClient};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let symbol = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "MSFT".to_string());

    let client = YfClient::new()?;
    let ticker = Ticker::new(&client, &symbol);

    let fast = ticker.fast_info().await?;
    println!(
        "{} — last={:?} ccy={:?} mcap={:?}",
        fast.symbol, fast.last_price, fast.currency, fast.market_cap
    );

    let info = ticker.info().await?;
    println!("Long name : {}", info.long_name.as_deref().unwrap_or("-"));
    println!("Sector    : {}", info.sector.as_deref().unwrap_or("-"));
    println!("Industry  : {}", info.industry.as_deref().unwrap_or("-"));
    println!("Country   : {}", info.country.as_deref().unwrap_or("-"));
    println!("Website   : {}", info.website.as_deref().unwrap_or("-"));
    println!("PE (TTM)  : {:?}", info.trailing_pe);
    println!("Forward PE: {:?}", info.forward_pe);
    println!("EPS (TTM) : {:?}", info.trailing_eps);
    println!("Beta      : {:?}", info.beta);
    if let Some(s) = info.summary.as_deref() {
        println!("\nBusiness summary:\n{}", s);
    }
    Ok(())
}
