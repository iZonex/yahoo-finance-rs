//! Company / fund profile via the `quoteSummary` modules
//! `assetProfile`, `quoteType`, and `fundProfile`.
//!
//! Yahoo splits profile data across two modules — `assetProfile` for equities,
//! `fundProfile` for ETFs and mutual funds — discriminated by `quoteType`.
//! [`Profile`] is the union.

use serde::Deserialize;

use crate::client::YfClient;
use crate::error::{Error, Result};

/// Mailing address fields from `assetProfile`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Address {
    /// First street line.
    pub street1: Option<String>,
    /// Second street line.
    pub street2: Option<String>,
    /// City.
    pub city: Option<String>,
    /// State / region code.
    pub state: Option<String>,
    /// Country.
    pub country: Option<String>,
    /// ZIP / postal code.
    pub zip: Option<String>,
}

/// Profile of a publicly traded company (`quoteType == "EQUITY"`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanyProfile {
    /// Display name (long, falling back to short).
    pub name: String,
    /// Industry sector.
    pub sector: Option<String>,
    /// Industry within the sector.
    pub industry: Option<String>,
    /// Investor-relations website.
    pub website: Option<String>,
    /// Long-form business description.
    pub summary: Option<String>,
    /// Headquarters address.
    pub address: Address,
    /// ISIN as reported by `assetProfile`. Note: not all equities expose it
    /// here — for a more reliable lookup use [`Ticker::isin`].
    ///
    /// [`Ticker::isin`]: crate::ticker::Ticker::isin
    pub isin: Option<String>,
}

/// Profile of a fund (`quoteType` ∈ {`"ETF"`, `"MUTUALFUND"`}).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FundProfile {
    /// Display name.
    pub name: String,
    /// Fund family / sponsor.
    pub family: Option<String>,
    /// Legal type (`"Open End Fund"`, `"Exchange Traded Fund"`, …).
    pub legal_type: Option<String>,
    /// ISIN as reported by `fundProfile`.
    pub isin: Option<String>,
}

/// Profile of a ticker.
///
/// `Other` covers the long tail (`INDEX`, `CURRENCY`, `FUTURE`, …) where
/// neither the company nor the fund modules carry useful data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Profile {
    /// Equity / company.
    Company(CompanyProfile),
    /// ETF / mutual fund.
    Fund(FundProfile),
    /// Anything else Yahoo classifies the symbol as.
    Other {
        /// Display name.
        name: String,
        /// Quote type (`"INDEX"`, `"CURRENCY"`, …).
        quote_type: String,
    },
}

impl Profile {
    /// Display name regardless of variant.
    pub fn name(&self) -> &str {
        match self {
            Self::Company(c) => &c.name,
            Self::Fund(f) => &f.name,
            Self::Other { name, .. } => name,
        }
    }
}

#[derive(Deserialize, Default)]
struct V10AssetProfile {
    #[serde(default)]
    address1: Option<String>,
    #[serde(default)]
    address2: Option<String>,
    #[serde(default)]
    city: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    country: Option<String>,
    #[serde(default)]
    zip: Option<String>,
    #[serde(default)]
    sector: Option<String>,
    #[serde(default)]
    industry: Option<String>,
    #[serde(default)]
    website: Option<String>,
    #[serde(default, rename = "longBusinessSummary")]
    long_business_summary: Option<String>,
    #[serde(default)]
    isin: Option<String>,
}

#[derive(Deserialize, Default)]
struct V10FundProfile {
    #[serde(default, rename = "legalType")]
    legal_type: Option<String>,
    #[serde(default)]
    family: Option<String>,
    #[serde(default)]
    isin: Option<String>,
}

#[derive(Deserialize)]
struct V10QuoteType {
    #[serde(default, rename = "quoteType")]
    quote_type: Option<String>,
    #[serde(default, rename = "longName")]
    long_name: Option<String>,
    #[serde(default, rename = "shortName")]
    short_name: Option<String>,
}

#[derive(Deserialize)]
struct V10Result {
    #[serde(default, rename = "assetProfile")]
    asset_profile: Option<V10AssetProfile>,
    #[serde(default, rename = "fundProfile")]
    fund_profile: Option<V10FundProfile>,
    #[serde(default, rename = "quoteType")]
    quote_type: Option<V10QuoteType>,
}

pub(crate) async fn fetch(client: &YfClient, symbol: &str) -> Result<Profile> {
    if let Some(modules) = client
        .fetch_quote_summary(
            symbol,
            "assetProfile,quoteType,fundProfile",
            "profile_quoteSummary",
        )
        .await?
    {
        let result: V10Result =
            serde_json::from_value(serde_json::Value::Object(modules)).map_err(Error::from)?;
        if let Some(profile) = profile_from_v10(symbol, result) {
            return Ok(profile);
        }
    }
    // API path was empty or had no recognisable shape — fall back to the
    // human-facing quote page, which embeds the same data in `root.App.main`.
    scrape_profile(client, symbol).await
}

