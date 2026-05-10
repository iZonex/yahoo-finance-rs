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

    /// ISIN for the symbol via the Business Insider suggest endpoint.
    ///
    /// Returns `None` when no candidate matches the symbol — Yahoo doesn't
    /// expose ISINs directly, so this lookup hits a third-party service that
    /// covers most equities and ETFs.
    pub async fn isin(&self) -> Result<Option<String>> {
        crate::isin::fetch(&self.client, &self.symbol).await
    }

    /// ESG / sustainability scores. Returns `None` when Yahoo has no coverage
    /// for the symbol — that path is increasingly common as Yahoo has been
    /// retiring the endpoint.
    pub async fn sustainability(&self) -> Result<Option<crate::esg::EsgSummary>> {
        crate::esg::EsgSummary::fetch(&self.client, &self.symbol).await
    }

    /// News-feed builder. Configure with [`NewsBuilder::tab`] /
    /// [`NewsBuilder::count`] then call `.fetch()`.
    ///
    /// [`NewsBuilder::tab`]: crate::news::NewsBuilder::tab
    /// [`NewsBuilder::count`]: crate::news::NewsBuilder::count
    pub fn news(&self) -> crate::news::NewsBuilder {
        crate::news::NewsBuilder::new(&self.client, &self.symbol)
    }

    /// Per-period analyst recommendation counts (`recommendationTrend`).
    pub async fn recommendations(&self) -> Result<Vec<crate::analysis::RecommendationRow>> {
        crate::analysis::recommendations(&self.client, &self.symbol).await
    }

    /// Latest analyst recommendation summary plus `recommendationMean`/`Key`.
    pub async fn recommendations_summary(&self) -> Result<crate::analysis::RecommendationSummary> {
        crate::analysis::recommendations_summary(&self.client, &self.symbol).await
    }

    /// History of analyst upgrades and downgrades (oldest first).
    pub async fn upgrades_downgrades(&self) -> Result<Vec<crate::analysis::UpgradeDowngradeRow>> {
        crate::analysis::upgrades_downgrades(&self.client, &self.symbol).await
    }

    /// Mean / high / low analyst price target plus contributor count.
    pub async fn price_target(&self) -> Result<crate::analysis::PriceTarget> {
        crate::analysis::price_target(&self.client, &self.symbol).await
    }

    /// Earnings & revenue estimates, EPS trend, EPS revisions across the four
    /// reporting periods (`0q`, `+1q`, `0y`, `+1y`).
    pub async fn earnings_trend(&self) -> Result<Vec<crate::analysis::EarningsTrendRow>> {
        crate::analysis::earnings_trend(&self.client, &self.symbol).await
    }

    /// Company / fund profile (sector, industry, summary, address, family,
    /// legal type). Returns [`Profile::Other`] for symbols that aren't
    /// equities or funds.
    ///
    /// [`Profile::Other`]: crate::profile::Profile::Other
    pub async fn profile(&self) -> Result<crate::profile::Profile> {
        crate::profile::fetch(&self.client, &self.symbol).await
    }

    /// Real-time quote stream builder. Available with the `stream` feature.
    /// Defaults to WebSocket transport.
    #[cfg(feature = "stream")]
    pub fn stream(&self) -> crate::stream::StreamBuilder {
        crate::stream::StreamBuilder::new(&self.client, &self.symbol)
    }

    /// Upcoming earnings, ex-dividend, and dividend-payment dates from
    /// `calendarEvents`. Returns an empty [`Calendar`] when Yahoo has no
    /// coverage.
    ///
    /// [`Calendar`]: crate::calendar::Calendar
    pub async fn calendar(&self) -> Result<crate::calendar::Calendar> {
        crate::calendar::fetch(&self.client, &self.symbol).await
    }

    /// Yearly + quarterly revenue/earnings totals plus EPS estimate-vs-actual
    /// from the `earnings` quoteSummary module.
    pub async fn earnings(&self) -> Result<crate::earnings::Earnings> {
        crate::earnings::fetch(&self.client, &self.symbol).await
    }

    /// Full quote snapshot including bid/ask, market state, and pre/post
    /// session prices. Same endpoint as [`fast_info`] but exposes more fields.
    ///
    /// [`fast_info`]: Ticker::fast_info
    pub async fn quote(&self) -> Result<crate::quote::Quote> {
        crate::quote::Quote::fetch(&self.client, &self.symbol).await
    }

    /// Annual reported shares outstanding via the fundamentals timeseries.
    pub async fn shares(&self) -> Result<Vec<crate::shares::ShareCount>> {
        crate::shares::fetch(&self.client, &self.symbol, false).await
    }

    /// Quarterly reported shares outstanding via the fundamentals timeseries.
    pub async fn quarterly_shares(&self) -> Result<Vec<crate::shares::ShareCount>> {
        crate::shares::fetch(&self.client, &self.symbol, true).await
    }
}
