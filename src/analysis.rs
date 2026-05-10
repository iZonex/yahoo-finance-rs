//! Analyst data: recommendations, price targets, upgrades/downgrades, earnings trend.
//!
//! Backed by `quoteSummary` modules `recommendationTrend`, `financialData`,
//! `upgradeDowngradeHistory`, and `earningsTrend`. Each public method on
//! [`Ticker`] requests just the modules it needs.
//!
//! [`Ticker`]: crate::ticker::Ticker

use serde::Deserialize;
use serde_json::Value;

use crate::client::YfClient;
use crate::error::{Error, Result};
use crate::wire::{
    from_raw_f64 as raw_f64, from_raw_i64 as raw_i64, from_raw_u32 as raw_u32, RawNum,
};

/// One row from `recommendationTrend.trend`. Periods are Yahoo's relative
/// labels (`"0m"`, `"-1m"`, `"-2m"`, `"-3m"`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecommendationRow {
    /// Period label as returned by Yahoo (e.g. `"0m"`).
    pub period: String,
    /// Strong-buy ratings count.
    pub strong_buy: Option<u32>,
    /// Buy ratings count.
    pub buy: Option<u32>,
    /// Hold ratings count.
    pub hold: Option<u32>,
    /// Sell ratings count.
    pub sell: Option<u32>,
    /// Strong-sell ratings count.
    pub strong_sell: Option<u32>,
}

/// Compact summary combining the latest recommendation period and Yahoo's
/// numeric `recommendationMean` / `recommendationKey`.
#[derive(Debug, Clone, PartialEq)]
pub struct RecommendationSummary {
    /// Latest period covered (typically `"0m"`).
    pub latest_period: Option<String>,
    /// Strong-buy count for the latest period.
    pub strong_buy: Option<u32>,
    /// Buy count.
    pub buy: Option<u32>,
    /// Hold count.
    pub hold: Option<u32>,
    /// Sell count.
    pub sell: Option<u32>,
    /// Strong-sell count.
    pub strong_sell: Option<u32>,
    /// Numeric recommendation mean (1.0 = strong buy, 5.0 = strong sell).
    pub mean: Option<f64>,
    /// Yahoo's text rating (`"buy"`, `"hold"`, …).
    pub rating: Option<String>,
}

/// One row from `upgradeDowngradeHistory.history`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeDowngradeRow {
    /// Event timestamp (UNIX seconds).
    pub timestamp: i64,
    /// Issuing firm.
    pub firm: Option<String>,
    /// Previous grade.
    pub from_grade: Option<String>,
    /// New grade.
    pub to_grade: Option<String>,
    /// Action (`"up"`, `"down"`, `"main"`, `"reit"`, …).
    pub action: Option<String>,
}

/// Aggregated analyst price targets from `financialData`.
#[derive(Debug, Clone, PartialEq)]
pub struct PriceTarget {
    /// Mean target price.
    pub mean: Option<f64>,
    /// Highest target price.
    pub high: Option<f64>,
    /// Lowest target price.
    pub low: Option<f64>,
    /// Number of analyst opinions.
    pub number_of_analysts: Option<u32>,
}

/// One row of `earningsTrend.trend`. The same shape covers the four periods
/// Yahoo reports (`"0q"`, `"+1q"`, `"0y"`, `"+1y"`).
#[derive(Debug, Clone, PartialEq)]
pub struct EarningsTrendRow {
    /// Period label.
    pub period: String,
    /// Year-over-year growth estimate.
    pub growth: Option<f64>,
    /// Earnings-per-share estimates.
    pub earnings_estimate: EarningsEstimate,
    /// Revenue estimates.
    pub revenue_estimate: RevenueEstimate,
    /// EPS trend across the last 90 days.
    pub eps_trend: EpsTrend,
    /// Up/down revisions over 7- and 30-day windows.
    pub eps_revisions: EpsRevisions,
}

/// Earnings-per-share estimate for one period.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EarningsEstimate {
    /// Mean estimate.
    pub avg: Option<f64>,
    /// Lowest estimate.
    pub low: Option<f64>,
    /// Highest estimate.
    pub high: Option<f64>,
    /// EPS the same period a year ago (for growth calculation).
    pub year_ago_eps: Option<f64>,
    /// Number of analysts contributing.
    pub num_analysts: Option<u32>,
    /// Year-over-year growth fraction.
    pub growth: Option<f64>,
}

/// Revenue estimate for one period (raw values are dollars).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RevenueEstimate {
    /// Mean estimate.
    pub avg: Option<i64>,
    /// Lowest estimate.
    pub low: Option<i64>,
    /// Highest estimate.
    pub high: Option<i64>,
    /// Revenue the same period a year ago.
    pub year_ago_revenue: Option<i64>,
    /// Number of analysts contributing.
    pub num_analysts: Option<u32>,
    /// Year-over-year growth fraction (decimal, e.g. `0.157` for +15.7%).
    pub growth: Option<f64>,
}

