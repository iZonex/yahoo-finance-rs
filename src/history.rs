//! Historical OHLCV (`v8/finance/chart`).
//!
//! Mirrors the behavior of `yfinance/scrapers/history.py`: parses the chart
//! payload into typed [`OhlcvRow`]s, attaches dividends/splits/capital-gains
//! events, optionally adjusts OHLC by the `adjclose` factor (`auto_adjust`),
//! and rounds prices to the precision Yahoo reports.

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use serde::Deserialize;

use crate::client::YfClient;
use crate::error::{Error, Result};
use crate::types::{Interval, Period};

/// One trading bar.
#[derive(Debug, Clone, PartialEq)]
pub struct OhlcvRow {
    /// Bar timestamp (in the exchange timezone — see [`History::timezone`]).
    pub timestamp: DateTime<Utc>,
    /// Opening price.
    pub open: f64,
    /// Highest price.
    pub high: f64,
    /// Lowest price.
    pub low: f64,
    /// Closing price (after `auto_adjust` if requested).
    pub close: f64,
    /// Yahoo's adjusted close.
    pub adj_close: f64,
    /// Volume traded during the bar.
    pub volume: u64,
    /// Dividend declared on this bar (0.0 if none).
    pub dividend: f64,
    /// Split factor on this bar (0.0 if none, otherwise `numerator / denominator`).
    pub split: f64,
    /// Capital gains distribution (mutual funds), 0.0 if none.
    pub capital_gain: f64,
}

/// Standalone corporate action (dividend / split / capital gain).
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Cash dividend.
    Dividend {
        /// Effective date.
        date: DateTime<Utc>,
        /// Per-share amount.
        amount: f64,
    },
    /// Stock split.
    Split {
        /// Effective date.
        date: DateTime<Utc>,
        /// Numerator of the split ratio (e.g. 4 for 4-for-1).
        numerator: f64,
        /// Denominator of the split ratio.
        denominator: f64,
    },
    /// Capital gains distribution (mutual funds).
    CapitalGain {
        /// Effective date.
        date: DateTime<Utc>,
        /// Per-share amount.
        amount: f64,
    },
}

/// A history response.
#[derive(Debug, Clone, Default)]
pub struct History {
    /// Symbol echoed from the response.
    pub symbol: String,
    /// Currency (`"USD"`, `"EUR"`, …).
    pub currency: Option<String>,
    /// Exchange-reported timezone (`"America/New_York"`, …).
    pub timezone: Option<String>,
    /// Exchange display name (`"NasdaqGS"`, …).
    pub exchange_name: Option<String>,
    /// Bars in chronological order.
    pub rows: Vec<OhlcvRow>,
    /// Standalone events (dividends/splits/capital gains).
    pub actions: Vec<Action>,
}

impl History {
    /// Parsed local timezone, if Yahoo reported one.
    pub fn local_tz(&self) -> Option<Tz> {
        self.timezone.as_deref().and_then(|s| s.parse().ok())
    }

    /// Bars expressed as `(NaiveDate, OhlcvRow)` in the exchange timezone.
    /// Useful for daily series.
    pub fn rows_local_date(&self) -> Vec<(NaiveDate, &OhlcvRow)> {
        let tz = self.local_tz();
        self.rows
            .iter()
            .map(|r| {
                let date = match tz {
                    Some(z) => r.timestamp.with_timezone(&z).date_naive(),
                    None => r.timestamp.date_naive(),
                };
                (date, r)
            })
            .collect()
    }

    /// Just the dividends, in order.
    pub fn dividends(&self) -> impl Iterator<Item = (DateTime<Utc>, f64)> + '_ {
        self.actions.iter().filter_map(|a| match a {
            Action::Dividend { date, amount } => Some((*date, *amount)),
            _ => None,
        })
    }

    /// Just the splits, in order.
    pub fn splits(&self) -> impl Iterator<Item = (DateTime<Utc>, f64, f64)> + '_ {
        self.actions.iter().filter_map(|a| match a {
            Action::Split {
                date,
                numerator,
                denominator,
            } => Some((*date, *numerator, *denominator)),
            _ => None,
        })
    }

    /// Run all [`repair`](crate::repair) passes against this history in place.
    /// Returns a [`RepairReport`](crate::RepairReport) with counts.
    pub fn repair(&mut self) -> crate::repair::RepairReport {
        crate::repair::repair_history(self)
    }
}

