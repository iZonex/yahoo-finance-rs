//! Yahoo Finance lookup (`v1/finance/lookup`).
//!
//! Lookup is a typeahead variant tuned for resolving free-form queries to
//! tickers, with optional pricing data attached.

use serde::Deserialize;

use crate::client::YfClient;
use crate::error::Result;

/// Lookup record category. Equivalent to the `type` query parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LookupType {
    /// All quote types.
    All,
    /// Stocks.
    Equity,
    /// Exchange-traded funds.
    Etf,
    /// Mutual funds.
    MutualFund,
    /// Indices.
    Index,
    /// Currencies.
    Currency,
    /// Cryptocurrencies.
    Cryptocurrency,
    /// Futures.
    Future,
}

impl LookupType {
    fn as_str(self) -> &'static str {
        match self {
            LookupType::All => "all",
            LookupType::Equity => "equity",
            LookupType::Etf => "etf",
            LookupType::MutualFund => "mutualfund",
            LookupType::Index => "index",
            LookupType::Currency => "currency",
            LookupType::Cryptocurrency => "cryptocurrency",
            LookupType::Future => "future",
        }
    }
}

/// One row of a lookup result.
#[derive(Debug, Clone, Deserialize)]
pub struct LookupRow {
    /// Ticker symbol.
    #[serde(default)]
    pub symbol: String,
    /// Display name.
    #[serde(default, rename = "shortName")]
    pub short_name: Option<String>,
    /// Quote type (`EQUITY`, …).
    #[serde(default, rename = "quoteType")]
    pub quote_type: Option<String>,
    /// Exchange code.
    #[serde(default, rename = "exchange")]
    pub exchange: Option<String>,
    /// Industry name.
    #[serde(default)]
    pub industry: Option<String>,
    /// Sector name.
    #[serde(default)]
    pub sector: Option<String>,
    /// Last price (only when `fetch_pricing_data = true`).
    #[serde(default, rename = "regularMarketPrice")]
    pub last_price: Option<f64>,
}

/// Lookup result.
#[derive(Debug, Clone)]
pub struct Lookup {
    /// The query that produced these results.
    pub query: String,
    /// Matching rows.
    pub rows: Vec<LookupRow>,
}

/// Builder for [`Lookup`].
#[derive(Debug)]
pub struct LookupBuilder<'a> {
    client: &'a YfClient,
    query: String,
    kind: LookupType,
    count: usize,
    fetch_pricing_data: bool,
}

impl<'a> LookupBuilder<'a> {
    /// Create a new lookup builder.
    pub fn new(client: &'a YfClient, query: impl Into<String>) -> Self {
        Self {
            client,
            query: query.into(),
            kind: LookupType::All,
            count: 25,
            fetch_pricing_data: true,
        }
    }

    /// Restrict to a single quote type.
    pub fn kind(mut self, t: LookupType) -> Self {
        self.kind = t;
        self
    }

    /// Maximum rows to return (default 25).
    pub fn count(mut self, c: usize) -> Self {
        self.count = c;
        self
    }

    /// Whether to attach last price information (default `true`).
    pub fn fetch_pricing_data(mut self, on: bool) -> Self {
        self.fetch_pricing_data = on;
        self
    }

    /// Run the request.
    pub async fn fetch(self) -> Result<Lookup> {
        #[derive(Debug, Deserialize)]
        struct Envelope {
            #[serde(rename = "finance")]
            finance: Finance,
        }
        #[derive(Debug, Deserialize)]
        struct Finance {
            #[serde(default)]
            result: Vec<ResultBlock>,
        }
        #[derive(Debug, Deserialize)]
        struct ResultBlock {
            #[serde(default)]
            documents: Vec<LookupRow>,
        }

        let q = vec![
            ("query", self.query.clone()),
            ("type", self.kind.as_str().to_string()),
            ("count", self.count.to_string()),
            ("formatted", "false".into()),
            ("lang", "en-US".into()),
            ("region", "US".into()),
            ("fetchPricingData", self.fetch_pricing_data.to_string()),
        ];

        let label = format!(
            "{}_{}",
            self.kind.as_str(),
            crate::test_fixtures::safe_label(&self.query)
        );
        let env: Envelope = self
            .client
            .get_json("/v1/finance/lookup", &q, Some(("lookup", &label)))
            .await?;
        let rows = env
            .finance
            .result
            .into_iter()
            .flat_map(|b| b.documents)
            .collect();
        Ok(Lookup {
            query: self.query,
            rows,
        })
    }
}

/// Convenience: fully-defaulted lookup.
pub async fn lookup(client: &YfClient, query: impl Into<String>) -> Result<Lookup> {
    LookupBuilder::new(client, query).fetch().await
}
