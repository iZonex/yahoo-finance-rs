//! Polars `DataFrame` conversions (`dataframe` feature).
//!
//! Provides the [`ToDataFrame`] trait and implementations for the main
//! data-bearing types in the crate. Designed for downstream analytical work —
//! the core API stays plain Rust structs and only opts in to `polars` when the
//! feature is enabled.
//!
//! ```no_run
//! # #[cfg(feature = "dataframe")]
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! use yfinance::{Ticker, YfClient, dataframe::ToDataFrame};
//!
//! let client = YfClient::new()?;
//! let history = Ticker::new(&client, "AAPL").history().fetch().await?;
//! let df = history.to_dataframe()?;
//! println!("{df}");
//! # Ok(()) }
//! ```

use polars::prelude::*;

use crate::analysis::{RecommendationRow, UpgradeDowngradeRow};
use crate::download::MultiHistory;
use crate::history::History;
use crate::holders::{HolderRow, InsiderRosterRow, InsiderTransaction, MajorHolderRow};
use crate::news::NewsArticle;
use crate::quote::FastInfo;

/// Convert a value to a Polars [`DataFrame`].
pub trait ToDataFrame {
    /// Lay out `&self` as a `DataFrame`. Implementations are deterministic in
    /// column names and order.
    fn to_dataframe(&self) -> PolarsResult<DataFrame>;
}

/// Build a UTC `Datetime(Milliseconds)` column from millisecond timestamps.
fn datetime_col(name: &str, ms: Vec<Option<i64>>) -> PolarsResult<Column> {
    let s = Series::new(name.into(), ms);
    Ok(s.cast(&DataType::Datetime(TimeUnit::Milliseconds, None))?
        .with_name(name.into())
        .into())
}

impl ToDataFrame for History {
    fn to_dataframe(&self) -> PolarsResult<DataFrame> {
        let n = self.rows.len();
        let mut ts = Vec::with_capacity(n);
        let mut open = Vec::with_capacity(n);
        let mut high = Vec::with_capacity(n);
        let mut low = Vec::with_capacity(n);
        let mut close = Vec::with_capacity(n);
        let mut adj_close = Vec::with_capacity(n);
        let mut volume = Vec::with_capacity(n);
        let mut dividend = Vec::with_capacity(n);
        let mut split = Vec::with_capacity(n);
        let mut capital_gain = Vec::with_capacity(n);

        for r in &self.rows {
            ts.push(Some(r.timestamp.timestamp_millis()));
            open.push(r.open);
            high.push(r.high);
            low.push(r.low);
            close.push(r.close);
            adj_close.push(r.adj_close);
            // u64 → i64 saturating; Yahoo volumes never overflow i64 in practice.
            volume.push(i64::try_from(r.volume).unwrap_or(i64::MAX));
            dividend.push(r.dividend);
            split.push(r.split);
            capital_gain.push(r.capital_gain);
        }

        DataFrame::new(vec![
            datetime_col("timestamp", ts)?,
            Column::new("open".into(), open),
            Column::new("high".into(), high),
            Column::new("low".into(), low),
            Column::new("close".into(), close),
            Column::new("adj_close".into(), adj_close),
            Column::new("volume".into(), volume),
            Column::new("dividend".into(), dividend),
            Column::new("split".into(), split),
            Column::new("capital_gain".into(), capital_gain),
        ])
    }
}

impl ToDataFrame for MultiHistory {
    /// Concatenate every per-symbol [`History`] vertically with a `symbol`
    /// column added on the left.
    fn to_dataframe(&self) -> PolarsResult<DataFrame> {
        if self.series.is_empty() {
            return DataFrame::new(vec![Column::new_empty("symbol".into(), &DataType::String)]);
        }
        let mut frames: Vec<DataFrame> = Vec::with_capacity(self.series.len());
        // Sort symbols for deterministic output.
        let mut symbols: Vec<&String> = self.series.keys().collect();
        symbols.sort();
        for sym in symbols {
            let h = &self.series[sym];
            let mut df = h.to_dataframe()?;
            let n = df.height();
            let symbol_col = Column::new("symbol".into(), vec![sym.clone(); n]);
            df.insert_column(0, symbol_col)?;
            frames.push(df);
        }
        let mut acc = frames.remove(0);
        for f in frames {
            acc.vstack_mut(&f)?;
        }
        acc.align_chunks_par();
        Ok(acc)
    }
}

