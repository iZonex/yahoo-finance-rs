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

pub mod client;
pub mod domain;
pub mod download;
pub mod error;
pub mod fundamentals;
pub mod history;
pub mod holders;
pub mod info;
pub mod lookup;
pub mod options;
pub mod quote;
pub mod repair;
pub mod search;
pub mod ticker;
pub mod types;

pub use client::{YfClient, YfClientBuilder};
pub use domain::{Industry, Market, Sector};
pub use download::{download, DownloadBuilder, MultiHistory};
pub use error::{Error, Result};
pub use history::{Action, History, HistoryBuilder, OhlcvRow};
pub use info::Info;
pub use lookup::{Lookup, LookupBuilder, LookupRow, LookupType};
pub use options::{OptionChain, OptionContract};
pub use quote::FastInfo;
pub use repair::{repair_history, RepairReport};
pub use search::{Search, SearchBuilder, SearchNews, SearchQuote};
pub use ticker::Ticker;
pub use types::{Interval, Period};
