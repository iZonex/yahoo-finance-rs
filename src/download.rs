//! Multi-ticker batch download with bounded concurrency.
//!
//! Equivalent to `yfinance.download(...)` in the Python library.

use std::collections::HashMap;
use std::sync::Arc;

use futures::stream::{FuturesUnordered, StreamExt};
use tokio::sync::Semaphore;

use crate::client::YfClient;
use crate::error::{Error, Result};
use crate::history::{History, HistoryBuilder};
use crate::ticker::Ticker;
use crate::types::{Interval, Period};

/// Result of a multi-ticker download.
#[derive(Debug, Clone)]
pub struct MultiHistory {
    /// Per-symbol history. Symbols that failed appear in [`Self::errors`].
    pub series: HashMap<String, History>,
    /// Per-symbol errors.
    pub errors: HashMap<String, String>,
}

/// Builder for [`download`].
#[derive(Debug)]
pub struct DownloadBuilder {
    client: YfClient,
    tickers: Vec<String>,
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
    concurrency: usize,
}

impl DownloadBuilder {
    /// Construct a new builder.
    pub fn new(client: &YfClient, tickers: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            client: client.clone(),
            tickers: tickers
                .into_iter()
                .map(|s| s.into().trim().to_uppercase())
                .filter(|s| !s.is_empty())
                .collect(),
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
            concurrency: 8,
        }
    }

    /// Use a relative period.
    pub fn period(mut self, period: Period) -> Self {
        self.period = Some(period);
        self.start = None;
        self.end = None;
        self
    }

    /// Sampling interval.
    pub fn interval(mut self, interval: Interval) -> Self {
        self.interval = interval;
        self
    }

    /// Explicit start (UNIX seconds). Clears `period`.
    pub fn start(mut self, ts_seconds: i64) -> Self {
        self.start = Some(ts_seconds);
        self.period = None;
        self
    }

    /// Explicit end (UNIX seconds). Clears `period`.
    pub fn end(mut self, ts_seconds: i64) -> Self {
        self.end = Some(ts_seconds);
        self.period = None;
        self
    }

    /// Toggle pre/post market.
    pub fn prepost(mut self, on: bool) -> Self {
        self.prepost = on;
        self
    }

    /// Auto-adjust OHLC by adj close (default `true`).
    pub fn auto_adjust(mut self, on: bool) -> Self {
        self.auto_adjust = on;
        self
    }

    /// Back-adjust historical prices to the latest split/dividend factor.
    pub fn back_adjust(mut self, on: bool) -> Self {
        self.back_adjust = on;
        self
    }

    /// Keep NaN rows.
    pub fn keepna(mut self, on: bool) -> Self {
        self.keepna = on;
        self
    }

    /// Round prices to 4 decimals client-side.
    pub fn rounding(mut self, on: bool) -> Self {
        self.rounding = on;
        self
    }

    /// Run [`repair`](crate::repair) passes on every symbol's history.
    pub fn repair(mut self, on: bool) -> Self {
        self.repair = on;
        self
    }

    /// Maximum concurrent in-flight requests (default 8). Yahoo doesn't publish
    /// a precise rate limit; staying ≤ 16 is a good rule of thumb.
    pub fn concurrency(mut self, n: usize) -> Self {
        self.concurrency = n.max(1);
        self
    }

    /// Run the batch download.
    pub async fn fetch(self) -> Result<MultiHistory> {
        if self.tickers.is_empty() {
            return Err(Error::invalid("download: empty ticker list"));
        }
        let sem = Arc::new(Semaphore::new(self.concurrency));
        let mut futs = FuturesUnordered::new();

        for sym in self.tickers.iter().cloned() {
            let permit_owner = sem.clone();
            let client = self.client.clone();
            let cfg = self.clone_for_one();
            futs.push(async move {
                let _permit = permit_owner.acquire_owned().await.unwrap();
                let t = Ticker::new(&client, &sym);
                let mut b: HistoryBuilder = t.history();
                if let Some(p) = cfg.period {
                    b = b.period(p);
                }
                if let Some(s) = cfg.start {
                    b = b.start(s);
                }
                if let Some(e) = cfg.end {
                    b = b.end(e);
                }
                b = b
                    .interval(cfg.interval)
                    .prepost(cfg.prepost)
                    .auto_adjust(cfg.auto_adjust)
                    .back_adjust(cfg.back_adjust)
                    .keepna(cfg.keepna)
                    .rounding(cfg.rounding)
                    .repair(cfg.repair);
                (sym, b.fetch().await)
            });
        }

        let mut series: HashMap<String, History> = HashMap::new();
        let mut errors: HashMap<String, String> = HashMap::new();
        while let Some((sym, res)) = futs.next().await {
            match res {
                Ok(h) => {
                    series.insert(sym, h);
                }
                Err(e) => {
                    errors.insert(sym, e.to_string());
                }
            }
        }
        Ok(MultiHistory { series, errors })
    }

    fn clone_for_one(&self) -> DownloadConfig {
        DownloadConfig {
            period: self.period,
            interval: self.interval,
            start: self.start,
            end: self.end,
            prepost: self.prepost,
            auto_adjust: self.auto_adjust,
            back_adjust: self.back_adjust,
            keepna: self.keepna,
            rounding: self.rounding,
            repair: self.repair,
        }
    }
}

#[derive(Clone)]
struct DownloadConfig {
    period: Option<Period>,
    interval: Interval,
    start: Option<i64>,
    end: Option<i64>,
    prepost: bool,
    auto_adjust: bool,
    back_adjust: bool,
    keepna: bool,
    repair: bool,
    rounding: bool,
}

/// Convenience entry point — equivalent to constructing a [`DownloadBuilder`]
/// with defaults and calling `.fetch()`.
pub async fn download<I, S>(client: &YfClient, tickers: I) -> Result<MultiHistory>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    DownloadBuilder::new(client, tickers).fetch().await
}
