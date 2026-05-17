# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2] - 2026-05-17

### Added

- **First trade date**: `History` now exposes `first_trade_date:
  Option<DateTime<Utc>>`, deserialized from the `firstTradeDate` field of
  the `v8/finance/chart` `meta` block. Lets consumers detect recently-listed
  symbols from a `Ticker::history()` response. `None` when Yahoo omits it.

## [0.1.1] - 2026-05-10

### Documentation

- README rewritten to match the actual 0.1 surface — adds quickstart, the
  full per-`Ticker` method list, snippets for streaming / dataframes / cache
  / browser-session injection, accurate architecture table, MSRV note (1.88).
- `[package.metadata.docs.rs]` now builds with `stream`, `dataframe`, and
  `tracing-subscriber` enabled so docs.rs renders the optional API surface.

## [0.1.0] - 2026-05-10

Initial public release. Async Rust port of the Python `yfinance` library, with
parity across the major Yahoo Finance endpoints and a few extras.

### Added — Ticker API

- **History**: `Ticker::history()` builder with `auto_adjust`, `back_adjust`,
  `keepna`, `rounding`, `repair`, `prepost`, range/period selection.
- **Quotes**: `Ticker::quote()` (full snapshot — bid/ask, market state,
  pre/post prices) and `Ticker::fast_info()` (compact subset).
- **Batch quotes**: `yfinance::batch_quotes(client, &[symbols])` — many
  symbols in one HTTP request.
- **Info**: `Ticker::info()` — flattened `quoteSummary` modules.
- **Fundamentals**: `income_stmt`, `quarterly_income_stmt`, `balance_sheet`,
  `quarterly_balance_sheet`, `cashflow`, `quarterly_cashflow`.
- **Earnings**: `Ticker::earnings()` — yearly/quarterly revenue+earnings plus
  EPS estimate-vs-actual.
- **Calendar**: `Ticker::calendar()` — upcoming earnings, ex-dividend, and
  dividend payment dates.
- **Shares outstanding**: `Ticker::shares()` and `quarterly_shares()`.
- **Analysis**: `recommendations`, `recommendations_summary`,
  `upgrades_downgrades`, `price_target`, `earnings_trend`.
- **ESG / sustainability**: `Ticker::sustainability()`.
- **Holders**: `Ticker::holders()` returning major / institutional /
  mutual-fund / insider transactions / insider roster / net-share-purchase.
- **Options**: `Ticker::options()`, `Ticker::option_chain(expiration)`.
- **News**: `Ticker::news()` with `NewsTab::{LatestNews, All, PressReleases}`.
- **Profile**: `Ticker::profile()` — company / fund profile with HTML scrape
  fallback when the `quoteSummary` API is empty.
- **ISIN lookup**: `Ticker::isin()` via Business Insider's suggest endpoint.

### Added — Top-level API

- `Search`, `Lookup`, `Market`, `Sector`, `Industry`.
- `download` builder for multi-ticker concurrent OHLCV.
- `YfClient` builder with: timeout, retries, exponential backoff,
  rotating User-Agent, cookie+crumb session, configurable base URLs (data,
  crumb, cookie-prime, ISIN, news, quote-page), `cache_ttl` for in-memory
  TTL cache, `api_preference` hint, `session_cookie` and `session_crumb`
  injection for environments with a pre-existing browser session.

### Added — Cargo features

- `stream` — WebSocket streaming with hand-transcribed `Yaticker.PricingData`
  protobuf, plus HTTP polling fallback. Per-update volume deltas.
- `dataframe` — `ToDataFrame` trait + impls for `History`, `MultiHistory`,
  `Vec<RecommendationRow>`, `Vec<UpgradeDowngradeRow>`, `Vec<NewsArticle>`,
  `FastInfo`, holder vectors. Backed by `polars` 0.51.
- `tracing` — bridges internal `log` events into the `tracing` ecosystem.
- `tracing-subscriber` — adds `init_tracing_for_tests()` dev convenience.
- `test-mode` — fixture recording; `YF_RECORD=1` writes responses to
  `tests/fixtures/{endpoint}_{symbol}.{ext}`.

### Added — Test infrastructure

- Live → record → replay flow via `just test-record` / `just test-offline`.
- `tests/common.rs` `httpmock` helpers covering every endpoint family.
- 29 test suites, including offline replays of recorded Yahoo responses
  for history, quote, info, holders, fundamentals, options, search, profile,
  ISIN, ESG, news, all five analysis methods.

### Notes

- The crate is published under `yahoo-finance-rs` on crates.io but the
  library name remains `yfinance` so user code reads `use yfinance::...`.
- Not affiliated with Yahoo, Inc.
