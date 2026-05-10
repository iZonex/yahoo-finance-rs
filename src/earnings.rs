//! Earnings — actual vs estimated EPS by quarter, plus yearly/quarterly
//! revenue and earnings totals.
//!
//! Backed by the `earnings` quoteSummary module (`financialsChart` +
//! `earningsChart`).

use serde::Deserialize;

use crate::client::YfClient;
use crate::error::Result;
use crate::wire::{from_raw_f64, RawNum};

/// One row of yearly revenue + net earnings (`financialsChart.yearly`).
#[derive(Debug, Clone, PartialEq)]
pub struct EarningsYear {
    /// Fiscal year (Yahoo returns this as `i64` already).
    pub year: i64,
    /// Total revenue for the year.
    pub revenue: Option<f64>,
    /// Net earnings for the year.
    pub earnings: Option<f64>,
}

/// One row of quarterly revenue + net earnings (`financialsChart.quarterly`).
#[derive(Debug, Clone, PartialEq)]
pub struct EarningsQuarter {
    /// Period label (`"1Q2024"`, …).
    pub period: String,
    /// Total revenue for the quarter.
    pub revenue: Option<f64>,
    /// Net earnings for the quarter.
    pub earnings: Option<f64>,
}

/// One row of EPS estimate vs actual (`earningsChart.quarterly`).
#[derive(Debug, Clone, PartialEq)]
pub struct EarningsEps {
    /// Period label (`"3Q2024"`, …).
    pub period: String,
    /// Reported (actual) EPS.
    pub actual: Option<f64>,
    /// Analyst consensus estimate going into the report.
    pub estimate: Option<f64>,
}

/// Combined `earnings` quoteSummary payload.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Earnings {
    /// Symbol the data was fetched for.
    pub symbol: String,
    /// Yearly totals.
    pub yearly: Vec<EarningsYear>,
    /// Quarterly totals (revenue + net earnings).
    pub quarterly: Vec<EarningsQuarter>,
    /// Quarterly EPS — estimate vs reported.
    pub eps_quarterly: Vec<EarningsEps>,
}

#[derive(Deserialize)]
struct EarningsNode {
    #[serde(default, rename = "financialsChart")]
    financials_chart: Option<FinancialsChartNode>,
    #[serde(default, rename = "earningsChart")]
    earnings_chart: Option<EarningsChartNode>,
}

#[derive(Deserialize)]
struct FinancialsChartNode {
    #[serde(default)]
    yearly: Option<Vec<FinancialYearNode>>,
    #[serde(default)]
    quarterly: Option<Vec<FinancialQuarterNode>>,
}

#[derive(Deserialize)]
struct FinancialYearNode {
    #[serde(default)]
    date: Option<i64>,
    #[serde(default)]
    revenue: Option<RawNum<f64>>,
    #[serde(default)]
    earnings: Option<RawNum<f64>>,
}

#[derive(Deserialize)]
struct FinancialQuarterNode {
    #[serde(default)]
    date: Option<String>,
    #[serde(default)]
    revenue: Option<RawNum<f64>>,
    #[serde(default)]
    earnings: Option<RawNum<f64>>,
}

#[derive(Deserialize)]
struct EarningsChartNode {
    #[serde(default)]
    quarterly: Option<Vec<EpsQuarterNode>>,
}

#[derive(Deserialize)]
struct EpsQuarterNode {
    #[serde(default)]
    date: Option<String>,
    #[serde(default)]
    actual: Option<RawNum<f64>>,
    #[serde(default)]
    estimate: Option<RawNum<f64>>,
}

pub(crate) async fn fetch(client: &YfClient, symbol: &str) -> Result<Earnings> {
    let Some(modules) = client
        .fetch_quote_summary(symbol, "earnings", "earnings_quoteSummary")
        .await?
    else {
        return Ok(Earnings {
            symbol: symbol.to_string(),
            ..Earnings::default()
        });
    };
    let Some(node) = modules.get("earnings").cloned() else {
        return Ok(Earnings {
            symbol: symbol.to_string(),
            ..Earnings::default()
        });
    };
    let parsed: EarningsNode = serde_json::from_value(node).unwrap_or(EarningsNode {
        financials_chart: None,
        earnings_chart: None,
    });

    let (yearly, quarterly) = parsed.financials_chart.map_or_else(
        || (vec![], vec![]),
        |fc| {
            let yearly = fc
                .yearly
                .unwrap_or_default()
                .into_iter()
                .map(|y| EarningsYear {
                    year: y.date.unwrap_or_default(),
                    revenue: from_raw_f64(y.revenue),
                    earnings: from_raw_f64(y.earnings),
                })
                .collect();
            let quarterly = fc
                .quarterly
                .unwrap_or_default()
                .into_iter()
                .map(|q| EarningsQuarter {
                    period: q.date.unwrap_or_default(),
                    revenue: from_raw_f64(q.revenue),
                    earnings: from_raw_f64(q.earnings),
                })
                .collect();
            (yearly, quarterly)
        },
    );

    let eps_quarterly = parsed
        .earnings_chart
        .and_then(|ec| ec.quarterly)
        .unwrap_or_default()
        .into_iter()
        .map(|q| EarningsEps {
            period: q.date.unwrap_or_default(),
            actual: from_raw_f64(q.actual),
            estimate: from_raw_f64(q.estimate),
        })
        .collect();

    Ok(Earnings {
        symbol: symbol.to_string(),
        yearly,
        quarterly,
        eps_quarterly,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_payload() {
        let node: EarningsNode = serde_json::from_value(serde_json::json!({
            "financialsChart": {
                "yearly": [
                    { "date": 2023, "revenue": { "raw": 1.0e11 }, "earnings": { "raw": 2.0e10 } }
                ],
                "quarterly": [
                    { "date": "1Q2024", "revenue": { "raw": 2.5e10 }, "earnings": { "raw": 5.0e9 } }
                ]
            },
            "earningsChart": {
                "quarterly": [
                    { "date": "3Q2024", "actual": { "raw": 1.5 }, "estimate": { "raw": 1.4 } }
                ]
            }
        }))
        .unwrap();
        let yearly = node.financials_chart.unwrap().yearly.unwrap();
        assert_eq!(yearly[0].date, Some(2023));
    }
}