/// EPS trend snapshot — current consensus and 7/30/60/90-day-old values.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EpsTrend {
    /// Current consensus.
    pub current: Option<f64>,
    /// Consensus 7 days ago.
    pub seven_days_ago: Option<f64>,
    /// Consensus 30 days ago.
    pub thirty_days_ago: Option<f64>,
    /// Consensus 60 days ago.
    pub sixty_days_ago: Option<f64>,
    /// Consensus 90 days ago.
    pub ninety_days_ago: Option<f64>,
}

/// Up- and down-revision counts over 7 and 30 days.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EpsRevisions {
    /// Up-revisions in the last 7 days.
    pub up_last_7_days: Option<u32>,
    /// Up-revisions in the last 30 days.
    pub up_last_30_days: Option<u32>,
    /// Down-revisions in the last 7 days.
    pub down_last_7_days: Option<u32>,
    /// Down-revisions in the last 30 days.
    pub down_last_30_days: Option<u32>,
}

#[derive(Deserialize)]
struct V10Result {
    #[serde(default, rename = "recommendationTrend")]
    recommendation_trend: Option<RecommendationTrendNode>,
    #[serde(default, rename = "upgradeDowngradeHistory")]
    upgrade_downgrade_history: Option<UpgradeDowngradeHistoryNode>,
    #[serde(default, rename = "financialData")]
    financial_data: Option<FinancialDataNode>,
    #[serde(default, rename = "earningsTrend")]
    earnings_trend: Option<EarningsTrendNode>,
}

#[derive(Deserialize)]
struct RecommendationTrendNode {
    #[serde(default)]
    trend: Option<Vec<RecommendationNode>>,
}

#[derive(Deserialize)]
struct RecommendationNode {
    #[serde(default)]
    period: Option<String>,
    #[serde(default, rename = "strongBuy")]
    strong_buy: Option<i64>,
    #[serde(default)]
    buy: Option<i64>,
    #[serde(default)]
    hold: Option<i64>,
    #[serde(default)]
    sell: Option<i64>,
    #[serde(default, rename = "strongSell")]
    strong_sell: Option<i64>,
}

#[derive(Deserialize)]
struct UpgradeDowngradeHistoryNode {
    #[serde(default)]
    history: Option<Vec<UpgradeNode>>,
}

#[derive(Deserialize)]
struct UpgradeNode {
    #[serde(default, rename = "epochGradeDate")]
    epoch_grade_date: Option<i64>,
    #[serde(default)]
    firm: Option<String>,
    #[serde(default, rename = "toGrade")]
    to_grade: Option<String>,
    #[serde(default, rename = "fromGrade")]
    from_grade: Option<String>,
    #[serde(default)]
    action: Option<String>,
    #[serde(default, rename = "gradeChange")]
    grade_change: Option<String>,
}

#[derive(Deserialize)]
struct FinancialDataNode {
    #[serde(default, rename = "targetMeanPrice")]
    target_mean_price: Option<RawNum<f64>>,
    #[serde(default, rename = "targetHighPrice")]
    target_high_price: Option<RawNum<f64>>,
    #[serde(default, rename = "targetLowPrice")]
    target_low_price: Option<RawNum<f64>>,
    #[serde(default, rename = "numberOfAnalystOpinions")]
    number_of_analyst_opinions: Option<RawNum<f64>>,
    #[serde(default, rename = "recommendationMean")]
    recommendation_mean: Option<RawNum<f64>>,
    #[serde(default, rename = "recommendationKey")]
    recommendation_key: Option<String>,
}

#[derive(Deserialize)]
struct EarningsTrendNode {
    #[serde(default)]
    trend: Option<Vec<EarningsTrendItemNode>>,
}

#[derive(Deserialize)]
struct EarningsTrendItemNode {
    #[serde(default)]
    period: Option<String>,
    #[serde(default)]
    growth: Option<RawNum<f64>>,
    #[serde(default, rename = "earningsEstimate")]
    earnings_estimate: Option<EarningsEstimateNode>,
    #[serde(default, rename = "revenueEstimate")]
    revenue_estimate: Option<RevenueEstimateNode>,
    #[serde(default, rename = "epsTrend")]
    eps_trend: Option<EpsTrendNode>,
    #[serde(default, rename = "epsRevisions")]
    eps_revisions: Option<EpsRevisionsNode>,
}

#[derive(Deserialize, Default)]
struct EarningsEstimateNode {
    #[serde(default)]
    avg: Option<RawNum<f64>>,
    #[serde(default)]
    low: Option<RawNum<f64>>,
    #[serde(default)]
    high: Option<RawNum<f64>>,
    #[serde(default, rename = "yearAgoEps")]
    year_ago_eps: Option<RawNum<f64>>,
    #[serde(default, rename = "numberOfAnalysts")]
    num_analysts: Option<RawNum<f64>>,
    #[serde(default)]
    growth: Option<RawNum<f64>>,
}

