//! ISIN lookup via the Business Insider suggest endpoint.
//!
//! Yahoo doesn't expose an ISIN endpoint, but the Python `yfinance` library —
//! and the gramistella upstream — both query Business Insider's autocomplete
//! API, which embeds ISINs in its results. This module ports that lookup.
//!
//! The endpoint returns one of several shapes (JSON object with `Suggestions`
//! / `data`, a flat array, or a JSONP-style callback). The fetch path tries
//! them in order and falls back to a raw scan for a 12-character ISIN-shaped
//! token. Use [`Ticker::isin`] for the public entry point.
//!
//! [`Ticker::isin`]: crate::ticker::Ticker::isin

use serde::Deserialize;

use crate::client::YfClient;
use crate::error::Result;

#[derive(Deserialize)]
struct FlatSuggest {
    #[serde(default, alias = "Value", alias = "value")]
    value: Option<String>,
    #[serde(default, alias = "Symbol", alias = "symbol")]
    symbol: Option<String>,
    #[serde(default, alias = "Isin", alias = "isin", alias = "ISIN")]
    isin: Option<String>,
}

/// Look up the ISIN for `symbol`. Returns `Ok(None)` if no candidate matches.
pub(crate) async fn fetch(client: &YfClient, symbol: &str) -> Result<Option<String>> {
    let url = format!(
        "{}?max_results=5&query={}",
        client.isin_base_url(),
        YfClient::path_encode(symbol)
    );
    let req = client.raw_get(&url);
    let Some(body) = client
        .send_text_recorded(req, Some(("isin_search", symbol)))
        .await?
    else {
        return Ok(None);
    };

    let target = normalize_sym(symbol);

    if let Some(isin) = parse_as_json_value(&body, &target) {
        return Ok(Some(isin));
    }
    if let Some(isin) = parse_as_flat_suggest(&body, &target) {
        return Ok(Some(isin));
    }
    Ok(scan_raw_body(&body))
}