/// Builder for a history (`v8/finance/chart`) request.
#[derive(Debug, Clone)]
pub struct HistoryBuilder {
    client: YfClient,
    symbol: String,
    period: Option<Period>,
    interval: Interval,
    start: Option<i64>,
    end: Option<i64>,
    prepost: bool,
    auto_adjust: bool,
    back_adjust: bool,
    keepna: bool,
    rounding: bool,
    repair: bool,
    cache_mode: crate::client::CacheMode,
}

impl HistoryBuilder {
    pub(crate) fn new(client: YfClient, symbol: String) -> Self {
        Self {
            client,
            symbol,
            period: Some(Period::M1),
            interval: Interval::D1,
            start: None,
            end: None,
            prepost: false,
            auto_adjust: true,
            back_adjust: false,
            keepna: false,
            rounding: false,
            repair: false,
            cache_mode: crate::client::CacheMode::Use,
        }
    }

    /// Override the cache mode for this request. Effective only when the
    /// client has [`YfClientBuilder::cache_ttl`] set.
    ///
    /// [`YfClientBuilder::cache_ttl`]: crate::YfClientBuilder::cache_ttl
    pub fn cache_mode(mut self, mode: crate::client::CacheMode) -> Self {
        self.cache_mode = mode;
        self
    }

    /// Use a relative period (mutually exclusive with `start`/`end`).
    pub fn period(mut self, period: Period) -> Self {
        self.period = Some(period);
        self.start = None;
        self.end = None;
        self
    }

    /// Sampling interval. Defaults to [`Interval::D1`].
    pub fn interval(mut self, interval: Interval) -> Self {
        self.interval = interval;
        self
    }

    /// Explicit start time (UNIX seconds). Clears `period`.
    pub fn start(mut self, ts_seconds: i64) -> Self {
        self.start = Some(ts_seconds);
        self.period = None;
        self
    }

    /// Explicit end time (UNIX seconds). Clears `period`.
    pub fn end(mut self, ts_seconds: i64) -> Self {
        self.end = Some(ts_seconds);
        self.period = None;
        self
    }

    /// Convenience: `start` from a `chrono::DateTime<Utc>`.
    pub fn start_dt(self, dt: DateTime<Utc>) -> Self {
        self.start(dt.timestamp())
    }

    /// Convenience: `end` from a `chrono::DateTime<Utc>`.
    pub fn end_dt(self, dt: DateTime<Utc>) -> Self {
        self.end(dt.timestamp())
    }

    /// Include pre/post market trades.
    pub fn prepost(mut self, on: bool) -> Self {
        self.prepost = on;
        self
    }

    /// If `true` (default), divide OHLC by `(adj_close / close)` so series
    /// reflects splits and dividends.
    pub fn auto_adjust(mut self, on: bool) -> Self {
        self.auto_adjust = on;
        self
    }

    /// If `true`, multiply prices by the latest adjustment factor so the most
    /// recent close equals the latest adjusted close.
    pub fn back_adjust(mut self, on: bool) -> Self {
        self.back_adjust = on;
        self
    }

    /// Retain rows with NaN OHLC values.
    pub fn keepna(mut self, on: bool) -> Self {
        self.keepna = on;
        self
    }

    /// Round prices to 4 decimal places client-side.
    pub fn rounding(mut self, on: bool) -> Self {
        self.rounding = on;
        self
    }

    /// Run [`repair`](crate::repair) passes on the parsed history before
    /// returning it. See the [`repair`](crate::repair) module for the list
    /// of detectors.
    pub fn repair(mut self, on: bool) -> Self {
        self.repair = on;
        self
    }

