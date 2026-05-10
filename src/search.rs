//! Yahoo Finance search.
//!
//! Maps to `https://query2.finance.yahoo.com/v1/finance/search`. Returns
//! candidate quotes (typeahead style) plus news and curated lists.

use serde::Deserialize;

use crate::client::YfClient;
use crate::error::Result;

/// One quote-like result from search.
#[derive(Debug, Clone, Deserialize)]
pub struct SearchQuote {
    /// Symbol.
    pub symbol: String,
    /// Quote type (`EQUITY`, `ETF`, …).
    #[serde(default, rename = "quoteType")]
    pub quote_type: Option<String>,
    /// Exchange.
    #[serde(default)]
    pub exchange: Option<String>,
    /// Short name.
    #[serde(default, rename = "shortname")]
    pub short_name: Option<String>,
    /// Long name.
    #[serde(default, rename = "longname")]
    pub long_name: Option<String>,
    /// Industry (equities only).
    #[serde(default)]
    pub industry: Option<String>,
    /// Sector (equities only).
    #[serde(default)]
    pub sector: Option<String>,
}

/// One news article result.
#[derive(Debug, Clone, Deserialize)]
pub struct SearchNews {
    /// Article uuid.
    #[serde(default)]
    pub uuid: Option<String>,
    /// Headline.
    #[serde(default)]
    pub title: Option<String>,
    /// Publisher name.
    #[serde(default)]
    pub publisher: Option<String>,
    /// Permalink.
    #[serde(default)]
    pub link: Option<String>,
    /// Published time (UNIX seconds).
    #[serde(default, rename = "providerPublishTime")]
    pub provider_publish_time: Option<i64>,
    /// Tickers mentioned.
    #[serde(default, rename = "relatedTickers")]
    pub related_tickers: Vec<String>,
}

/// Search result.
#[derive(Debug, Clone)]
pub struct Search {
    /// The query that produced these results.
    pub query: String,
    /// Quote-like matches.
    pub quotes: Vec<SearchQuote>,
    /// News articles.
    pub news: Vec<SearchNews>,
}

/// Builder for [`Search`].
#[derive(Debug)]
pub struct SearchBuilder<'a> {
    client: &'a YfClient,
    query: String,
    max_results: usize,
    news_count: usize,
    enable_fuzzy: bool,
}

impl<'a> SearchBuilder<'a> {
    /// Create a new search builder.
    pub fn new(client: &'a YfClient, query: impl Into<String>) -> Self {
        Self {
            client,
            query: query.into(),
            max_results: 10,
            news_count: 5,
            enable_fuzzy: true,
        }
    }

    /// Maximum number of quote matches to return (default 10).
    pub fn max_results(mut self, n: usize) -> Self {
        self.max_results = n;
        self
    }

    /// Maximum number of news articles to return (default 5).
    pub fn news_count(mut self, n: usize) -> Self {
        self.news_count = n;
        self
    }

    /// Enable fuzzy matching (default `true`).
    pub fn fuzzy(mut self, on: bool) -> Self {
        self.enable_fuzzy = on;
        self
    }

    /// Run the request.
    pub async fn fetch(self) -> Result<Search> {
        #[derive(Debug, Deserialize)]
        struct Envelope {
            #[serde(default)]
            quotes: Vec<SearchQuote>,
            #[serde(default)]
            news: Vec<SearchNews>,
        }

        let q = vec![
            ("q", self.query.clone()),
            ("quotesCount", self.max_results.to_string()),
            ("newsCount", self.news_count.to_string()),
            ("enableFuzzyQuery", self.enable_fuzzy.to_string()),
            ("lang", "en-US".to_string()),
            ("region", "US".to_string()),
        ];
        let env: Envelope = self.client.get_json("/v1/finance/search", &q).await?;
        Ok(Search {
            query: self.query,
            quotes: env.quotes,
            news: env.news,
        })
    }
}

/// Convenience: fully-defaulted search.
pub async fn search(client: &YfClient, query: impl Into<String>) -> Result<Search> {
    SearchBuilder::new(client, query).fetch().await
}
