//! [`Ticker`] — entry point for everything about a single symbol.

use crate::client::YfClient;
use crate::error::Result;
use crate::fundamentals::{Fundamentals, FundamentalsKind};
use crate::history::HistoryBuilder;
use crate::holders::Holders;
use crate::info::Info;
use crate::options::{OptionChain, OptionExpirations};
use crate::quote::FastInfo;

/// Handle to a single Yahoo Finance symbol.
///
/// Construct one from a [`YfClient`] and call its async methods to fetch data:
///
/// ```no_run
/// # async fn run() -> yfinance::Result<()> {
/// let client = yfinance::YfClient::new()?;
/// let aapl = yfinance::Ticker::new(&client, "AAPL");
/// let fast = aapl.fast_info().await?;
/// println!("last={:?}", fast.last_price);
/// # Ok(()) }
/// ```
#[derive(Debug, Clone)]
pub struct Ticker {
    client: YfClient,
    symbol: String,
}

impl Ticker {
    /// Create a ticker for `symbol`. The symbol is uppercased and trimmed.
    pub fn new(client: &YfClient, symbol: impl Into<String>) -> Self {
        let s: String = symbol.into();
        Self {
            client: client.clone(),
            symbol: s.trim().to_uppercase(),
        }
    }

    /// Underlying client.
    pub fn client(&self) -> &YfClient {
        &self.client
    }

    /// The normalized symbol (e.g. `"AAPL"`).
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// Start a history (OHLCV) request.
    ///
    /// See [`HistoryBuilder`] for the full set of options.
    pub fn history(&self) -> HistoryBuilder {
        HistoryBuilder::new(self.client.clone(), self.symbol.clone())
    }

    /// Lazy "fast" info (price, market cap, 52w high/low, currency, etc.).
    pub async fn fast_info(&self) -> Result<FastInfo> {
        FastInfo::fetch(&self.client, &self.symbol).await
    }

    /// Full company / fund info from `quoteSummary`.
    pub async fn info(&self) -> Result<Info> {
        Info::fetch(&self.client, &self.symbol).await
    }

    /// Annual income statement (most recent year first).
    pub async fn income_stmt(&self) -> Result<Fundamentals> {
        Fundamentals::fetch(
            &self.client,
            &self.symbol,
            FundamentalsKind::IncomeStatement,
            false,
        )
        .await
    }

    /// Quarterly income statement.
    pub async fn quarterly_income_stmt(&self) -> Result<Fundamentals> {
        Fundamentals::fetch(
            &self.client,
            &self.symbol,
            FundamentalsKind::IncomeStatement,
            true,
        )
        .await
    }

    /// Annual balance sheet.
    pub async fn balance_sheet(&self) -> Result<Fundamentals> {
        Fundamentals::fetch(
            &self.client,
            &self.symbol,
            FundamentalsKind::BalanceSheet,
            false,
        )
        .await
    }

    /// Quarterly balance sheet.
    pub async fn quarterly_balance_sheet(&self) -> Result<Fundamentals> {
        Fundamentals::fetch(
            &self.client,
            &self.symbol,
            FundamentalsKind::BalanceSheet,
            true,
        )
        .await
    }

    /// Annual cash flow statement.
    pub async fn cashflow(&self) -> Result<Fundamentals> {
        Fundamentals::fetch(
            &self.client,
            &self.symbol,
            FundamentalsKind::CashFlow,
            false,
        )
        .await
    }

    /// Quarterly cash flow statement.
    pub async fn quarterly_cashflow(&self) -> Result<Fundamentals> {
        Fundamentals::fetch(&self.client, &self.symbol, FundamentalsKind::CashFlow, true).await
    }

    /// Holders breakdown (major / institutional / mutual fund / insider).
    pub async fn holders(&self) -> Result<Holders> {
        Holders::fetch(&self.client, &self.symbol).await
    }

    /// List of option-chain expiration dates (UNIX seconds).
    pub async fn options(&self) -> Result<OptionExpirations> {
        OptionExpirations::fetch(&self.client, &self.symbol).await
    }

    /// Option chain for a single expiration. If `expiration` is `None`, returns the nearest.
    pub async fn option_chain(&self, expiration: Option<i64>) -> Result<OptionChain> {
        OptionChain::fetch(&self.client, &self.symbol, expiration).await
    }
}
