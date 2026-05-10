//! Bring an existing browser session into the crate when fresh `curl`-style
//! requests get 429'd by Yahoo's anti-scraper layer.
//!
//! ```bash
//! YF_COOKIE='A1=d=...; A3=d=...; gpp=DBAA' YF_CRUMB='ABC123xyz' \
//!     cargo run --example browser_session -- AAPL
//! ```
//!
//! ## How to extract from your browser
//!
//! 1. Open <https://finance.yahoo.com/quote/AAPL> while signed in to Yahoo.
//! 2. Open DevTools → Network → reload → click any request to
//!    `query1.finance.yahoo.com/...`.
//! 3. From the request headers copy the full `Cookie:` value into `YF_COOKIE`.
//! 4. From the same request's URL copy the `crumb=` query parameter value
//!    into `YF_CRUMB` (or fetch it from
//!    `https://query1.finance.yahoo.com/v1/test/getcrumb` while logged in).

use std::env;
use yfinance::{Period, Result, Ticker, YfClient};

#[tokio::main]
async fn main() -> Result<()> {
    let symbol = env::args().nth(1).unwrap_or_else(|| "AAPL".into());
    let cookie = env::var("YF_COOKIE").unwrap_or_default();
    let crumb = env::var("YF_CRUMB").unwrap_or_default();

    let mut builder = YfClient::builder();
    if !cookie.is_empty() {
        builder = builder.cookie_prime_url("").session_cookie(cookie);
    }
    if !crumb.is_empty() {
        builder = builder.session_crumb(crumb);
    }
    let client = builder.build()?;

    let ticker = Ticker::new(&client, &symbol);
    let h = ticker.history().period(Period::M1).fetch().await?;
    println!("{symbol}: {} bars", h.rows.len());
    if let Some(last) = h.rows.last() {
        println!(
            "  last bar: ts={} O={:.2} H={:.2} L={:.2} C={:.2} V={}",
            last.timestamp, last.open, last.high, last.low, last.close, last.volume,
        );
    }

    // crumb-protected: only works when YF_CRUMB is set.
    if !env::var("YF_CRUMB").unwrap_or_default().is_empty() {
        match ticker.options().await {
            Ok(o) => println!("  options expirations: {}", o.expirations.len()),
            Err(e) => eprintln!("  options failed (likely stale crumb): {e}"),
        }
    }
    Ok(())
}
