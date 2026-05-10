# yahoo-finance-rs

[![CI](https://github.com/iZonex/yahoo-finance-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/iZonex/yahoo-finance-rs/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/yahoo-finance-rs.svg)](https://crates.io/crates/yahoo-finance-rs)
[![Docs.rs](https://docs.rs/yahoo-finance-rs/badge.svg)](https://docs.rs/yahoo-finance-rs)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Async Rust client for Yahoo Finance — a port of the Python
[`yfinance`](https://github.com/ranaroussi/yfinance) library.

> The crate is published as **`yahoo-finance-rs`** on crates.io. The library
> name is **`yfinance`**, so user code reads `use yfinance::Ticker;`.

> **Disclaimer.** This crate is not affiliated, endorsed, or vetted by Yahoo, Inc.
> It uses Yahoo's publicly available APIs and is intended for research and
> educational purposes. Respect Yahoo's
> [terms of service](https://policies.yahoo.com/us/en/yahoo/terms/index.htm)
> and rate limits.

## Quick start

```toml
[dependencies]
yahoo-finance-rs = "0.1"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

```rust
use yfinance::{Period, Ticker, YfClient};

#[tokio::main]
async fn main() -> yfinance::Result<()> {
    let client = YfClient::new()?;
    let aapl = Ticker::new(&client, "AAPL");

    let history = aapl.history().period(Period::M3).fetch().await?;
    println!("AAPL: {} bars", history.rows.len());

    let quote = aapl.quote().await?;
    println!("price = {:?}, bid/ask = {:?}/{:?}", quote.price, quote.bid, quote.ask);

    Ok(())
}
```

## What's covered

### Per-`Ticker` data
- **`history()`** — OHLCV with `auto_adjust`, `back_adjust`, `keepna`, `rounding`, `repair`, prepost, period or start/end.
- **`quote()` / `fast_info()`** — full snapshot (bid/ask, market state, pre/post) vs compact subset.
- **`info()`** — flattened `quoteSummary` modules (sector, industry, summary, ratios).
- **`income_stmt`, `balance_sheet`, `cashflow`** + `quarterly_*` variants (~200 metrics).
- **`earnings()`** — yearly + quarterly revenue/earnings + EPS estimate-vs-actual.
- **`calendar()`** — upcoming earnings, ex-dividend, dividend payment dates.
- **`shares()` / `quarterly_shares()`** — historical shares outstanding.
- **`recommendations()`, `recommendations_summary()`, `upgrades_downgrades()`, `price_target()`, `earnings_trend()`**.
- **`sustainability()`** — ESG scores + involvement flags.
- **`holders()`** — major / institutional / mutual-fund / insider transactions / insider roster / net-share-purchase.
- **`options()`, `option_chain(expiration)`** — expirations, calls, puts.
- **`news()`** — `LatestNews` / `All` / `PressReleases`.
- **`profile()`** — company / fund profile, with HTML scrape fallback.
- **`isin()`** — ISIN lookup via Business Insider.
- **`stream()`** — real-time quotes via WebSocket / HTTP polling (feature `stream`).

### Top-level
- **`download(...)`** — bounded-concurrency multi-symbol OHLCV.
- **`batch_quotes(...)`** — many symbols in one HTTP call.
- **`Search`**, **`Lookup`**, **`Market`**, **`Sector`**, **`Industry`**.

### Client knobs
- Timeout, retries with exponential backoff, rotating User-Agent.
- Cookie + crumb session; configurable bases (data host, crumb URL, cookie-prime, ISIN, news, quote-page).
- **In-memory TTL cache** (`CacheMode::Use` / `Refresh` / `Bypass`, opt-in via `client.cache_ttl(...)`).
- **Per-request overrides** on `HistoryBuilder` / `DownloadBuilder` / `NewsBuilder` (`cache_mode`, `retry`).
- **`session_cookie` / `session_crumb`** injection for environments with a pre-existing browser session.

## Optional features

| Feature | Pulls in | Enables |
| --- | --- | --- |
| `stream` | `prost`, `tokio-tungstenite`, `futures-util`, `base64` | `Ticker::stream()`, `StreamBuilder` |
| `dataframe` | `polars` | `ToDataFrame` impls for `History`, `MultiHistory`, recommendations, news, holders … |
| `tracing` | `tracing`, `tracing-log` | Forward internal `log` events into `tracing` |
| `tracing-subscriber` | also `tracing-subscriber` | `init_tracing_for_tests()` dev convenience |
| `test-mode` | – | Fixture recording when `YF_RECORD=1` |

```toml
yahoo-finance-rs = { version = "0.1", features = ["stream", "dataframe", "tracing"] }
```

## Examples

```bash
cargo run --example ticker_history -- AAPL
cargo run --example ticker_info -- MSFT
cargo run --example concurrent_requests -- AAPL
cargo run --example download_multi -- AAPL MSFT GOOG NVDA
cargo run --example search -- apple
cargo run --example options_chain -- AAPL
cargo run --example holders_and_insiders -- AAPL
cargo run --example news_and_calendar -- AAPL
cargo run --example builder_configuration
cargo run --example browser_session -- AAPL              # session-cookie injection
cargo run --features dataframe --example polars_dataframes -- AAPL
```

## Snippets

### Repair

```rust
let mut hist = aapl
    .history()
    .period(Period::Y2)
    .repair(true)               // 100× currency mixup, zero-close fill, missed splits
    .fetch()
    .await?;
let report = hist.repair();     // or after the fact
```

### Multi-ticker download

```rust
use yfinance::{download, Period};

let multi = download(&client, ["AAPL", "MSFT", "NVDA"])
    .period(Period::M1)
    .concurrency(8)
    .run()
    .await?;
for (sym, h) in &multi.series {
    println!("{}: {} bars", sym, h.rows.len());
}
```

### News + calendar

```rust
use yfinance::NewsTab;

let articles = aapl.news().tab(NewsTab::LatestNews).count(5).fetch().await?;
let cal = aapl.calendar().await?;
println!("next earnings: {:?}", cal.earnings_dates.first());
```

### Polars (feature `dataframe`)

```rust
use yfinance::dataframe::ToDataFrame;
let df = aapl.history().period(Period::M3).fetch().await?.to_dataframe()?;
println!("{df}");
```

### WebSocket streaming (feature `stream`)

```rust
use yfinance::stream::StreamMethod;

let (mut rx, handle) = aapl.stream()
    .add_symbol("MSFT")
    .method(StreamMethod::WebSocket)
    .start()?;

while let Some(update) = rx.recv().await {
    println!("{}: {} ({:?} change)", update.symbol, update.price, update.change_percent);
}
handle.stop().await;
```

### Caching

```rust
use std::time::Duration;
use yfinance::{CacheMode, RetryConfig};

let client = YfClient::builder()
    .cache_ttl(Duration::from_secs(60))
    .retry(RetryConfig::new(5, Duration::from_millis(250)))
    .build()?;

// Force a refresh on a single call:
let bars = Ticker::new(&client, "AAPL")
    .history()
    .cache_mode(CacheMode::Refresh)
    .fetch()
    .await?;
```

### Bringing your own browser session

When fresh `curl`-style HTTP clients get throttled by Yahoo's anti-scraper
layer, paste a working browser session from DevTools:

```rust
let client = YfClient::builder()
    .cookie_prime_url("")                                 // skip /consent
    .session_cookie("A1=d=...; A3=d=...; gpp=DBAA")       // from DevTools → Network
    .session_crumb("ABC123xyz")                           // from /v1/test/getcrumb
    .build()?;
```

## Testing

The crate ships a fixture-based test harness with three modes:

```bash
just test-offline            # default — replays recorded fixtures (no network)
just test-record             # YF_RECORD=1 — hits live Yahoo, writes fixtures
just test-live               # YF_LIVE=1 — exercises live API without writing
just test-full               # record then replay
```

Fixtures live under `tests/fixtures/{endpoint}_{symbol}.{ext}`. Recording is
gated by the `test-mode` cargo feature plus `YF_RECORD=1`. The 0.1.0 release
ships with parsers verified against real responses for: history, quote, info,
holders, fundamentals, ISIN, ESG, news, profile, all five analysis methods.

## Architecture

`YfClient` is cheap to clone (`Arc` internally). It serializes crumb refreshes
through a single `tokio::sync::Mutex` so concurrent callers share one
handshake. The cookie store is provided by `reqwest`'s built-in jar.

| File | Endpoint family |
| ---- | --------------- |
| `src/client.rs` | `YfClient`, cookie+crumb session, cache, retries |
| `src/history.rs` | `v8/finance/chart` |
| `src/quote.rs` | `v7/finance/quote` (`FastInfo`, `Quote`, `batch_quotes`) |
| `src/info.rs` | `v10/quoteSummary` (composite Info) |
| `src/fundamentals.rs` | `fundamentals-timeseries` |
| `src/earnings.rs` | `quoteSummary?modules=earnings` |
| `src/calendar.rs` | `quoteSummary?modules=calendarEvents` |
| `src/shares.rs` | `fundamentals-timeseries` (basicAverageShares) |
| `src/analysis.rs` | `quoteSummary` recommendation/financialData/upgradeDowngrade/earningsTrend |
| `src/esg.rs` | `quoteSummary?modules=esgScores` |
| `src/holders.rs` | `quoteSummary` holders modules |
| `src/options.rs` | `v7/finance/options` |
| `src/news.rs` | `xhr/ncp` (POST) |
| `src/profile.rs` | `quoteSummary` profile + HTML scrape fallback |
| `src/isin.rs` | `markets.businessinsider.com/ajax/SearchController_Suggest` |
| `src/search.rs` / `src/lookup.rs` | `v1/finance/search` / `v1/finance/lookup` |
| `src/domain.rs` | `Market`, `Sector`, `Industry` taxonomy |
| `src/download.rs` | Concurrent multi-symbol batcher |
| `src/stream.rs` | WebSocket + polling (feature `stream`) |
| `src/dataframe.rs` | Polars `ToDataFrame` impls (feature `dataframe`) |
| `src/repair.rs` | Local repair passes |
| `src/wire.rs` | Internal `RawNum<T>` parser |
| `src/test_fixtures.rs` | Fixture loading / recording |

## Minimum supported Rust version

MSRV is **1.88** and is enforced in CI. The bump from 1.75 is forced by
transitive dependencies (`time-core` requires the `edition2024` feature
stabilised in 1.85, and `time-macros` 0.2.27 requires 1.88).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
Security issues go through [SECURITY.md](SECURITY.md), not public issues.

## License

Licensed under the [Apache License, Version 2.0](LICENSE). See [NOTICE](NOTICE).
