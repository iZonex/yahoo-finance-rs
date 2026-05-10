//! ESG / sustainability scores via Yahoo's `esgScores` quoteSummary module.
//!
//! Mirrors `Ticker.sustainability` from the Python `yfinance` library: returns
//! the three core scores and a list of involvement flags (adult, gambling,
//! tobacco, …) the company is tagged with.
//!
//! ⚠️ Yahoo has been moving this endpoint around; an empty `esgScores` payload
//! is mapped to `Ok(None)` so callers can degrade gracefully.

use serde::Deserialize;

use crate::client::YfClient;
use crate::error::{Error, Result};
use crate::wire::RawNum;

/// Top-level ESG scores. Each component is `Option` because Yahoo occasionally
/// omits one even when the others are present.
#[derive(Debug, Clone, PartialEq)]
pub struct EsgScores {
    /// Overall ESG risk score (lower is better in Sustainalytics' framing).
    pub total: Option<f64>,
    /// Environmental sub-score.
    pub environment: Option<f64>,
    /// Social sub-score.
    pub social: Option<f64>,
    /// Governance sub-score.
    pub governance: Option<f64>,
    /// Peer-percentile rank (0..100).
    pub percentile: Option<f64>,
    /// Highest controversy level (1..5).
    pub highest_controversy: Option<f64>,
}

/// One involvement-area flag — Yahoo only reports the category, not a score.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EsgInvolvement {
    /// Lower-snake-case name (`adult`, `controversial_weapons`, `thermal_coal`, …).
    pub category: String,
}

/// Combined sustainability snapshot for a single symbol.
#[derive(Debug, Clone, PartialEq)]
pub struct EsgSummary {
    /// Symbol the data was fetched for.
    pub symbol: String,
    /// Numeric scores.
    pub scores: EsgScores,
    /// Involvement-area flags Yahoo set to `true`.
    pub involvement: Vec<EsgInvolvement>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawEsg {
    #[serde(default)]
    total_esg: Option<RawNum<f64>>,
    #[serde(default)]
    environment_score: Option<RawNum<f64>>,
    #[serde(default)]
    social_score: Option<RawNum<f64>>,
    #[serde(default)]
    governance_score: Option<RawNum<f64>>,
    #[serde(default)]
    percentile: Option<f64>,
    #[serde(default)]
    highest_controversy: Option<f64>,

    #[serde(default)]
    adult: Option<bool>,
    #[serde(default)]
    alcoholic: Option<bool>,
    #[serde(default)]
    animal_testing: Option<bool>,
    #[serde(default)]
    catholic: Option<bool>,
    #[serde(default)]
    controversial_weapons: Option<bool>,
    #[serde(default)]
    small_arms: Option<bool>,
    #[serde(default)]
    fur_leather: Option<bool>,
    #[serde(default)]
    gambling: Option<bool>,
    #[serde(default)]
    gmo: Option<bool>,
    #[serde(default)]
    military_contract: Option<bool>,
    #[serde(default)]
    nuclear: Option<bool>,
    #[serde(default)]
    palm_oil: Option<bool>,
    #[serde(default)]
    pesticides: Option<bool>,
    /// JSON key is `coal`, exposed as the more descriptive `thermal_coal`.
    #[serde(default, rename = "coal")]
    thermal_coal: Option<bool>,
    #[serde(default)]
    tobacco: Option<bool>,
}

impl EsgSummary {
    pub(crate) async fn fetch(client: &YfClient, symbol: &str) -> Result<Option<Self>> {
        let Some(node) = client
            .fetch_quote_summary(symbol, "esgScores", "esg_quoteSummary")
            .await?
            .and_then(|m| m.get("esgScores").cloned())
        else {
            return Ok(None);
        };
        if node.is_null() {
            return Ok(None);
        }
        let raw: RawEsg = serde_json::from_value(node).map_err(Error::from)?;

        let scores = EsgScores {
            total: raw.total_esg.as_ref().and_then(|n| n.raw),
            environment: raw.environment_score.as_ref().and_then(|n| n.raw),
            social: raw.social_score.as_ref().and_then(|n| n.raw),
            governance: raw.governance_score.as_ref().and_then(|n| n.raw),
            percentile: raw.percentile,
            highest_controversy: raw.highest_controversy,
        };

        let involvement = collect_involvement(&raw);

        Ok(Some(Self {
            symbol: symbol.to_string(),
            scores,
            involvement,
        }))
    }
}

fn collect_involvement(r: &RawEsg) -> Vec<EsgInvolvement> {
    let flags: &[(&str, Option<bool>)] = &[
        ("adult", r.adult),
        ("alcoholic", r.alcoholic),
        ("animal_testing", r.animal_testing),
        ("catholic", r.catholic),
        ("controversial_weapons", r.controversial_weapons),
        ("small_arms", r.small_arms),
        ("fur_leather", r.fur_leather),
        ("gambling", r.gambling),
        ("gmo", r.gmo),
        ("military_contract", r.military_contract),
        ("nuclear", r.nuclear),
        ("palm_oil", r.palm_oil),
        ("pesticides", r.pesticides),
        ("thermal_coal", r.thermal_coal),
        ("tobacco", r.tobacco),
    ];
    flags
        .iter()
        .filter(|(_, set)| set.unwrap_or(false))
        .map(|(name, _)| EsgInvolvement {
            category: (*name).to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_payload() {
        let raw: RawEsg = serde_json::from_value(serde_json::json!({
            "totalEsg": { "raw": 17.5 },
            "environmentScore": { "raw": 1.2 },
            "socialScore": { "raw": 8.0 },
            "governanceScore": { "raw": 8.3 },
            "percentile": 73.4,
            "highestControversy": 2.0,
            "tobacco": true,
            "coal": true
        }))
        .unwrap();

        let scores = EsgScores {
            total: raw.total_esg.as_ref().and_then(|n| n.raw),
            environment: raw.environment_score.as_ref().and_then(|n| n.raw),
            social: raw.social_score.as_ref().and_then(|n| n.raw),
            governance: raw.governance_score.as_ref().and_then(|n| n.raw),
            percentile: raw.percentile,
            highest_controversy: raw.highest_controversy,
        };

        assert!((scores.total.unwrap() - 17.5).abs() < 1e-9);
        let inv = collect_involvement(&raw);
        let cats: Vec<&str> = inv.iter().map(|i| i.category.as_str()).collect();
        assert!(cats.contains(&"tobacco"));
        assert!(cats.contains(&"thermal_coal"));
    }
}
