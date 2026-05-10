//! Search and lookup.
//!
//! Run: `cargo run --example search -- apple`

use yfinance::{LookupBuilder, LookupType, SearchBuilder, YfClient};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let q = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "apple".to_string());

    let client = YfClient::new()?;

    let s = SearchBuilder::new(&client, &q)
        .max_results(5)
        .news_count(3)
        .fetch()
        .await?;
    println!("== Search quotes ==");
    for q in &s.quotes {
        println!(
            "{:>8}  {:<24} {:<12} {}",
            q.symbol,
            q.short_name.as_deref().unwrap_or("-"),
            q.exchange.as_deref().unwrap_or("-"),
            q.quote_type.as_deref().unwrap_or("-")
        );
    }

    println!("\n== Search news ==");
    for n in &s.news {
        if let Some(t) = n.title.as_deref() {
            println!("- {}", t);
        }
    }

    println!("\n== Lookup (equities only) ==");
    let l = LookupBuilder::new(&client, &q)
        .kind(LookupType::Equity)
        .count(5)
        .fetch()
        .await?;
    for r in &l.rows {
        println!(
            "{:>8}  {:<24} {:<10}",
            r.symbol,
            r.short_name.as_deref().unwrap_or("-"),
            r.exchange.as_deref().unwrap_or("-")
        );
    }
    Ok(())
}
