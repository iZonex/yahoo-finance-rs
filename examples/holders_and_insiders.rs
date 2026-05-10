//! Print every holders breakdown for a symbol.
//!
//! ```bash
//! cargo run --example holders_and_insiders -- AAPL
//! ```

use yfinance::{Result, Ticker, YfClient};

#[tokio::main]
async fn main() -> Result<()> {
    let symbol = std::env::args().nth(1).unwrap_or_else(|| "AAPL".into());
    let client = YfClient::new()?;
    let h = Ticker::new(&client, &symbol).holders().await?;

    println!("--- Major holders breakdown ---");
    for r in &h.major {
        println!("  {:.<40} {:>6.2}%", r.label, r.percent * 100.0);
    }

    println!(
        "\n--- Top institutional holders ({}) ---",
        h.institutional.len()
    );
    for r in h.institutional.iter().take(5) {
        println!(
            "  {:<35} {:>10} shares  {:>5.2}%",
            r.holder,
            r.shares.unwrap_or_default(),
            r.percent_held.unwrap_or_default() * 100.0,
        );
    }

    println!(
        "\n--- Recent insider transactions ({}) ---",
        h.insider_transactions.len()
    );
    for t in h.insider_transactions.iter().take(5) {
        println!(
            "  {:<25} {:?}  {:>10} sh",
            t.name,
            t.transaction.as_deref().unwrap_or("-"),
            t.shares.unwrap_or_default(),
        );
    }
    Ok(())
}