fn profile_from_v10(symbol: &str, result: V10Result) -> Option<Profile> {
    let kind = result
        .quote_type
        .as_ref()
        .and_then(|q| q.quote_type.clone())
        .unwrap_or_default();
    let name = result
        .quote_type
        .as_ref()
        .and_then(|q| q.long_name.clone().or_else(|| q.short_name.clone()))
        .unwrap_or_else(|| symbol.to_string());
    Some(match kind.as_str() {
        "EQUITY" => {
            let p = result.asset_profile?;
            Profile::Company(CompanyProfile {
                name,
                sector: p.sector,
                industry: p.industry,
                website: p.website,
                summary: p.long_business_summary,
                address: Address {
                    street1: p.address1,
                    street2: p.address2,
                    city: p.city,
                    state: p.state,
                    country: p.country,
                    zip: p.zip,
                },
                isin: p.isin,
            })
        }
        "ETF" | "MUTUALFUND" => {
            let p = result.fund_profile?;
            Profile::Fund(FundProfile {
                name,
                family: p.family,
                legal_type: p.legal_type,
                isin: p.isin,
            })
        }
        "" => return None,
        other => Profile::Other {
            name,
            quote_type: other.to_string(),
        },
    })
}

/* ------------------- HTML scrape fallback ------------------- */

#[derive(Deserialize)]
struct Bootstrap {
    #[serde(default)]
    context: Option<Ctx>,
}

#[derive(Deserialize)]
struct Ctx {
    #[serde(default)]
    dispatcher: Option<Dispatch>,
}

#[derive(Deserialize)]
struct Dispatch {
    #[serde(default)]
    stores: Option<Stores>,
}

#[derive(Deserialize)]
struct Stores {
    #[serde(default, rename = "QuoteSummaryStore")]
    quote_summary_store: Option<QuoteSummaryStore>,
}

#[derive(Deserialize)]
struct QuoteSummaryStore {
    #[serde(default, rename = "quoteType")]
    quote_type: Option<ScrapeQuoteType>,
    #[serde(default, rename = "summaryProfile")]
    summary_profile: Option<V10AssetProfile>,
    #[serde(default, rename = "fundProfile")]
    fund_profile: Option<V10FundProfile>,
}

#[derive(Deserialize)]
struct ScrapeQuoteType {
    #[serde(default, rename = "quoteType")]
    kind: Option<String>,
    #[serde(default, rename = "longName")]
    long_name: Option<String>,
    #[serde(default, rename = "shortName")]
    short_name: Option<String>,
}

async fn scrape_profile(client: &YfClient, symbol: &str) -> Result<Profile> {
    let url = format!(
        "{}/{}?p={}",
        client.quote_page_base_url(),
        YfClient::path_encode(symbol),
        YfClient::path_encode(symbol)
    );
    let req = client.raw_get(&url);
    let Some(html) = client
        .send_text_recorded(req, Some(("profile_html", symbol)))
        .await?
    else {
        return Err(Error::TickerMissing {
            ticker: symbol.to_string(),
            reason: "quote page returned non-success".into(),
        });
    };

    let json_str = extract_bootstrap_json(&html).ok_or_else(|| Error::TickerMissing {
        ticker: symbol.to_string(),
        reason: "could not locate profile JSON in scrape body".into(),
    })?;
    let boot: Bootstrap = serde_json::from_str(&json_str).map_err(Error::from)?;
    let store = boot
        .context
        .and_then(|c| c.dispatcher)
        .and_then(|d| d.stores)
        .and_then(|s| s.quote_summary_store)
        .ok_or_else(|| Error::TickerMissing {
            ticker: symbol.to_string(),
            reason: "scrape JSON missing QuoteSummaryStore".into(),
        })?;

    let qt = store.quote_type.as_ref();
    let name = qt
        .and_then(|q| q.long_name.clone().or_else(|| q.short_name.clone()))
        .unwrap_or_else(|| symbol.to_string());
    let kind = qt
        .and_then(|q| q.kind.clone())
        .or_else(|| {
            if store.summary_profile.is_some() {
                Some("EQUITY".into())
            } else if store.fund_profile.is_some() {
                Some("ETF".into())
            } else {
                None
            }
        })
        .unwrap_or_default();

    Ok(match kind.as_str() {
        "EQUITY" => {
            let p = store.summary_profile.unwrap_or_default();
            Profile::Company(CompanyProfile {
                name,
                sector: p.sector,
                industry: p.industry,
                website: p.website,
                summary: p.long_business_summary,
                address: Address {
                    street1: p.address1,
                    street2: p.address2,
                    city: p.city,
                    state: p.state,
                    country: p.country,
                    zip: p.zip,
                },
                isin: p.isin,
            })
        }
        "ETF" | "MUTUALFUND" => {
            let p = store.fund_profile.unwrap_or_default();
            Profile::Fund(FundProfile {
                name,
                family: p.family,
                legal_type: p.legal_type,
                isin: p.isin,
            })
        }
        other => Profile::Other {
            name,
            quote_type: other.to_string(),
        },
    })
}