impl ToDataFrame for Vec<RecommendationRow> {
    fn to_dataframe(&self) -> PolarsResult<DataFrame> {
        let period: Vec<String> = self.iter().map(|r| r.period.clone()).collect();
        // u32 → i64 (Polars default features only carry i64/f64 for numerics).
        let strong_buy: Vec<Option<i64>> =
            self.iter().map(|r| r.strong_buy.map(i64::from)).collect();
        let buy: Vec<Option<i64>> = self.iter().map(|r| r.buy.map(i64::from)).collect();
        let hold: Vec<Option<i64>> = self.iter().map(|r| r.hold.map(i64::from)).collect();
        let sell: Vec<Option<i64>> = self.iter().map(|r| r.sell.map(i64::from)).collect();
        let strong_sell: Vec<Option<i64>> =
            self.iter().map(|r| r.strong_sell.map(i64::from)).collect();
        DataFrame::new(vec![
            Column::new("period".into(), period),
            Column::new("strong_buy".into(), strong_buy),
            Column::new("buy".into(), buy),
            Column::new("hold".into(), hold),
            Column::new("sell".into(), sell),
            Column::new("strong_sell".into(), strong_sell),
        ])
    }
}

impl ToDataFrame for Vec<UpgradeDowngradeRow> {
    fn to_dataframe(&self) -> PolarsResult<DataFrame> {
        let ts: Vec<Option<i64>> = self.iter().map(|r| Some(r.timestamp * 1000)).collect();
        let firm: Vec<Option<String>> = self.iter().map(|r| r.firm.clone()).collect();
        let from_grade: Vec<Option<String>> = self.iter().map(|r| r.from_grade.clone()).collect();
        let to_grade: Vec<Option<String>> = self.iter().map(|r| r.to_grade.clone()).collect();
        let action: Vec<Option<String>> = self.iter().map(|r| r.action.clone()).collect();

        DataFrame::new(vec![
            datetime_col("timestamp", ts)?,
            Column::new("firm".into(), firm),
            Column::new("from_grade".into(), from_grade),
            Column::new("to_grade".into(), to_grade),
            Column::new("action".into(), action),
        ])
    }
}

impl ToDataFrame for Vec<NewsArticle> {
    fn to_dataframe(&self) -> PolarsResult<DataFrame> {
        let uuid: Vec<String> = self.iter().map(|a| a.uuid.clone()).collect();
        let title: Vec<String> = self.iter().map(|a| a.title.clone()).collect();
        let publisher: Vec<Option<String>> = self.iter().map(|a| a.publisher.clone()).collect();
        let link: Vec<Option<String>> = self.iter().map(|a| a.link.clone()).collect();
        let published_at_ms: Vec<Option<i64>> = self
            .iter()
            .map(|a| a.published_at.map(|s| s * 1000))
            .collect();

        DataFrame::new(vec![
            Column::new("uuid".into(), uuid),
            Column::new("title".into(), title),
            Column::new("publisher".into(), publisher),
            Column::new("link".into(), link),
            datetime_col("published_at", published_at_ms)?,
        ])
    }
}

impl ToDataFrame for FastInfo {
    /// One-row DataFrame snapshot — handy for joining with other quotes.
    fn to_dataframe(&self) -> PolarsResult<DataFrame> {
        DataFrame::new(vec![
            Column::new("symbol".into(), vec![self.symbol.clone()]),
            Column::new("currency".into(), vec![self.currency.clone()]),
            Column::new("exchange".into(), vec![self.exchange.clone()]),
            Column::new("last_price".into(), vec![self.last_price]),
            Column::new(
                "last_volume".into(),
                vec![self.last_volume.and_then(|v| i64::try_from(v).ok())],
            ),
            Column::new("previous_close".into(), vec![self.previous_close]),
            Column::new("open".into(), vec![self.open]),
            Column::new("day_high".into(), vec![self.day_high]),
            Column::new("day_low".into(), vec![self.day_low]),
        ])
    }
}

fn u64_to_i64_opt(v: Option<u64>) -> Option<i64> {
    v.and_then(|x| i64::try_from(x).ok())
}

impl ToDataFrame for Vec<MajorHolderRow> {
    fn to_dataframe(&self) -> PolarsResult<DataFrame> {
        let label: Vec<String> = self.iter().map(|r| r.label.clone()).collect();
        let percent: Vec<f64> = self.iter().map(|r| r.percent).collect();
        DataFrame::new(vec![
            Column::new("label".into(), label),
            Column::new("percent".into(), percent),
        ])
    }
}

impl ToDataFrame for Vec<HolderRow> {
    fn to_dataframe(&self) -> PolarsResult<DataFrame> {
        let holder: Vec<String> = self.iter().map(|r| r.holder.clone()).collect();
        let report_date: Vec<Option<String>> = self.iter().map(|r| r.report_date.clone()).collect();
        let shares: Vec<Option<i64>> = self.iter().map(|r| u64_to_i64_opt(r.shares)).collect();
        let value: Vec<Option<i64>> = self.iter().map(|r| u64_to_i64_opt(r.value)).collect();
        let percent_held: Vec<Option<f64>> = self.iter().map(|r| r.percent_held).collect();

        DataFrame::new(vec![
            Column::new("holder".into(), holder),
            Column::new("report_date".into(), report_date),
            Column::new("shares".into(), shares),
            Column::new("value".into(), value),
            Column::new("percent_held".into(), percent_held),
        ])
    }
}

