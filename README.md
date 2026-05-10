# yfinance-rust

[![CI](https://github.com/iZonex/yfinance-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/iZonex/yfinance-rust/actions/workflows/ci.yml)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Async Rust client for Yahoo Finance — a port of the Python
[`yfinance`](https://github.com/ranaroussi/yfinance) library.

> **Disclaimer.** This crate is not affiliated, endorsed, or vetted by Yahoo, Inc.
> It uses Yahoo's publicly available APIs and is intended for research and
> educational purposes. Respect Yahoo's
> [terms of service](https://policies.yahoo.com/us/en/yahoo/terms/index.htm)
> and rate limits.

## Features

- **Historical OHLCV** with auto-adjust, back-adjust, dividends and splits.
- **Quote / fast info** — single-call price, volume, 52w range, market cap.
- **Full info** via `quoteSummary` (sector, industry, business summary, ratios).
- **Fundamentals** — annual & quarterly income statement, balance sheet, cash flow.
- **Holders** — major / institutional / mutual-fund / insider transactions.
- **Options chain** — expirations, calls and puts with greeks-adjacent fields.
- **Search & lookup** — typeahead-style symbol resolution.
- **Multi-ticker `download`** with bounded concurrency.
- **Domain** — Market summary, Sector and Industry taxonomy.
- Cookie + crumb session, per-request retries with exponential backoff,
  rotating User-Agent.

## Quick start

Add to your `Cargo.toml`:

```toml
[dependencies]
yfinance = "0.1"
tokio = { version = "1", features = ["full"] }
```

### History (OHLCV)

```rust
use yfinance::{Interval, Period, Ticker, YfClient};

#[tokio::main]
async fn main() -> yfinance::Result<()> {
    let client = YfClient::new()?;
    let aapl = Ticker::new(&client, "AAPL");

    let hist = aapl
        .history()
        .period(Period::M3)
        .interval(Interval::D1)
        .auto_adjust(true)
        .fetch()
        .await?;

    for row in hist.rows.iter().take(5) {
        println!(
            "{}  O={:.2}  C={:.2}  V={}",
            row.timestamp.format("%Y-%m-%d"),
            row.open,
            row.close,
            row.volume
        );
    }
    Ok(())
}
```

### Quote / info

```rust
let fast = aapl.fast_info().await?;
println!("price = {:?}, market cap = {:?}", fast.last_price, fast.market_cap);

let info = aapl.info().await?;
println!("{} — {}", info.long_name.unwrap_or_default(), info.sector.unwrap_or_default());
```

### Fundamentals

```rust
let income = aapl.income_stmt().await?;
let bs = aapl.balance_sheet().await?;
let cf = aapl.quarterly_cashflow().await?;
println!("revenue series: {:?}", income.series.get("TotalRevenue"));
```

The full income statement, balance sheet and cash flow key lists from the
upstream library (~200 metrics) are mirrored in `src/fundamentals.rs`. Yahoo
silently drops keys that don't apply to a given symbol.

### Repair

```rust
let mut hist = aapl
    .history()
    .period(Period::Y2)
    .repair(true) // detect 100× currency mixups, zero closes, missed splits
    .fetch()
    .await?;

// Or, after the fact:
let report = hist.repair();
if report.any() { eprintln!("fixed something: {:?}", report); }
```

### Multi-ticker download

```rust
use yfinance::{DownloadBuilder, Period};

let multi = DownloadBuilder::new(&client, ["AAPL", "MSFT", "NVDA"])
    .period(Period::M1)
    .concurrency(8)
    .fetch()
    .await?;

for (sym, h) in &multi.series {
    println!("{}: {} bars", sym, h.rows.len());
}
```

### Options

```rust
let exp = aapl.options().await?;
let chain = aapl.option_chain(exp.expirations.first().copied()).await?;
println!("{} calls / {} puts", chain.calls.len(), chain.puts.len());
```

### Search & lookup

```rust
use yfinance::{LookupBuilder, LookupType, SearchBuilder};

let s = SearchBuilder::new(&client, "apple").max_results(5).fetch().await?;
let l = LookupBuilder::new(&client, "apple").kind(LookupType::Equity).fetch().await?;
```

## Examples

```bash
cargo run --example ticker_history -- AAPL
cargo run --example ticker_info -- MSFT
cargo run --example download_multi -- AAPL MSFT GOOG NVDA
cargo run --example search -- "apple"
cargo run --example options_chain -- AAPL
```

## Architecture

| File | Module |
| ---- | ------ |
| `src/client.rs` | `YfClient` — cookie / crumb session, retries, host config. |
| `src/types.rs` | `Period`, `Interval` enums. |
| `src/error.rs` | `Error` / `Result`. |
| `src/ticker.rs` | `Ticker` — entry point per symbol. |
| `src/history.rs` | `v8/finance/chart` — OHLCV + actions. |
| `src/quote.rs` | `v7/finance/quote` — `FastInfo`. |
| `src/info.rs` | `quoteSummary` modules — `Info`. |
| `src/fundamentals.rs` | `fundamentals-timeseries` — financial statements. |
| `src/holders.rs` | Ownership / insider via `quoteSummary`. |
| `src/options.rs` | `v7/finance/options` — option chain. |
| `src/repair.rs` | Local repair passes (currency unit, zero close, bad split). |
| `src/search.rs` | `v1/finance/search`. |
| `src/lookup.rs` | `v1/finance/lookup`. |
| `src/domain.rs` | `Market`, `Sector`, `Industry`. |
| `src/download.rs` | Concurrent multi-ticker batcher. |

`YfClient` is cheap to clone (`Arc` internally). It serializes crumb refreshes
through a single `tokio::sync::Mutex`, so concurrent callers share one
handshake. The cookie store is provided by `reqwest`'s built-in jar.

## Status — what's implemented vs the Python library

| Python feature | Status | Notes |
| -------------- | ------ | ----- |
| `Ticker.history` | ✅ | OHLCV, dividends, splits, capital gains, auto/back-adjust |
| `Ticker.fast_info` | ✅ | via `v7/finance/quote` |
| `Ticker.info` | ✅ | via `quoteSummary` (default modules); raw payload exposed |
| `Ticker.{income_stmt,balance_sheet,cashflow}` | ✅ | annual + quarterly |
| `Ticker.{major,institutional,mutualfund}_holders` | ✅ | via `Holders` struct |
| `Ticker.insider_{transactions,roster_holders}` | ✅ | |
| `Ticker.option_chain` | ✅ | calls + puts + expirations |
| `download(...)` | ✅ | bounded concurrency, per-symbol error reporting |
| `Search`, `Lookup` | ✅ | |
| `Sector`, `Industry`, `Market` | ✅ | |
| `Ticker.recommendations`, `analyst_price_targets`, `earnings_*` | 🟡 | available via `info.raw` until typed wrappers land |
| `Ticker.news` | 🟡 | available via `Search.news`; dedicated endpoint pending |
| Price repair (`repair=true`) | ✅ | 100× currency mixup, zero-close fill, missed-split rescale |
| Parquet/Pickle cache (`yfinance/cache.py`) | ❌ | use the rich crate ecosystem (e.g. `polars`) |
| WebSocket live stream (`yfinance/live.py`) | ❌ | requires protobuf; tracked as a follow-up |
| Screener DSL (`EquityQuery`, `screen(...)`) | ❌ | follow-up |
| `Tickers` batch object | ❌ | use `download` |

✅ implemented · 🟡 partial / via raw · ❌ not yet

Pull requests for the remaining items are very welcome.

## Testing

- `cargo test` — runs unit tests (parsers, type round-trips) and `httpmock`-based
  integration tests for the chart endpoint.
- Live-network tests are not run by default. To exercise real Yahoo APIs:
  `cargo test --features integration-tests -- --ignored` (no such tests are
  shipped in 0.1; the feature flag is reserved for them).

## Minimum supported Rust version

MSRV is **1.75** and is enforced in CI.

## License

Licensed under the [Apache License, Version 2.0](LICENSE). See [NOTICE](NOTICE).
