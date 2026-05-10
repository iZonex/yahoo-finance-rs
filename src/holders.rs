//! Ownership and insider data via `quoteSummary`.

use serde_json::Value;

use crate::client::YfClient;
use crate::error::Result;

/// Aggregate of all holder-related modules from `quoteSummary`.
#[derive(Debug, Clone, Default)]
pub struct Holders {
    /// Symbol echoed back.
    pub symbol: String,
    /// `majorHoldersBreakdown` → percent of shares held in each bucket.
    pub major: Vec<MajorHolderRow>,
    /// `institutionOwnership.ownershipList`.
    pub institutional: Vec<HolderRow>,
    /// `fundOwnership.ownershipList`.
    pub mutual_fund: Vec<HolderRow>,
    /// `insiderTransactions.transactions`.
    pub insider_transactions: Vec<InsiderTransaction>,
    /// `insiderHolders.holders` (the roster of insiders by role).
    pub insider_roster: Vec<InsiderRosterRow>,
    /// `netSharePurchaseActivity` summary.
    pub net_share_purchase: Option<NetSharePurchaseActivity>,
}

/// One row from `majorHoldersBreakdown`.
#[derive(Debug, Clone)]
pub struct MajorHolderRow {
    /// Bucket label (`Insiders`, `Institutions`, `Top 10`, …).
    pub label: String,
    /// Percent of shares (0..1).
    pub percent: f64,
}

/// One row from institutional / mutual-fund ownership lists.
#[derive(Debug, Clone)]
pub struct HolderRow {
    /// Reporting date (YYYY-MM-DD).
    pub report_date: Option<String>,
    /// Holder name.
    pub holder: String,
    /// Shares held.
    pub shares: Option<u64>,
    /// Position value at reporting date.
    pub value: Option<u64>,
    /// Percent of float held (decimal).
    pub percent_held: Option<f64>,
}

/// Insider buy/sell activity.
#[derive(Debug, Clone)]
pub struct InsiderTransaction {
    /// Insider's name.
    pub name: String,
    /// Insider's relation (CEO, Director, …).
    pub relation: Option<String>,
    /// Transaction date.
    pub start_date: Option<String>,
    /// Description of the action.
    pub transaction: Option<String>,
    /// Direct or indirect ownership.
    pub ownership: Option<String>,
    /// Shares involved.
    pub shares: Option<u64>,
    /// Reported dollar value.
    pub value: Option<u64>,
}

/// One insider with current position.
#[derive(Debug, Clone)]
pub struct InsiderRosterRow {
    /// Insider's name.
    pub name: String,
    /// Title / role.
    pub relation: Option<String>,
    /// Shares directly held.
    pub direct_shares: Option<u64>,
    /// As-of date.
    pub position_date: Option<String>,
}

/// Insider net purchase activity summary.
#[derive(Debug, Clone, Default)]
pub struct NetSharePurchaseActivity {
    /// Period label (`6m`, …).
    pub period: Option<String>,
    /// Net shares acquired.
    pub net_info_shares: Option<i64>,
    /// Number of buy-side transactions.
    pub buy_info_count: Option<u64>,
    /// Total shares bought.
    pub buy_info_shares: Option<u64>,
    /// Number of sell-side transactions.
    pub sell_info_count: Option<u64>,
    /// Total shares sold.
    pub sell_info_shares: Option<u64>,
}

const HOLDERS_MODULES: &[&str] = &[
    "majorHoldersBreakdown",
    "institutionOwnership",
    "fundOwnership",
    "insiderTransactions",
    "insiderHolders",
    "netSharePurchaseActivity",
];

impl Holders {
    pub(crate) async fn fetch(client: &YfClient, symbol: &str) -> Result<Self> {
        let modules = client
            .fetch_quote_summary(symbol, &HOLDERS_MODULES.join(","), "holders_quoteSummary")
            .await?
            .unwrap_or_default();
        Ok(parse_holders(symbol, modules))
    }
}

