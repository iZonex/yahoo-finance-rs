//! Lightweight quote endpoint (`v7/finance/quote`) and `FastInfo`.
//!
//! [`FastInfo`] mirrors `Ticker.fast_info` from the Python library — a small
//! subset of frequently-needed fields fetched via a single, fast request.

use serde::Deserialize;

use crate::client::YfClient;
use crate::error::{Error, Result};

/// A small, "fast" subset of quote info — a single request to Yahoo's quote endpoint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FastInfo {
    /// The symbol echoed back.
    pub symbol: String,
    /// Quote type (`EQUITY`, `ETF`, `MUTUALFUND`, `CURRENCY`, `INDEX`, `FUTURE`, …).
    #[serde(default)]
    pub quote_type: Option<String>,
    /// ISO currency code.
    #[serde(default)]
    pub currency: Option<String>,
    /// Exchange code (`NMS`, `LSE`, …).
    #[serde(default)]
    pub exchange: Option<String>,
    /// Exchange timezone name (`America/New_York`).
    #[serde(default, rename = "exchangeTimezoneName")]
    pub timezone: Option<String>,

    /// Most recent traded price.
    #[serde(default, rename = "regularMarketPrice")]
    pub last_price: Option<f64>,
    /// Volume since session open.
    #[serde(default, rename = "regularMarketVolume")]
    pub last_volume: Option<u64>,
    /// Previous trading day's close.
    #[serde(default, rename = "regularMarketPreviousClose")]
    pub previous_close: Option<f64>,
    /// Today's opening price.
    #[serde(default, rename = "regularMarketOpen")]
    pub open: Option<f64>,
    /// Highest price today.
    #[serde(default, rename = "regularMarketDayHigh")]
    pub day_high: Option<f64>,
    /// Lowest price today.
    #[serde(default, rename = "regularMarketDayLow")]
    pub day_low: Option<f64>,
    /// 52-week high.
    #[serde(default, rename = "fiftyTwoWeekHigh")]
    pub year_high: Option<f64>,
    /// 52-week low.
    #[serde(default, rename = "fiftyTwoWeekLow")]
    pub year_low: Option<f64>,
    /// 50-day moving average price.
    #[serde(default, rename = "fiftyDayAverage")]
    pub fifty_day_average: Option<f64>,
    /// 200-day moving average price.
    #[serde(default, rename = "twoHundredDayAverage")]
    pub two_hundred_day_average: Option<f64>,
    /// 3-month average daily volume.
    #[serde(default, rename = "averageDailyVolume3Month")]
    pub three_month_average_volume: Option<u64>,
    /// 10-day average daily volume.
    #[serde(default, rename = "averageDailyVolume10Day")]
    pub ten_day_average_volume: Option<u64>,
    /// Market cap (USD).
    #[serde(default, rename = "marketCap")]
    pub market_cap: Option<u64>,
    /// Shares outstanding.
    #[serde(default, rename = "sharesOutstanding")]
    pub shares: Option<u64>,
}

/// Richer quote snapshot — same wire payload as [`FastInfo`] but exposes the
/// micro-structure fields (bid/ask/sizes, market state, change since prior
/// close) that `FastInfo` deliberately omits.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    /// Symbol echoed back.
    pub symbol: String,
    /// Quote type (`EQUITY`, `ETF`, …).
    #[serde(default)]
    pub quote_type: Option<String>,
    /// Market state (`REGULAR`, `PRE`, `POST`, `CLOSED`).
    #[serde(default, rename = "marketState")]
    pub market_state: Option<String>,
    /// ISO currency code.
    #[serde(default)]
    pub currency: Option<String>,
    /// Exchange code (`NMS`, …).
    #[serde(default)]
    pub exchange: Option<String>,
    /// Exchange display name.
    #[serde(default, rename = "fullExchangeName")]
    pub exchange_name: Option<String>,
    /// IANA timezone (`America/New_York`).
    #[serde(default, rename = "exchangeTimezoneName")]
    pub timezone: Option<String>,

    /// Last regular-session price.
    #[serde(default, rename = "regularMarketPrice")]
    pub price: Option<f64>,
    /// Day-over-day change.
    #[serde(default, rename = "regularMarketChange")]
    pub change: Option<f64>,
    /// Day-over-day change as a fraction-of-100 percentage.
    #[serde(default, rename = "regularMarketChangePercent")]
    pub change_percent: Option<f64>,
    /// Last trade timestamp (UNIX seconds).
    #[serde(default, rename = "regularMarketTime")]
    pub time: Option<i64>,
    /// Cumulative session volume.
    #[serde(default, rename = "regularMarketVolume")]
    pub volume: Option<u64>,
    /// Today's opening price.
    #[serde(default, rename = "regularMarketOpen")]
    pub open: Option<f64>,
    /// Highest price today.
    #[serde(default, rename = "regularMarketDayHigh")]
    pub day_high: Option<f64>,
    /// Lowest price today.
    #[serde(default, rename = "regularMarketDayLow")]
    pub day_low: Option<f64>,
    /// Previous trading day's close.
    #[serde(default, rename = "regularMarketPreviousClose")]
    pub previous_close: Option<f64>,

    /// Best bid price.
    #[serde(default)]
    pub bid: Option<f64>,
    /// Bid size (lots / 100-share units depending on exchange).
    #[serde(default, rename = "bidSize")]
    pub bid_size: Option<i64>,
    /// Best ask price.
    #[serde(default)]
    pub ask: Option<f64>,
    /// Ask size.
    #[serde(default, rename = "askSize")]
    pub ask_size: Option<i64>,
    /// Size of the last trade.
    #[serde(default, rename = "regularMarketLastSize")]
    pub last_size: Option<i64>,

    /// Pre-market price (only outside regular hours).
    #[serde(default, rename = "preMarketPrice")]
    pub pre_market_price: Option<f64>,
    /// Pre-market change.
    #[serde(default, rename = "preMarketChange")]
    pub pre_market_change: Option<f64>,
    /// Post-market (after-hours) price.
    #[serde(default, rename = "postMarketPrice")]
    pub post_market_price: Option<f64>,
    /// Post-market change.
    #[serde(default, rename = "postMarketChange")]
    pub post_market_change: Option<f64>,
}