#[derive(Deserialize, Default)]
struct RevenueEstimateNode {
    #[serde(default)]
    avg: Option<RawNum<i64>>,
    #[serde(default)]
    low: Option<RawNum<i64>>,
    #[serde(default)]
    high: Option<RawNum<i64>>,
    #[serde(default, rename = "yearAgoRevenue")]
    year_ago_revenue: Option<RawNum<i64>>,
    #[serde(default, rename = "numberOfAnalysts")]
    num_analysts: Option<RawNum<f64>>,
    #[serde(default)]
    growth: Option<RawNum<f64>>,
}

#[derive(Deserialize, Default)]
struct EpsTrendNode {
    #[serde(default)]
    current: Option<RawNum<f64>>,
    #[serde(default, rename = "7daysAgo")]
    seven_days_ago: Option<RawNum<f64>>,
    #[serde(default, rename = "30daysAgo")]
    thirty_days_ago: Option<RawNum<f64>>,
    #[serde(default, rename = "60daysAgo")]
    sixty_days_ago: Option<RawNum<f64>>,
    #[serde(default, rename = "90daysAgo")]
    ninety_days_ago: Option<RawNum<f64>>,
}

#[derive(Deserialize, Default)]
struct EpsRevisionsNode {
    #[serde(default, rename = "upLast7days")]
    up_last_7_days: Option<RawNum<f64>>,
    #[serde(default, rename = "upLast30days")]
    up_last_30_days: Option<RawNum<f64>>,
    #[serde(default, rename = "downLast7days")]
    down_last_7_days: Option<RawNum<f64>>,
    #[serde(default, rename = "downLast30days")]
    down_last_30_days: Option<RawNum<f64>>,
}

async fn fetch_modules(
    client: &YfClient,
    symbol: &str,
    modules: &str,
    fixture_label: &str,
) -> Result<V10Result> {
    let Some(map) = client
        .fetch_quote_summary(symbol, modules, fixture_label)
        .await?
    else {
        return Ok(V10Result {
            recommendation_trend: None,
            upgrade_downgrade_history: None,
            financial_data: None,
            earnings_trend: None,
        });
    };
    serde_json::from_value(Value::Object(map)).map_err(Error::from)
}

/// Per-period analyst counts (`recommendationTrend`).
pub(crate) async fn recommendations(
    client: &YfClient,
    symbol: &str,
) -> Result<Vec<RecommendationRow>> {
    let r = fetch_modules(
        client,
        symbol,
        "recommendationTrend",
        "analysis_recommendationTrend",
    )
    .await?;
    let trend = r
        .recommendation_trend
        .and_then(|x| x.trend)
        .unwrap_or_default();
    Ok(trend
        .into_iter()
        .map(|n| RecommendationRow {
            period: n.period.unwrap_or_default(),
            strong_buy: n.strong_buy.and_then(|v| u32::try_from(v).ok()),
            buy: n.buy.and_then(|v| u32::try_from(v).ok()),
            hold: n.hold.and_then(|v| u32::try_from(v).ok()),
            sell: n.sell.and_then(|v| u32::try_from(v).ok()),
            strong_sell: n.strong_sell.and_then(|v| u32::try_from(v).ok()),
        })
        .collect())
}

/// Latest period plus `recommendationMean` / `recommendationKey`.
pub(crate) async fn recommendations_summary(
    client: &YfClient,
    symbol: &str,
) -> Result<RecommendationSummary> {
    let r = fetch_modules(
        client,
        symbol,
        "recommendationTrend,financialData",
        "analysis_recommendationTrend-financialData",
    )
    .await?;
    let latest = r
        .recommendation_trend
        .and_then(|x| x.trend.and_then(|t| t.into_iter().next()));
    let (period, sb, b, h, s, ss) = latest.map_or((None, None, None, None, None, None), |t| {
        (
            t.period,
            t.strong_buy.and_then(|v| u32::try_from(v).ok()),
            t.buy.and_then(|v| u32::try_from(v).ok()),
            t.hold.and_then(|v| u32::try_from(v).ok()),
            t.sell.and_then(|v| u32::try_from(v).ok()),
            t.strong_sell.and_then(|v| u32::try_from(v).ok()),
        )
    });
    let (mean, rating) = r.financial_data.map_or((None, None), |fd| {
        (raw_f64(fd.recommendation_mean), fd.recommendation_key)
    });
    Ok(RecommendationSummary {
        latest_period: period,
        strong_buy: sb,
        buy: b,
        hold: h,
        sell: s,
        strong_sell: ss,
        mean,
        rating,
    })
}

