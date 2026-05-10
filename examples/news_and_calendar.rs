//! Latest news + the upcoming corporate-event calendar for one symbol.
//!
//! ```bash
//! cargo run --example news_and_calendar -- AAPL
//! ```

use yfinance::{NewsTab, Result, Ticker, YfClient};

#[tokio::main]
async fn main() -> Result<()> {
    let symbol = std::env::args().nth(1).unwrap_or_else(|| "AAPL".into());
    let client = YfClient::new()?;
    let ticker = Ticker::new(&client, &symbol);

    let articles = ticker
        .news()
        .tab(NewsTab::LatestNews)
        .count(5)
        .fetch()
        .await?;
    println!("--- {} latest news ({}) ---", symbol, articles.len());
    for a in &articles {
        let pub_at = a.published_at.unwrap_or_default();
        println!(
            "  [{pub_at}] {} ({})",
            a.title,
            a.publisher.as_deref().unwrap_or("?")
        );
    }

    println!();
    let cal = ticker.calendar().await?;
    println!("--- {symbol} calendar ---");
    println!("  upcoming earnings: {} dates", cal.earnings_dates.len());
    for d in &cal.earnings_dates {
        println!("    {d}");
    }
    if let Some(d) = cal.ex_dividend_date {
        println!("  ex-dividend: {d}");
    }
    if let Some(d) = cal.dividend_date {
        println!("  dividend pay: {d}");
    }
    Ok(())
}
