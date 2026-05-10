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

impl FastInfo {
    pub(crate) async fn fetch(client: &YfClient, symbol: &str) -> Result<Self> {
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

        let path = "/v7/finance/quote";
        let q = vec![("symbols", symbol.to_string())];
        let env: QuoteResponseEnvelope = client.get_json_crumb(path, &q).await?;
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