    /// Execute the request.
    pub async fn fetch(self) -> Result<History> {
        let mut q: Vec<(&str, String)> = vec![
            ("interval", self.interval.as_str().to_string()),
            ("includePrePost", self.prepost.to_string()),
            ("events", "div,splits,capitalGains".to_string()),
        ];
        match (self.period, self.start, self.end) {
            (Some(p), _, _) => q.push(("range", p.as_str().to_string())),
            (None, Some(s), Some(e)) => {
                q.push(("period1", s.to_string()));
                q.push(("period2", e.to_string()));
            }
            (None, Some(s), None) => {
                q.push(("period1", s.to_string()));
                q.push(("period2", Utc::now().timestamp().to_string()));
            }
            (None, None, _) => {
                return Err(Error::invalid("history requires either period or start"));
            }
        }

        let path = format!("/v8/finance/chart/{}", YfClient::path_encode(&self.symbol));
        let raw: ChartResponse = self
            .client
            .get_json_cached(
                &path,
                &q,
                Some(("history_chart", &self.symbol)),
                self.cache_mode,
            )
            .await?;
        let mut history = parse_chart(&self.symbol, raw, &self)?;
        if self.repair {
            history.repair();
        }
        Ok(history)
    }
}

// ---------------- raw chart payload ----------------

