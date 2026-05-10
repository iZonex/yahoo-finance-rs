//! Full company / fund info from `quoteSummary`.
//!
//! Yahoo's `v10/finance/quoteSummary` endpoint returns a tree of *modules*. We
//! request the full set used by `yfinance.Ticker.info` and flatten the most
//! commonly-consumed fields into [`Info`]. The raw module tree is preserved as
//! [`Info::raw`] so callers can dig deeper.

use serde_json::Value;

use crate::client::YfClient;
use crate::error::{Error, Result};

/// Default modules requested by [`Ticker::info`](crate::Ticker::info) —
/// equivalent to the union fetched by Python `yfinance` when `Ticker.info`
/// is accessed.
pub const DEFAULT_MODULES: &[&str] = &[
    "summaryProfile",
    "summaryDetail",
    "assetProfile",
    "fundProfile",
    "price",
    "quoteType",
    "esgScores",
    "defaultKeyStatistics",
    "financialData",
    "calendarEvents",
    "secFilings",
    "earnings",
    "earningsHistory",
    "earningsTrend",
    "industryTrend",
    "indexTrend",
    "sectorTrend",
    "recommendationTrend",
];

/// Aggregated company / fund info.
#[derive(Debug, Clone)]
pub struct Info {
    /// The requested symbol.
    pub symbol: String,
    /// `EQUITY`, `ETF`, …
    pub quote_type: Option<String>,
    /// Long company name.
    pub long_name: Option<String>,
    /// Short ticker name.
    pub short_name: Option<String>,
    /// GICS sector (equities only).
    pub sector: Option<String>,
    /// GICS industry (equities only).
    pub industry: Option<String>,
    /// Long-form business summary.
    pub summary: Option<String>,
    /// Headquarters country.
    pub country: Option<String>,
    /// Website URL.
    pub website: Option<String>,
    /// Reporting currency.
    pub currency: Option<String>,
    /// Market capitalization in reporting currency.
    pub market_cap: Option<u64>,
    /// Last regular-market trade price.
    pub regular_market_price: Option<f64>,
    /// Trailing 12-month P/E ratio.
    pub trailing_pe: Option<f64>,
    /// Forward P/E ratio.
    pub forward_pe: Option<f64>,
    /// Trailing 12-month EPS.
    pub trailing_eps: Option<f64>,
    /// Forward EPS estimate.
    pub forward_eps: Option<f64>,
    /// Most recent dividend rate (annualized).
    pub dividend_rate: Option<f64>,
    /// Trailing dividend yield (decimal).
    pub dividend_yield: Option<f64>,
    /// Beta (5y, monthly).
    pub beta: Option<f64>,
    /// Raw `quoteSummary` payload, keyed by module name.
    pub raw: serde_json::Map<String, Value>,
}

impl Info {
    pub(crate) async fn fetch(client: &YfClient, symbol: &str) -> Result<Self> {
        Self::fetch_modules(client, symbol, DEFAULT_MODULES).await
    }

    /// Fetch a custom subset of `quoteSummary` modules.
    pub async fn fetch_modules(client: &YfClient, symbol: &str, modules: &[&str]) -> Result<Self> {
        let modules_map = client
            .fetch_quote_summary(symbol, &modules.join(","), "info_quoteSummary")
            .await?
            .ok_or_else(|| Error::TickerMissing {
                ticker: symbol.to_string(),
                reason: "quoteSummary returned empty result".into(),
            })?;
        Ok(flatten_info(symbol, modules_map))
    }
}