fn parse_as_json_value(body: &str, target: &str) -> Option<String> {
    let val: serde_json::Value = serde_json::from_str(body).ok()?;
    let mut arrays: Vec<&serde_json::Value> = Vec::new();
    match &val {
        serde_json::Value::Array(_) => arrays.push(&val),
        serde_json::Value::Object(map) => {
            for key in [
                "Suggestions",
                "suggestions",
                "items",
                "results",
                "Result",
                "data",
            ] {
                if let Some(v) = map.get(key) {
                    if v.is_array() {
                        arrays.push(v);
                    }
                }
            }
            if arrays.is_empty() {
                for (_, v) in map {
                    if v.is_array() {
                        arrays.push(v);
                    } else if let Some(obj) = v.as_object() {
                        for (_, inner) in obj {
                            if inner.is_array() {
                                arrays.push(inner);
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }

    for arr in arrays {
        for item in arr.as_array()? {
            let obj = match item.as_object() {
                Some(o) => o,
                None => continue,
            };
            for k in ["Isin", "isin", "ISIN"] {
                if let Some(s) = obj.get(k).and_then(|v| v.as_str()) {
                    if looks_like_isin(s) {
                        let sym = obj
                            .get("Symbol")
                            .and_then(|v| v.as_str())
                            .or_else(|| obj.get("symbol").and_then(|v| v.as_str()))
                            .unwrap_or("");
                        if sym.is_empty() || normalize_sym(sym) == target {
                            return Some(s.to_uppercase());
                        }
                    }
                }
            }
            if let Some(value) = obj
                .get("Value")
                .and_then(|v| v.as_str())
                .or_else(|| obj.get("value").and_then(|v| v.as_str()))
            {
                let parts: Vec<String> = value
                    .split('|')
                    .map(|p| p.trim().to_string())
                    .filter(|p| !p.is_empty())
                    .collect();
                if let Some(isin) = pick_from_parts(&parts, target) {
                    return Some(isin);
                }
            }
        }
    }
    None
}

fn parse_as_flat_suggest(body: &str, target: &str) -> Option<String> {
    let arr: Vec<FlatSuggest> = serde_json::from_str(body).ok()?;
    for r in &arr {
        if let Some(isin) = r.isin.as_deref() {
            if looks_like_isin(isin)
                && r.symbol.as_deref().map(normalize_sym) == Some(target.to_string())
            {
                return Some(isin.to_uppercase());
            }
        }
        if let Some(value) = r.value.as_deref() {
            let parts: Vec<String> = value
                .split('|')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect();
            if let Some(isin) = pick_from_parts(&parts, target) {
                return Some(isin);
            }
        }
    }
    // Loose pass — first ISIN-shaped token wins.
    for r in &arr {
        if let Some(isin) = r.isin.as_deref() {
            if looks_like_isin(isin) {
                return Some(isin.to_uppercase());
            }
        }
        if let Some(value) = r.value.as_deref() {
            for tok in value.split('|').map(str::trim) {
                if looks_like_isin(tok) {
                    return Some(tok.to_uppercase());
                }
            }
        }
    }
    None
}

/// Last-ditch fallback: look for a 12-char ISIN-shaped token anywhere in the
/// raw body. Handles JSONP-style replies (`mmSuggestDeliver(...)`) that don't
/// parse as JSON at all.
fn scan_raw_body(body: &str) -> Option<String> {
    // Walk the bytes, splitting on non-alphanumerics, then look for a
    // 12-char ISIN-shaped sub-window. O(N) vs the naive `String::remove(0)`
    // sliding window, which is O(N²).
    let bytes = body.as_bytes();
    let mut start = 0usize;
    for (i, b) in bytes.iter().enumerate() {
        if !b.is_ascii_alphanumeric() {
            check_run(bytes, start, i).map(|s| s.to_ascii_uppercase());
            start = i + 1;
            continue;
        }
        if let Some(found) = check_run(bytes, start, i + 1) {
            return Some(found.to_ascii_uppercase());
        }
    }
    check_run(bytes, start, bytes.len()).map(|s| s.to_ascii_uppercase())
}

fn check_run(bytes: &[u8], start: usize, end: usize) -> Option<String> {
    if end < start + 12 {
        return None;
    }
    for w in bytes[start..end].windows(12) {
        // SAFETY: bytes are ASCII alphanumeric within a run, so valid UTF-8.
        let s = std::str::from_utf8(w).ok()?;
        if looks_like_isin(s) {
            return Some(s.to_string());
        }
    }
    None
}

fn normalize_sym(s: &str) -> String {
    let mut t = s.trim().replace('-', ".");
    for sep in ['.', ':', ' ', '\t', '\n', '\r'] {
        if let Some(idx) = t.find(sep) {
            t.truncate(idx);
            break;
        }
    }
    t.to_ascii_lowercase()
}

fn looks_like_isin(s: &str) -> bool {
    let t = s.trim();
    if t.len() != 12 {
        return false;
    }
    let b = t.as_bytes();
    if !(b[0].is_ascii_alphabetic() && b[1].is_ascii_alphabetic()) {
        return false;
    }
    if !t[2..11].chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    b[11].is_ascii_digit()
}

fn pick_from_parts(parts: &[String], target: &str) -> Option<String> {
    let first = parts.first()?;
    if normalize_sym(first) != target {
        return None;
    }
    parts
        .iter()
        .map(String::as_str)
        .find(|s| looks_like_isin(s))
        .map(str::to_uppercase)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_like_isin_basic() {
        assert!(looks_like_isin("US0378331005"));
        assert!(looks_like_isin("FR0013410370"));
        assert!(!looks_like_isin("AAPL"));
        assert!(!looks_like_isin("US037833100A")); // last char must be digit
        assert!(!looks_like_isin("000378331005")); // first two must be alpha
    }

    #[test]
    fn normalize_drops_suffix() {
        assert_eq!(normalize_sym("AAPL"), "aapl");
        assert_eq!(normalize_sym("BRK.B"), "brk");
        assert_eq!(normalize_sym("brk-b"), "brk");
        assert_eq!(normalize_sym("7203.T"), "7203");
    }

    #[test]
    fn scan_finds_isin_in_jsonp() {
        let body = r#"mmSuggestDeliver(0, ..., new Array("Apple Inc.", "Stocks", "AAPL|US0378331005|AAPL||AAPL", ...))"#;
        assert_eq!(scan_raw_body(body).as_deref(), Some("US0378331005"));
    }

    #[test]
    fn parse_flat_suggest_matches_symbol() {
        let body = r#"[{"symbol":"AAPL","isin":"US0378331005"}]"#;
        assert_eq!(
            parse_as_flat_suggest(body, "aapl").as_deref(),
            Some("US0378331005"),
        );
    }
}
