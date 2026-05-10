//! Async Rust client for Yahoo Finance.
//!
//! This crate is a port of the popular Python [`yfinance`](https://github.com/ranaroussi/yfinance)
//! library. It provides access to Yahoo Finance market data: quotes, historical OHLCV,
//! fundamentals, holders, options chains, search, and more.
//!
//! # Quick start
//!
//! ```no_run
//! use yfinance::{YfClient, Ticker, Period, Interval};
//!
//! # async fn run() -> yfinance::Result<()> {
//! let client = YfClient::new()?;
//! let ticker = Ticker::new(&client, "AAPL");
//!
//! // 1 month of daily candles
//! let history = ticker
//!     .history()
//!     .period(Period::M1)
//!     .interval(Interval::D1)
//!     .auto_adjust(true)
//!     .fetch()
//!     .await?;
//!
//! for row in &history.rows {
//!     println!("{}  O={} H={} L={} C={} V={}",
//!         row.timestamp, row.open, row.high, row.low, row.close, row.volume);
//! }
//!
//! // Fundamentals
//! let info = ticker.info().await?;
//! println!("{}: {}", info.symbol, info.long_name.unwrap_or_default());
//! # Ok(()) }
//! ```
//!
//! # Disclaimer
//!
//! This library is **not** affiliated with, endorsed, or vetted by Yahoo, Inc.
//! It uses Yahoo's publicly available APIs. Use at your own risk and respect
//! their [terms of service](https://policies.yahoo.com/us/en/yahoo/terms/index.htm).

#![deny(rust_2018_idioms)]
#![warn(missing_debug_implementations)]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod analysis;
pub mod calendar;
pub mod client;
#[cfg(feature = "dataframe")]
pub mod dataframe;
pub mod domain;
pub mod download;
pub mod earnings;
pub mod error;
pub mod esg;
pub mod fundamentals;
pub mod history;
pub mod holders;
pub mod info;
pub mod isin;
pub mod lookup;
pub mod news;
pub mod options;
pub mod profile;
pub mod quote;
pub mod repair;
pub mod search;
pub mod shares;
#[cfg(feature = "stream")]
pub mod stream;
pub mod ticker;
pub mod types;

pub mod test_fixtures;

pub(crate) mod wire;

/// Forward our internal `log` events to the `tracing` ecosystem. Idempotent
/// — safe to call multiple times. No-op without the `tracing` feature.
#[cfg(feature = "tracing")]
pub fn init_tracing_bridge() {
    let _ = tracing_log::LogTracer::init();
}

/// Initialise a default `tracing-subscriber` for tests / examples. Reads
/// `RUST_LOG` for the env-filter and falls back to `info`. Available with
/// the `tracing-subscriber` feature.
#[cfg(feature = "tracing-subscriber")]
pub fn init_tracing_for_tests() {
    use tracing_subscriber::{fmt, EnvFilter};
    init_tracing_bridge();
    let filter = std::env::var("RUST_LOG")
        .ok()
        .and_then(|s| EnvFilter::try_new(s).ok())
        .unwrap_or_else(|| EnvFilter::new("info"));
    let _ = fmt::Subscriber::builder()
        .with_env_filter(filter)
        .with_target(true)
        .try_init();
}

pub use analysis::{
    EarningsEstimate, EarningsTrendRow, EpsRevisions, EpsTrend, PriceTarget, RecommendationRow,
    RecommendationSummary, RevenueEstimate, UpgradeDowngradeRow,
};
pub use calendar::Calendar;
pub use client::{ApiPreference, CacheMode, RetryConfig, YfClient, YfClientBuilder};
pub use domain::{Industry, Market, Sector};
pub use download::{download, DownloadBuilder, MultiHistory};
pub use earnings::{Earnings, EarningsEps, EarningsQuarter, EarningsYear};
pub use error::{Error, Result};
pub use esg::{EsgInvolvement, EsgScores, EsgSummary};
pub use history::{Action, History, HistoryBuilder, OhlcvRow};
pub use info::Info;
pub use lookup::{Lookup, LookupBuilder, LookupRow, LookupType};
pub use news::{NewsArticle, NewsBuilder, NewsTab};
pub use options::{OptionChain, OptionContract};
pub use profile::{Address, CompanyProfile, FundProfile, Profile};
pub use quote::{batch as batch_quotes, FastInfo, Quote};
pub use repair::{repair_history, RepairReport};
pub use search::{Search, SearchBuilder, SearchNews, SearchQuote};
pub use shares::ShareCount;
pub use ticker::Ticker;
pub use types::{Interval, Period};