fn flatten_info(symbol: &str, modules: serde_json::Map<String, Value>) -> Info {
    let get = |module: &str, key: &str| -> Option<Value> {
        modules
            .get(module)
            .and_then(|m| m.get(key))
            .cloned()
            .filter(|v| !v.is_null())
    };
    let raw_field = |v: Value| -> Option<Value> {
        if let Value::Object(m) = &v {
            if let Some(r) = m.get("raw") {
                return Some(r.clone());
            }
        }
        if v.is_null() {
            None
        } else {
            Some(v)
        }
    };
    let as_str = |v: Option<Value>| -> Option<String> {
        v.and_then(raw_field).and_then(|x| match x {
            Value::String(s) => Some(s),
            _ => None,
        })
    };
    let as_f64 = |v: Option<Value>| -> Option<f64> {
        v.and_then(raw_field).and_then(|x| match x {
            Value::Number(n) => n.as_f64(),
            _ => None,
        })
    };
    let as_u64 = |v: Option<Value>| -> Option<u64> {
        v.and_then(raw_field).and_then(|x| match x {
            Value::Number(n) => n.as_u64().or_else(|| n.as_f64().map(|f| f as u64)),
            _ => None,
        })
    };

    Info {
        symbol: symbol.to_string(),
        quote_type: as_str(get("quoteType", "quoteType")),
        long_name: as_str(get("price", "longName"))
            .or_else(|| as_str(get("quoteType", "longName"))),
        short_name: as_str(get("price", "shortName"))
            .or_else(|| as_str(get("quoteType", "shortName"))),
        sector: as_str(get("assetProfile", "sector"))
            .or_else(|| as_str(get("summaryProfile", "sector"))),
        industry: as_str(get("assetProfile", "industry"))
            .or_else(|| as_str(get("summaryProfile", "industry"))),
        summary: as_str(get("assetProfile", "longBusinessSummary"))
            .or_else(|| as_str(get("summaryProfile", "longBusinessSummary"))),
        country: as_str(get("assetProfile", "country"))
            .or_else(|| as_str(get("summaryProfile", "country"))),
        website: as_str(get("assetProfile", "website"))
            .or_else(|| as_str(get("summaryProfile", "website"))),
        currency: as_str(get("price", "currency"))
            .or_else(|| as_str(get("summaryDetail", "currency"))),
        market_cap: as_u64(get("price", "marketCap"))
            .or_else(|| as_u64(get("summaryDetail", "marketCap"))),
        regular_market_price: as_f64(get("price", "regularMarketPrice")),
        trailing_pe: as_f64(get("summaryDetail", "trailingPE")),
        forward_pe: as_f64(get("summaryDetail", "forwardPE")),
        trailing_eps: as_f64(get("defaultKeyStatistics", "trailingEps")),
        forward_eps: as_f64(get("defaultKeyStatistics", "forwardEps")),
        dividend_rate: as_f64(get("summaryDetail", "dividendRate")),
        dividend_yield: as_f64(get("summaryDetail", "dividendYield")),
        beta: as_f64(get("summaryDetail", "beta")),
        raw: modules,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flatten_unwraps_raw_fields() {
        let json = serde_json::json!({
            "price": {
                "regularMarketPrice": { "raw": 150.25, "fmt": "150.25" },
                "longName": "Apple Inc.",
                "currency": "USD",
                "marketCap": { "raw": 3000000000000_u64 }
            },
            "quoteType": { "quoteType": "EQUITY" },
            "summaryDetail": {
                "trailingPE": { "raw": 30.5 }
            },
            "assetProfile": {
                "sector": "Technology",
                "industry": "Consumer Electronics",
                "country": "United States"
            }
        });
        let modules = json.as_object().unwrap().clone();
        let info = flatten_info("AAPL", modules);
        assert_eq!(info.symbol, "AAPL");
        assert_eq!(info.long_name.as_deref(), Some("Apple Inc."));
        assert_eq!(info.currency.as_deref(), Some("USD"));
        assert_eq!(info.regular_market_price, Some(150.25));
        assert_eq!(info.market_cap, Some(3_000_000_000_000));
        assert_eq!(info.sector.as_deref(), Some("Technology"));
        assert_eq!(info.trailing_pe, Some(30.5));
        assert_eq!(info.quote_type.as_deref(), Some("EQUITY"));
    }
}