/// History of analyst upgrades and downgrades, oldest first.
pub(crate) async fn upgrades_downgrades(
    client: &YfClient,
    symbol: &str,
) -> Result<Vec<UpgradeDowngradeRow>> {
    let r = fetch_modules(
        client,
        symbol,
        "upgradeDowngradeHistory",
        "analysis_upgradeDowngradeHistory",
    )
    .await?;
    let mut rows: Vec<_> = r
        .upgrade_downgrade_history
        .and_then(|x| x.history)
        .unwrap_or_default()
        .into_iter()
        .map(|h| UpgradeDowngradeRow {
            timestamp: h.epoch_grade_date.unwrap_or_default(),
            firm: h.firm,
            from_grade: h.from_grade,
            to_grade: h.to_grade,
            action: h.action.or(h.grade_change),
        })
        .collect();
    rows.sort_by_key(|r| r.timestamp);
    Ok(rows)
}

/// Mean / high / low analyst price target plus contributor count.
pub(crate) async fn price_target(client: &YfClient, symbol: &str) -> Result<PriceTarget> {
    let r = fetch_modules(client, symbol, "financialData", "analysis_financialData").await?;
    let fd = r
        .financial_data
        .ok_or_else(|| Error::invalid("analysis: financialData module missing"))?;
    Ok(PriceTarget {
        mean: raw_f64(fd.target_mean_price),
        high: raw_f64(fd.target_high_price),
        low: raw_f64(fd.target_low_price),
        number_of_analysts: raw_u32(fd.number_of_analyst_opinions),
    })
}

/// Earnings & revenue estimates, EPS trend, EPS revisions for the four
/// reporting periods Yahoo exposes (`0q`, `+1q`, `0y`, `+1y`).
pub(crate) async fn earnings_trend(
    client: &YfClient,
    symbol: &str,
) -> Result<Vec<EarningsTrendRow>> {
    let r = fetch_modules(client, symbol, "earningsTrend", "analysis_earningsTrend").await?;
    let trend = r.earnings_trend.and_then(|x| x.trend).unwrap_or_default();
    Ok(trend
        .into_iter()
        .map(|n| EarningsTrendRow {
            period: n.period.unwrap_or_default(),
            growth: raw_f64(n.growth),
            earnings_estimate: n
                .earnings_estimate
                .map(|e| EarningsEstimate {
                    avg: raw_f64(e.avg),
                    low: raw_f64(e.low),
                    high: raw_f64(e.high),
                    year_ago_eps: raw_f64(e.year_ago_eps),
                    num_analysts: raw_u32(e.num_analysts),
                    growth: raw_f64(e.growth),
                })
                .unwrap_or_default(),
            revenue_estimate: n
                .revenue_estimate
                .map(|e| RevenueEstimate {
                    avg: raw_i64(e.avg),
                    low: raw_i64(e.low),
                    high: raw_i64(e.high),
                    year_ago_revenue: raw_i64(e.year_ago_revenue),
                    num_analysts: raw_u32(e.num_analysts),
                    growth: raw_f64(e.growth),
                })
                .unwrap_or_default(),
            eps_trend: n
                .eps_trend
                .map(|e| EpsTrend {
                    current: raw_f64(e.current),
                    seven_days_ago: raw_f64(e.seven_days_ago),
                    thirty_days_ago: raw_f64(e.thirty_days_ago),
                    sixty_days_ago: raw_f64(e.sixty_days_ago),
                    ninety_days_ago: raw_f64(e.ninety_days_ago),
                })
                .unwrap_or_default(),
            eps_revisions: n
                .eps_revisions
                .map(|e| EpsRevisions {
                    up_last_7_days: raw_u32(e.up_last_7_days),
                    up_last_30_days: raw_u32(e.up_last_30_days),
                    down_last_7_days: raw_u32(e.down_last_7_days),
                    down_last_30_days: raw_u32(e.down_last_30_days),
                })
                .unwrap_or_default(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_recommendation_trend() {
        let v: V10Result = serde_json::from_value(serde_json::json!({
            "recommendationTrend": {
                "trend": [
                    {"period": "0m", "strongBuy": 5, "buy": 10, "hold": 3, "sell": 1, "strongSell": 0}
                ]
            }
        }))
        .unwrap();
        let trend = v.recommendation_trend.unwrap().trend.unwrap();
        assert_eq!(trend.len(), 1);
        assert_eq!(trend[0].buy, Some(10));
    }

    #[test]
    fn raw_u32_rounds_and_filters() {
        assert_eq!(raw_u32(Some(RawNum { raw: Some(3.6) })), Some(4));
        assert_eq!(raw_u32(Some(RawNum { raw: Some(-1.0) })), None);
        assert_eq!(raw_u32(None::<RawNum<f64>>), None);
    }
}