fn parse_holders(symbol: &str, modules: serde_json::Map<String, Value>) -> Holders {
    let mut h = Holders {
        symbol: symbol.to_string(),
        ..Default::default()
    };

    if let Some(b) = modules
        .get("majorHoldersBreakdown")
        .and_then(|v| v.as_object())
    {
        for (k, v) in b {
            if k == "maxAge" {
                continue;
            }
            if let Some(pct) = raw_f64(v) {
                h.major.push(MajorHolderRow {
                    label: humanize(k),
                    percent: pct,
                });
            }
        }
    }
    if let Some(list) = modules
        .get("institutionOwnership")
        .and_then(|v| v.get("ownershipList"))
        .and_then(|v| v.as_array())
    {
        h.institutional = list.iter().filter_map(parse_holder_row).collect();
    }
    if let Some(list) = modules
        .get("fundOwnership")
        .and_then(|v| v.get("ownershipList"))
        .and_then(|v| v.as_array())
    {
        h.mutual_fund = list.iter().filter_map(parse_holder_row).collect();
    }
    if let Some(list) = modules
        .get("insiderTransactions")
        .and_then(|v| v.get("transactions"))
        .and_then(|v| v.as_array())
    {
        for item in list {
            let Some(o) = item.as_object() else { continue };
            h.insider_transactions.push(InsiderTransaction {
                name: o
                    .get("filerName")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string(),
                relation: o
                    .get("filerRelation")
                    .and_then(|x| x.as_str())
                    .map(String::from),
                start_date: raw_str_date(o.get("startDate")),
                transaction: o
                    .get("transactionText")
                    .and_then(|x| x.as_str())
                    .map(String::from),
                ownership: o
                    .get("ownership")
                    .and_then(|x| x.as_str())
                    .map(String::from),
                shares: o.get("shares").and_then(raw_u64),
                value: o.get("value").and_then(raw_u64),
            });
        }
    }
    if let Some(list) = modules
        .get("insiderHolders")
        .and_then(|v| v.get("holders"))
        .and_then(|v| v.as_array())
    {
        for item in list {
            let Some(o) = item.as_object() else { continue };
            h.insider_roster.push(InsiderRosterRow {
                name: o
                    .get("name")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string(),
                relation: o.get("relation").and_then(|x| x.as_str()).map(String::from),
                direct_shares: o.get("positionDirect").and_then(raw_u64),
                position_date: raw_str_date(o.get("positionDirectDate")),
            });
        }
    }
    if let Some(o) = modules
        .get("netSharePurchaseActivity")
        .and_then(|v| v.as_object())
    {
        let period = o.get("period").and_then(|v| v.as_str()).map(String::from);
        h.net_share_purchase = Some(NetSharePurchaseActivity {
            period,
            net_info_shares: o.get("netInfoShares").and_then(raw_i64),
            buy_info_count: o.get("buyInfoCount").and_then(raw_u64),
            buy_info_shares: o.get("buyInfoShares").and_then(raw_u64),
            sell_info_count: o.get("sellInfoCount").and_then(raw_u64),
            sell_info_shares: o.get("sellInfoShares").and_then(raw_u64),
        });
    }
    h
}

fn parse_holder_row(v: &Value) -> Option<HolderRow> {
    let o = v.as_object()?;
    Some(HolderRow {
        report_date: raw_str_date(o.get("reportDate")),
        holder: o
            .get("organization")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        shares: o.get("position").and_then(raw_u64),
        value: o.get("value").and_then(raw_u64),
        percent_held: o.get("pctHeld").and_then(raw_f64),
    })
}

fn raw_f64(v: &Value) -> Option<f64> {
    if let Some(f) = v.as_f64() {
        return Some(f);
    }
    v.get("raw").and_then(|x| x.as_f64())
}
fn raw_u64(v: &Value) -> Option<u64> {
    if let Some(u) = v.as_u64() {
        return Some(u);
    }
    if let Some(f) = v.as_f64() {
        return Some(f as u64);
    }
    let r = v.get("raw")?;
    if let Some(u) = r.as_u64() {
        return Some(u);
    }
    r.as_f64().map(|f| f as u64)
}
fn raw_i64(v: &Value) -> Option<i64> {
    if let Some(i) = v.as_i64() {
        return Some(i);
    }
    let r = v.get("raw")?;
    r.as_i64().or_else(|| r.as_f64().map(|f| f as i64))
}
fn raw_str_date(v: Option<&Value>) -> Option<String> {
    let v = v?;
    if let Some(s) = v.get("fmt").and_then(|x| x.as_str()) {
        return Some(s.to_string());
    }
    v.as_str().map(String::from)
}

fn humanize(key: &str) -> String {
    match key {
        "insidersPercentHeld" => "Insiders".into(),
        "institutionsPercentHeld" => "Institutions".into(),
        "institutionsFloatPercentHeld" => "Institutions Float".into(),
        "institutionsCount" => "Institutions Count".into(),
        other => other.to_string(),
    }
}