impl Quote {
    pub(crate) async fn fetch(client: &YfClient, symbol: &str) -> Result<Self> {
        #[derive(Deserialize)]
        struct Envelope {
            #[serde(rename = "quoteResponse")]
            quote_response: Inner,
        }
        #[derive(Deserialize)]
        struct Inner {
            #[serde(default)]
            result: Vec<Quote>,
            #[serde(default)]
            error: Option<serde_json::Value>,
        }
        let q = vec![("symbols", symbol.to_string())];
        let env: Envelope = client
            .get_json_crumb("/v7/finance/quote", &q, Some(("quote_v7", symbol)))
            .await?;
        if let Some(err) = env.quote_response.error {
            return Err(Error::Yahoo {
                symbol: symbol.to_string(),
                code: "quote_error".into(),
                description: err.to_string(),
            });
        }
        env.quote_response
            .result
            .into_iter()
            .next()
            .ok_or_else(|| Error::TickerMissing {
                ticker: symbol.to_string(),
                reason: "quote endpoint returned no results".into(),
            })
    }
}

impl FastInfo {
    pub(crate) async fn fetch(client: &YfClient, symbol: &str) -> Result<Self> {
        let mut rows = fetch_many(client, std::slice::from_ref(&symbol)).await?;
        rows.pop().ok_or_else(|| Error::TickerMissing {
            ticker: symbol.to_string(),
            reason: "quote endpoint returned no results".into(),
        })
    }
}

/// Fetch quotes for many symbols in a single `/v7/finance/quote` call.
///
/// Symbols Yahoo doesn't recognise are silently dropped. Returns rows in the
/// order Yahoo emits them (typically request order).
pub async fn batch(client: &YfClient, symbols: &[&str]) -> Result<Vec<FastInfo>> {
    if symbols.is_empty() {
        return Ok(Vec::new());
    }
    fetch_many(client, symbols).await
}

async fn fetch_many(client: &YfClient, symbols: &[&str]) -> Result<Vec<FastInfo>> {
    #[derive(Deserialize)]
    struct QuoteResponseEnvelope {
        #[serde(rename = "quoteResponse")]
        quote_response: QuoteResponse,
    }
    #[derive(Deserialize)]
    struct QuoteResponse {
        #[serde(default)]
        result: Vec<FastInfo>,
        #[serde(default)]
        error: Option<serde_json::Value>,
    }

    let csv = symbols.join(",");
    let label = if symbols.len() == 1 {
        symbols[0].to_string()
    } else {
        format!("MULTI_{}", symbols.len())
    };
    let q = vec![("symbols", csv)];
    let env: QuoteResponseEnvelope = client
        .get_json_crumb("/v7/finance/quote", &q, Some(("quote_v7", &label)))
        .await?;
    if let Some(err) = env.quote_response.error {
        return Err(Error::Yahoo {
            symbol: symbols.join(","),
            code: "quote_error".into(),
            description: err.to_string(),
        });
    }
    Ok(env.quote_response.result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_minimal() {
        let json = serde_json::json!({
            "symbol": "AAPL",
            "currency": "USD",
            "regularMarketPrice": 150.25,
            "fiftyTwoWeekHigh": 200.0
        });
        let fi: FastInfo = serde_json::from_value(json).unwrap();
        assert_eq!(fi.symbol, "AAPL");
        assert_eq!(fi.currency.as_deref(), Some("USD"));
        assert_eq!(fi.last_price, Some(150.25));
        assert_eq!(fi.year_high, Some(200.0));
        assert!(fi.market_cap.is_none());
    }
}
