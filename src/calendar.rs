//! Calendar events: next earnings dates, ex-dividend, dividend-payment date.
//!
//! Backed by the `calendarEvents` quoteSummary module. Yahoo returns dates as
//! UNIX seconds; this module converts to `DateTime<Utc>` and exposes them
//! flat.

use chrono::{DateTime, TimeZone, Utc};
use serde::Deserialize;

use crate::client::YfClient;
use crate::error::Result;
use crate::wire::RawNum;

/// Upcoming corporate-event dates for a symbol.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Calendar {
    /// Earnings announcement window (1–2 dates: confirmed and/or estimated).
    pub earnings_dates: Vec<DateTime<Utc>>,
    /// Last day to be a shareholder of record for the next dividend.
    pub ex_dividend_date: Option<DateTime<Utc>>,
    /// Date the next dividend is paid out.
    pub dividend_date: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
struct CalendarEventsNode {
    #[serde(default)]
    earnings: Option<EarningsBlock>,
    #[serde(default, rename = "exDividendDate")]
    ex_dividend_date: Option<RawNum<i64>>,
    #[serde(default, rename = "dividendDate")]
    dividend_date: Option<RawNum<i64>>,
}

#[derive(Deserialize)]
struct EarningsBlock {
    #[serde(default, rename = "earningsDate")]
    earnings_date: Option<Vec<RawNum<i64>>>,
}

fn ts(r: Option<RawNum<i64>>) -> Option<DateTime<Utc>> {
    r.and_then(|x| x.raw)
        .and_then(|s| Utc.timestamp_opt(s, 0).single())
}

pub(crate) async fn fetch(client: &YfClient, symbol: &str) -> Result<Calendar> {
    let Some(modules) = client
        .fetch_quote_summary(symbol, "calendarEvents", "calendar_quoteSummary")
        .await?
    else {
        return Ok(Calendar::default());
    };
    let Some(node) = modules.get("calendarEvents").cloned() else {
        return Ok(Calendar::default());
    };
    let parsed: CalendarEventsNode = serde_json::from_value(node).unwrap_or(CalendarEventsNode {
        earnings: None,
        ex_dividend_date: None,
        dividend_date: None,
    });
    Ok(Calendar {
        earnings_dates: parsed
            .earnings
            .and_then(|e| e.earnings_date)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|d| ts(Some(d)))
            .collect(),
        ex_dividend_date: ts(parsed.ex_dividend_date),
        dividend_date: ts(parsed.dividend_date),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_calendar_events() {
        let n: CalendarEventsNode = serde_json::from_value(serde_json::json!({
            "earnings": {
                "earningsDate": [
                    { "raw": 1_700_000_000_i64 },
                    { "raw": 1_700_086_400_i64 }
                ]
            },
            "exDividendDate": { "raw": 1_699_000_000_i64 },
            "dividendDate": { "raw": 1_701_000_000_i64 }
        }))
        .unwrap();
        let dates = n.earnings.unwrap().earnings_date.unwrap();
        assert_eq!(dates.len(), 2);
        assert_eq!(dates[0].raw, Some(1_700_000_000));
    }
}