impl ToDataFrame for Vec<InsiderTransaction> {
    fn to_dataframe(&self) -> PolarsResult<DataFrame> {
        let name: Vec<String> = self.iter().map(|t| t.name.clone()).collect();
        let relation: Vec<Option<String>> = self.iter().map(|t| t.relation.clone()).collect();
        let start_date: Vec<Option<String>> = self.iter().map(|t| t.start_date.clone()).collect();
        let transaction: Vec<Option<String>> = self.iter().map(|t| t.transaction.clone()).collect();
        let ownership: Vec<Option<String>> = self.iter().map(|t| t.ownership.clone()).collect();
        let shares: Vec<Option<i64>> = self.iter().map(|t| u64_to_i64_opt(t.shares)).collect();
        let value: Vec<Option<i64>> = self.iter().map(|t| u64_to_i64_opt(t.value)).collect();

        DataFrame::new(vec![
            Column::new("name".into(), name),
            Column::new("relation".into(), relation),
            Column::new("start_date".into(), start_date),
            Column::new("transaction".into(), transaction),
            Column::new("ownership".into(), ownership),
            Column::new("shares".into(), shares),
            Column::new("value".into(), value),
        ])
    }
}

impl ToDataFrame for Vec<InsiderRosterRow> {
    fn to_dataframe(&self) -> PolarsResult<DataFrame> {
        let name: Vec<String> = self.iter().map(|r| r.name.clone()).collect();
        let relation: Vec<Option<String>> = self.iter().map(|r| r.relation.clone()).collect();
        let direct_shares: Vec<Option<i64>> = self
            .iter()
            .map(|r| u64_to_i64_opt(r.direct_shares))
            .collect();
        let position_date: Vec<Option<String>> =
            self.iter().map(|r| r.position_date.clone()).collect();

        DataFrame::new(vec![
            Column::new("name".into(), name),
            Column::new("relation".into(), relation),
            Column::new("direct_shares".into(), direct_shares),
            Column::new("position_date".into(), position_date),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample_history() -> History {
        let ts1 = chrono::Utc.timestamp_opt(1_700_000_000, 0).unwrap();
        let ts2 = chrono::Utc.timestamp_opt(1_700_086_400, 0).unwrap();
        History {
            symbol: "AAPL".into(),
            currency: Some("USD".into()),
            timezone: Some("America/New_York".into()),
            exchange_name: Some("NMS".into()),
            rows: vec![
                crate::history::OhlcvRow {
                    timestamp: ts1,
                    open: 100.0,
                    high: 101.0,
                    low: 99.0,
                    close: 100.5,
                    adj_close: 100.5,
                    volume: 1_000_000,
                    dividend: 0.0,
                    split: 0.0,
                    capital_gain: 0.0,
                },
                crate::history::OhlcvRow {
                    timestamp: ts2,
                    open: 100.5,
                    high: 102.0,
                    low: 100.0,
                    close: 101.5,
                    adj_close: 101.5,
                    volume: 1_500_000,
                    dividend: 0.25,
                    split: 0.0,
                    capital_gain: 0.0,
                },
            ],
            actions: vec![],
            first_trade_date: None,
        }
    }

    #[test]
    fn history_dataframe_shape_and_columns() {
        let df = sample_history().to_dataframe().unwrap();
        assert_eq!(df.shape(), (2, 10));
        let names: Vec<&str> = df.get_column_names_str();
        assert_eq!(
            names,
            vec![
                "timestamp",
                "open",
                "high",
                "low",
                "close",
                "adj_close",
                "volume",
                "dividend",
                "split",
                "capital_gain",
            ]
        );
    }

    #[test]
    fn multi_history_prepends_symbol_column() {
        let mut series = std::collections::HashMap::new();
        series.insert("AAPL".to_string(), sample_history());
        let multi = MultiHistory {
            series,
            errors: std::collections::HashMap::new(),
        };
        let df = multi.to_dataframe().unwrap();
        assert_eq!(df.shape(), (2, 11));
        let names = df.get_column_names_str();
        assert_eq!(names[0], "symbol");
    }

    #[test]
    fn recommendations_dataframe() {
        let rows = vec![
            RecommendationRow {
                period: "0m".into(),
                strong_buy: Some(5),
                buy: Some(10),
                hold: Some(2),
                sell: Some(0),
                strong_sell: Some(0),
            },
            RecommendationRow {
                period: "-1m".into(),
                strong_buy: Some(4),
                buy: Some(11),
                hold: Some(2),
                sell: Some(0),
                strong_sell: Some(0),
            },
        ];
        let df = rows.to_dataframe().unwrap();
        assert_eq!(df.shape(), (2, 6));
    }
}