#[derive(Debug, Deserialize)]
pub(crate) struct ChartResponse {
    pub(crate) chart: ChartContent,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChartContent {
    #[serde(default)]
    pub(crate) result: Vec<ChartResult>,
    #[serde(default)]
    pub(crate) error: Option<ChartError>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChartError {
    #[serde(default)]
    pub(crate) code: String,
    #[serde(default)]
    pub(crate) description: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChartResult {
    pub(crate) meta: ChartMeta,
    #[serde(default)]
    pub(crate) timestamp: Vec<i64>,
    #[serde(default)]
    pub(crate) indicators: Indicators,
    #[serde(default)]
    pub(crate) events: Option<Events>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChartMeta {
    #[serde(default)]
    pub(crate) symbol: Option<String>,
    #[serde(default)]
    pub(crate) currency: Option<String>,
    #[serde(rename = "exchangeTimezoneName", default)]
    pub(crate) exchange_timezone: Option<String>,
    #[serde(rename = "exchangeName", default)]
    pub(crate) exchange_name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct Indicators {
    #[serde(default)]
    pub(crate) quote: Vec<QuoteSeries>,
    #[serde(default)]
    pub(crate) adjclose: Vec<AdjCloseSeries>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct QuoteSeries {
    #[serde(default)]
    pub(crate) open: Vec<Option<f64>>,
    #[serde(default)]
    pub(crate) high: Vec<Option<f64>>,
    #[serde(default)]
    pub(crate) low: Vec<Option<f64>>,
    #[serde(default)]
    pub(crate) close: Vec<Option<f64>>,
    #[serde(default)]
    pub(crate) volume: Vec<Option<u64>>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct AdjCloseSeries {
    #[serde(default)]
    pub(crate) adjclose: Vec<Option<f64>>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct Events {
    #[serde(default)]
    pub(crate) dividends: std::collections::HashMap<String, DividendEvent>,
    #[serde(default)]
    pub(crate) splits: std::collections::HashMap<String, SplitEvent>,
    #[serde(default, rename = "capitalGains")]
    pub(crate) capital_gains: std::collections::HashMap<String, CapitalGainEvent>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DividendEvent {
    pub(crate) date: i64,
    pub(crate) amount: f64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SplitEvent {
    pub(crate) date: i64,
    pub(crate) numerator: f64,
    pub(crate) denominator: f64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CapitalGainEvent {
    pub(crate) date: i64,
    pub(crate) amount: f64,
}

pub(crate) fn parse_chart(
    symbol: &str,
    raw: ChartResponse,
    cfg: &HistoryBuilder,
) -> Result<History> {
    if let Some(err) = raw.chart.error {
        return Err(Error::Yahoo {
            symbol: symbol.to_string(),
            code: err.code,
            description: err.description,
        });
    }
    let res = raw
        .chart
        .result
        .into_iter()
        .next()
        .ok_or_else(|| Error::PricesMissing {
            ticker: symbol.to_string(),
            hint: "empty chart.result".into(),
        })?;

    let meta = res.meta;
    let quote = res.indicators.quote.into_iter().next().unwrap_or_default();
    let adj = res
        .indicators
        .adjclose
        .into_iter()
        .next()
        .unwrap_or_default();
    let events = res.events.unwrap_or_default();

    let mut rows: Vec<OhlcvRow> = Vec::with_capacity(res.timestamp.len());
    for (idx, ts) in res.timestamp.iter().enumerate() {
        let dt = Utc
            .timestamp_opt(*ts, 0)
            .single()
            .ok_or_else(|| Error::unexpected(format!("bad timestamp {}", ts)))?;
        let open = quote.open.get(idx).copied().flatten().unwrap_or(f64::NAN);
        let high = quote.high.get(idx).copied().flatten().unwrap_or(f64::NAN);
        let low = quote.low.get(idx).copied().flatten().unwrap_or(f64::NAN);
        let close = quote.close.get(idx).copied().flatten().unwrap_or(f64::NAN);
        let adj_close = adj.adjclose.get(idx).copied().flatten().unwrap_or(close);
        let volume = quote.volume.get(idx).copied().flatten().unwrap_or(0);
        rows.push(OhlcvRow {
            timestamp: dt,
            open,
            high,
            low,
            close,
            adj_close,
            volume,
            dividend: 0.0,
            split: 0.0,
            capital_gain: 0.0,
        });
    }

    if !cfg.keepna {
        rows.retain(|r| {
            !(r.open.is_nan() && r.high.is_nan() && r.low.is_nan() && r.close.is_nan())
        });
    }

    // Attach events to nearest preceding bar (matches yfinance's daily attribution).
    attach_events(&mut rows, &events);

    // auto_adjust: scale OHLC by adj_close/close.
    if cfg.auto_adjust {
        for r in rows.iter_mut() {
            if r.close > 0.0 && r.adj_close.is_finite() {
                let ratio = r.adj_close / r.close;
                r.open *= ratio;
                r.high *= ratio;
                r.low *= ratio;
                r.close = r.adj_close;
            }
        }
    }

    // back_adjust: shift series so the latest close equals the latest adj_close.
    if cfg.back_adjust {
        if let Some(last) = rows.last() {
            if last.close > 0.0 && last.adj_close.is_finite() {
                let factor = last.adj_close / last.close;
                for r in rows.iter_mut() {
                    r.open *= factor;
                    r.high *= factor;
                    r.low *= factor;
                    r.close *= factor;
                }
            }
        }
    }

    if cfg.rounding {
        for r in rows.iter_mut() {
            r.open = round4(r.open);
            r.high = round4(r.high);
            r.low = round4(r.low);
            r.close = round4(r.close);
            r.adj_close = round4(r.adj_close);
        }
    }

    let mut actions: Vec<Action> = Vec::new();
    for d in events.dividends.values() {
        let dt = Utc.timestamp_opt(d.date, 0).single();
        if let Some(date) = dt {
            actions.push(Action::Dividend {
                date,
                amount: d.amount,
            });
        }
    }
    for s in events.splits.values() {
        let dt = Utc.timestamp_opt(s.date, 0).single();
        if let Some(date) = dt {
            actions.push(Action::Split {
                date,
                numerator: s.numerator,
                denominator: s.denominator,
            });
        }
    }
    for c in events.capital_gains.values() {
        let dt = Utc.timestamp_opt(c.date, 0).single();
        if let Some(date) = dt {
            actions.push(Action::CapitalGain {
                date,
                amount: c.amount,
            });
        }
    }
    actions.sort_by_key(|a| match a {
        Action::Dividend { date, .. }
        | Action::Split { date, .. }
        | Action::CapitalGain { date, .. } => *date,
    });

    Ok(History {
        symbol: meta.symbol.unwrap_or_else(|| symbol.to_string()),
        currency: meta.currency,
        timezone: meta.exchange_timezone,
        exchange_name: meta.exchange_name,
        rows,
        actions,
    })
}

fn attach_events(rows: &mut [OhlcvRow], events: &Events) {
    if events.dividends.is_empty() && events.splits.is_empty() && events.capital_gains.is_empty() {
        return;
    }
    let nearest_idx = |ts: i64, rows: &[OhlcvRow]| -> Option<usize> {
        rows.iter()
            .enumerate()
            .min_by_key(|(_, r)| (r.timestamp.timestamp() - ts).abs())
            .map(|(i, _)| i)
    };
    for d in events.dividends.values() {
        if let Some(i) = nearest_idx(d.date, rows) {
            rows[i].dividend += d.amount;
        }
    }
    for s in events.splits.values() {
        if let Some(i) = nearest_idx(s.date, rows) {
            if s.denominator > 0.0 {
                rows[i].split = s.numerator / s.denominator;
            }
        }
    }
    for c in events.capital_gains.values() {
        if let Some(i) = nearest_idx(c.date, rows) {
            rows[i].capital_gain += c.amount;
        }
    }
}

fn round4(x: f64) -> f64 {
    if x.is_finite() {
        (x * 10_000.0).round() / 10_000.0
    } else {
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(symbol: &str) -> ChartResponse {
        let json = serde_json::json!({
            "chart": {
                "result": [{
                    "meta": {
                        "symbol": symbol,
                        "currency": "USD",
                        "exchangeName": "NMS",
                        "exchangeTimezoneName": "America/New_York"
                    },
                    "timestamp": [1_700_000_000_i64, 1_700_086_400_i64],
                    "indicators": {
                        "quote": [{
                            "open":   [100.0, 102.0],
                            "high":   [101.5, 103.5],
                            "low":    [ 99.5, 101.0],
                            "close":  [101.0, 103.0],
                            "volume": [1000000, 1500000]
                        }],
                        "adjclose": [{ "adjclose": [101.0, 103.0] }]
                    },
                    "events": {
                        "dividends": {
                            "1700000000": { "date": 1_700_000_000_i64, "amount": 0.25 }
                        }
                    }
                }],
                "error": null
            }
        });
        serde_json::from_value(json).unwrap()
    }

    fn default_cfg() -> HistoryBuilder {
        let client = YfClient::new().expect("client");
        HistoryBuilder::new(client, "TEST".into())
    }

    #[test]
    fn parses_basic_chart() {
        let raw = fixture("AAPL");
        let h = parse_chart("AAPL", raw, &default_cfg().auto_adjust(false)).unwrap();
        assert_eq!(h.symbol, "AAPL");
        assert_eq!(h.rows.len(), 2);
        assert_eq!(h.rows[0].open, 100.0);
        assert!((h.rows[0].dividend - 0.25).abs() < 1e-9);
        assert_eq!(h.actions.len(), 1);
    }

    #[test]
    fn auto_adjust_scales_ohlc() {
        let mut payload = fixture("AAPL");
        payload.chart.result[0].indicators.adjclose[0].adjclose = vec![Some(50.5), Some(51.5)];
        let h = parse_chart("AAPL", payload, &default_cfg().auto_adjust(true)).unwrap();
        // close was 101 -> 50.5 (factor 0.5). open should also halve.
        assert!((h.rows[0].open - 50.0).abs() < 1e-6);
        assert!((h.rows[0].close - 50.5).abs() < 1e-6);
    }

    #[test]
    fn yahoo_error_propagates() {
        let json = serde_json::json!({
            "chart": {
                "result": [],
                "error": { "code": "Not Found", "description": "No data found, symbol may be delisted" }
            }
        });
        let raw: ChartResponse = serde_json::from_value(json).unwrap();
        let e = parse_chart("BOGUS", raw, &default_cfg()).unwrap_err();
        assert!(matches!(e, Error::Yahoo { .. }));
    }
}
