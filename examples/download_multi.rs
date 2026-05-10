//! Download daily OHLCV for several symbols concurrently.
//!
//! Run: `cargo run --example download_multi -- AAPL MSFT GOOG NVDA`

use yfinance::{download, Period, YfClient};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        args = vec!["AAPL".into(), "MSFT".into(), "GOOG".into()];
    }

    let client = YfClient::new()?;
    let multi = yfinance::DownloadBuilder::new(&client, args)
        .period(Period::M3)
        .concurrency(4)
        .fetch()
        .await?;

    for (sym, h) in &multi.series {
        let last = h.rows.last();
        println!(
            "{}: {} bars; last close = {:?}",
            sym,
            h.rows.len(),
            last.map(|r| r.close)
        );
    }
    for (sym, e) in &multi.errors {
        eprintln!("{}: ERROR — {}", sym, e);
    }

    // Same one-shot using the convenience helper.
    let _ = download(&client, vec!["TSLA"]).await?;
    Ok(())
}
