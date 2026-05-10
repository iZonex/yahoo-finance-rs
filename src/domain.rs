//! Market / Sector / Industry domain objects.
//!
//! Yahoo's `quoteSummary` exposes hierarchical taxonomy data and relative
//! performance for each region/sector/industry. This module provides thin
//! wrappers over those endpoints.

use serde::Deserialize;
use serde_json::Value;

use crate::client::YfClient;
use crate::error::{Error, Result};

/// Market summary by region (e.g. `US`, `EU`, `ASIA`).
#[derive(Debug, Clone)]
pub struct Market {
    /// Region key (`US`, …).
    pub region: String,
    /// Per-index summary entries.
    pub summary: Vec<MarketSummaryEntry>,
}

/// Single index summary returned by `marketSummary`.
#[derive(Debug, Clone, Deserialize)]
pub struct MarketSummaryEntry {
    /// Symbol of the index (`^GSPC`, …).
    pub symbol: String,
    /// Quote type.
    #[serde(default, rename = "quoteType")]
    pub quote_type: Option<String>,
    /// Short display name.
    #[serde(default, rename = "shortName")]
    pub short_name: Option<String>,
    /// Last regular-market price.
    #[serde(default, rename = "regularMarketPrice")]
    pub regular_market_price: Option<f64>,
    /// Change since prior close.
    #[serde(default, rename = "regularMarketChange")]
    pub regular_market_change: Option<f64>,
    /// Percent change since prior close.
    #[serde(default, rename = "regularMarketChangePercent")]
    pub regular_market_change_percent: Option<f64>,
}

impl Market {
    /// Fetch summary for `region` (`US`, `EU`, `ASIA`, …).
    pub async fn fetch(client: &YfClient, region: impl Into<String>) -> Result<Self> {
        let region: String = region.into();
        #[derive(Debug, Deserialize)]
        struct Envelope {
            #[serde(rename = "marketSummaryResponse")]
            response: Inner,
        }
        #[derive(Debug, Deserialize)]
        struct Inner {
            #[serde(default)]
            result: Vec<MarketSummaryEntry>,
            #[serde(default)]
            error: Option<Value>,
        }
        let q = vec![("region", region.clone()), ("lang", "en-US".into())];
        let env: Envelope = client
            .get_json(
                "/v6/finance/quote/marketSummary",
                &q,
                Some(("market_summary", &region)),
            )
            .await?;
        if let Some(err) = env.response.error {
            return Err(Error::Yahoo {
                symbol: region.clone(),
                code: "market_error".into(),
                description: err.to_string(),
            });
        }
        Ok(Market {
            region,
            summary: env.response.result,
        })
    }
}

/// Sector taxonomy node (e.g. `technology`).
#[derive(Debug, Clone)]
pub struct Sector {
    /// Slug identifier (e.g. `technology`).
    pub key: String,
    /// Display name.
    pub name: Option<String>,
    /// Industries within the sector.
    pub industries: Vec<TaxonomyNode>,
    /// Top companies (limited).
    pub top_companies: Vec<TaxonomyNode>,
    /// Raw `quoteSummary` payload.
    pub raw: serde_json::Map<String, Value>,
}

/// Industry taxonomy node (e.g. `software-infrastructure`).
#[derive(Debug, Clone)]
pub struct Industry {
    /// Slug identifier.
    pub key: String,
    /// Display name.
    pub name: Option<String>,
    /// Top companies in this industry.
    pub top_companies: Vec<TaxonomyNode>,
    /// Raw `quoteSummary` payload.
    pub raw: serde_json::Map<String, Value>,
}

/// Generic taxonomy entry — used both for nested industries (under a sector)
/// and for top-companies lists.
#[derive(Debug, Clone)]
pub struct TaxonomyNode {
    /// Symbol or slug.
    pub key: String,
    /// Display name.
    pub name: Option<String>,
    /// Optional weight or market cap.
    pub weight: Option<f64>,
}

impl Sector {
    /// Fetch a sector by slug (e.g. `"technology"`).
    pub async fn fetch(client: &YfClient, key: impl Into<String>) -> Result<Self> {
        let key: String = key.into();
        let modules = fetch_taxonomy(client, &key, "sector").await?;
        Ok(Sector {
            key: key.clone(),
            name: modules
                .get("name")
                .and_then(|v| v.as_str())
                .map(String::from),
            industries: take_node_list(&modules, "industries"),
            top_companies: take_node_list(&modules, "topCompanies"),
            raw: modules,
        })
    }
}

impl Industry {
    /// Fetch an industry by slug (e.g. `"software-infrastructure"`).
    pub async fn fetch(client: &YfClient, key: impl Into<String>) -> Result<Self> {
        let key: String = key.into();
        let modules = fetch_taxonomy(client, &key, "industry").await?;
        Ok(Industry {
            key: key.clone(),
            name: modules
                .get("name")
                .and_then(|v| v.as_str())
                .map(String::from),
            top_companies: take_node_list(&modules, "topCompanies"),
            raw: modules,
        })
    }
}

async fn fetch_taxonomy(
    client: &YfClient,
    key: &str,
    kind: &str,
) -> Result<serde_json::Map<String, Value>> {
    let modules = client
        .fetch_quote_summary(key, kind, "domain_quoteSummary")
        .await?
        .ok_or_else(|| Error::TickerMissing {
            ticker: key.to_string(),
            reason: format!("{kind} returned empty result"),
        })?;
    Ok(modules
        .get(kind)
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default())
}

fn take_node_list(map: &serde_json::Map<String, Value>, key: &str) -> Vec<TaxonomyNode> {
    let Some(arr) = map.get(key).and_then(|v| v.as_array()) else {
        return vec![];
    };
    arr.iter()
        .filter_map(|v| {
            let o = v.as_object()?;
            Some(TaxonomyNode {
                key: o
                    .get("key")
                    .or_else(|| o.get("symbol"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string(),
                name: o.get("name").and_then(|x| x.as_str()).map(String::from),
                weight: o.get("weight").and_then(|x| x.as_f64()),
            })
        })
        .collect()
}