/// Extract the JSON blob Yahoo embeds in the quote page. Handles two forms:
/// the legacy `root.App.main = {...};` assignment and a literal
/// `"QuoteSummaryStore" : {...}` object — which we wrap into the same
/// dispatcher shape so a single `Bootstrap` deserializer handles both.
fn extract_bootstrap_json(body: &str) -> Option<String> {
    if let Some(json) = strategy_root_app_main(body) {
        return Some(json);
    }
    strategy_quote_summary_literal(body)
}

fn strategy_root_app_main(body: &str) -> Option<String> {
    let start = body.find("root.App.main")?;
    let after = &body[start..];
    let eq = after.find('=')?;
    let payload = after[eq + 1..].trim_start();
    let end_script = payload.find("</script>").unwrap_or(payload.len());
    let segment = &payload[..end_script];
    let semi = segment.rfind(';')?;
    Some(segment[..semi].trim().to_string())
}

fn strategy_quote_summary_literal(body: &str) -> Option<String> {
    let key = "\"QuoteSummaryStore\"";
    let pos = body.find(key)?;
    let after = &body[pos + key.len()..];
    let brace_rel = after.find('{')?;
    let obj_start = pos + key.len() + brace_rel;
    let obj_end = find_matching_brace(body, obj_start)?;
    let obj = &body[obj_start..=obj_end];
    Some(format!(
        r#"{{"context":{{"dispatcher":{{"stores":{{"QuoteSummaryStore":{obj}}}}}}}}}"#
    ))
}

/// Find the index of the `}` that closes the `{` at `start`, respecting
/// JSON string escapes.
fn find_matching_brace(s: &str, start: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    if bytes.get(start)? != &b'{' {
        return None;
    }
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escape = false;
    for (i, b) in bytes.iter().enumerate().skip(start) {
        if in_str {
            if escape {
                escape = false;
            } else if *b == b'\\' {
                escape = true;
            } else if *b == b'"' {
                in_str = false;
            }
            continue;
        }
        match *b {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_equity_payload() {
        let raw: V10Result = serde_json::from_value(serde_json::json!({
            "assetProfile": {
                "sector": "Technology",
                "industry": "Consumer Electronics",
                "website": "https://www.apple.com",
                "longBusinessSummary": "Apple Inc. designs, manufactures, ...",
                "address1": "One Apple Park Way",
                "city": "Cupertino",
                "state": "CA",
                "country": "United States",
                "zip": "95014"
            },
            "quoteType": {
                "quoteType": "EQUITY",
                "longName": "Apple Inc.",
                "shortName": "Apple"
            }
        }))
        .unwrap();

        assert_eq!(
            raw.quote_type.unwrap().long_name.as_deref(),
            Some("Apple Inc.")
        );
        assert_eq!(
            raw.asset_profile.unwrap().sector.as_deref(),
            Some("Technology")
        );
    }

    #[test]
    fn extracts_root_app_main_block() {
        let body = r#"<script>root.App.main = {"context":{"dispatcher":{"stores":{"QuoteSummaryStore":{"quoteType":{"quoteType":"EQUITY","longName":"Acme Inc."},"summaryProfile":{"sector":"Technology"}}}}}};
</script>"#;
        let json = strategy_root_app_main(body).expect("found");
        let boot: Bootstrap = serde_json::from_str(&json).unwrap();
        let store = boot
            .context
            .unwrap()
            .dispatcher
            .unwrap()
            .stores
            .unwrap()
            .quote_summary_store
            .unwrap();
        assert_eq!(
            store.quote_type.unwrap().long_name.as_deref(),
            Some("Acme Inc.")
        );
    }

    #[test]
    fn extracts_literal_quote_summary_store() {
        let body = r#"<html>...prefix...{"QuoteSummaryStore":{"quoteType":{"quoteType":"ETF","shortName":"Vanguard ETF"},"fundProfile":{"family":"Vanguard"}}}...suffix..."#;
        let wrapped = strategy_quote_summary_literal(body).expect("found");
        let boot: Bootstrap = serde_json::from_str(&wrapped).unwrap();
        assert_eq!(
            boot.context
                .unwrap()
                .dispatcher
                .unwrap()
                .stores
                .unwrap()
                .quote_summary_store
                .unwrap()
                .fund_profile
                .unwrap()
                .family
                .as_deref(),
            Some("Vanguard")
        );
    }

    #[test]
    fn brace_matcher_respects_strings() {
        let s = r#"{"a":"}}","b":{"c":1}}"#;
        let end = find_matching_brace(s, 0).unwrap();
        assert_eq!(&s[end..=end], "}");
        assert_eq!(end, s.len() - 1);
    }

    #[test]
    fn parses_etf_payload() {
        let raw: V10Result = serde_json::from_value(serde_json::json!({
            "fundProfile": {
                "legalType": "Exchange Traded Fund",
                "family": "Vanguard"
            },
            "quoteType": {
                "quoteType": "ETF",
                "shortName": "Vanguard 500 Index Fund ETF"
            }
        }))
        .unwrap();
        assert_eq!(
            raw.fund_profile.unwrap().family.as_deref(),
            Some("Vanguard")
        );
    }
}
