//! Historical shares outstanding (annual or quarterly).
//!
//! Backed by the `fundamentals-timeseries` endpoint with
//! `type=annualBasicAverageShares` or `quarterlyBasicAverageShares`.

use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::Value;

use crate::client::YfClient;
use crate::error::{Error, Result};

/// One reported share count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareCount {
    /// As-of date (YYYY-MM-DD).
    pub date: NaiveDate,
    /// Reported share count.
    pub shares: u64,
}

#[derive(Deserialize)]
struct TimeseriesEnvelope {
    timeseries: TimeseriesContent,
}

#[derive(Deserialize)]
struct TimeseriesContent {
    #[serde(default)]
    result: Vec<serde_json::Map<String, Value>>,
    #[serde(default)]
    error: Option<Value>,
}

pub(crate) async fn fetch(
    client: &YfClient,
    symbol: &str,
    quarterly: bool,
) -> Result<Vec<ShareCount>> {
    let type_key = if quarterly {
        "quarterlyBasicAverageShares"
    } else {
        "annualBasicAverageShares"
    };

    let path = format!(
        "/ws/fundamentals-timeseries/v1/finance/timeseries/{}",
        YfClient::path_encode(symbol)
    );
    // 18-month window matches what Yahoo's UI hits.
    let end = chrono::Utc::now().timestamp();
    let start = end - 60 * 60 * 24 * 548;
    let q = vec![
        ("symbol", symbol.to_string()),
        ("type", type_key.to_string()),
        ("period1", start.to_string()),
        ("period2", end.to_string()),
    ];

    let label = format!("shares_{type_key}");
    let env: TimeseriesEnvelope = client
        .get_json_crumb(&path, &q, Some((&label, symbol)))
        .await?;
    if let Some(err) = env.timeseries.error {
        return Err(Error::Yahoo {
            symbol: symbol.to_string(),
            code: format!("{type_key}_error"),
            description: err.to_string(),
        });
    }

    let Some(entry) = env.timeseries.result.into_iter().next() else {
        return Ok(Vec::new());
    };
    let Some(values) = entry.get(type_key).and_then(|v| v.as_array()) else {
        return Ok(Vec::new());
    };

    let rows = values
        .iter()
        .filter_map(|item| {
            let obj = item.as_object()?;
            let as_of = obj.get("asOfDate").and_then(|x| x.as_str())?;
            let date = NaiveDate::parse_from_str(as_of, "%Y-%m-%d").ok()?;
            let shares = obj
                .get("reportedValue")
                .and_then(|rv| rv.get("raw"))
                .and_then(|x| x.as_u64())?;
            Some(ShareCount { date, shares })
        })
        .collect();
    Ok(rows)
}
