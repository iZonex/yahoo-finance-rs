//! Shared helpers for integration tests.
//!
//! Provides:
//! - fixture file loading (mirrors `src/test_fixtures` on the read path);
//! - `httpmock` helpers for the most-used Yahoo endpoints;
//! - `build_test_client` which routes data, crumb, and cookie-prime traffic to a
//!   single mock server;
//! - convenience checks for `YF_LIVE` / `YF_RECORD`.
//!
//! Fixtures live in `tests/fixtures/{endpoint}_{symbol}.{ext}` by default;
//! `YF_FIXDIR` overrides the directory. Recording is performed by the HTTP
//! layer when the crate is built with `--features test-mode` and `YF_RECORD=1`.

#![allow(dead_code)]

use httpmock::{Method::GET, Mock, MockServer};
use std::{fs, path::PathBuf};
use yfinance::test_fixtures::{fixture_path as src_fixture_path, is_recording as src_is_recording};
use yfinance::YfClient;

/// Crumb body returned by `mock_cookie_crumb` — tests can match on it explicitly
/// (e.g. via `query_param("crumb", CRUMB)`) when they need to.
pub const CRUMB: &str = "crumb-value";

#[must_use]
pub fn setup_server() -> MockServer {
    MockServer::start()
}

/// Build a `YfClient` that routes data, crumb, and cookie priming through
/// `server`. Retries are disabled to keep tests deterministic.
#[must_use]
pub fn build_test_client(server: &MockServer) -> YfClient {
    YfClient::builder()
        .base_host(server.base_url())
        .crumb_url(format!("{}/v1/test/getcrumb", server.base_url()))
        .cookie_prime_url(format!("{}/consent", server.base_url()))
        .max_retries(0)
        .build()
        .expect("test client")
}

#[must_use]
pub fn fixture_path(endpoint: &str, symbol: &str, ext: &str) -> PathBuf {
    src_fixture_path(endpoint, symbol, ext)
}

#[must_use]
pub fn fixture_exists(endpoint: &str, symbol: &str, ext: &str) -> bool {
    fixture_path(endpoint, symbol, ext).exists()
}

/// Read fixture contents, panicking with a useful path on failure.
#[must_use]
pub fn fixture(endpoint: &str, symbol: &str, ext: &str) -> String {
    let path = fixture_path(endpoint, symbol, ext);
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()))
}

/// True when the test should hit Yahoo for real (`YF_LIVE=1` or `YF_RECORD=1`).
#[must_use]
pub fn live_or_record_enabled() -> bool {
    std::env::var("YF_LIVE").ok().as_deref() == Some("1") || src_is_recording()
}

/// True when fixtures should be written (`YF_RECORD=1`).
#[must_use]
pub fn is_recording() -> bool {
    src_is_recording()
}

/// Stub the cookie-prime + crumb-fetch round trip.
#[must_use]
pub fn mock_cookie_crumb(server: &'_ MockServer) -> (Mock<'_>, Mock<'_>) {
    let cookie = server.mock(|when, then| {
        when.method(GET).path("/consent");
        then.status(200).header(
            "set-cookie",
            "A=B; Max-Age=315360000; Domain=.yahoo.com; Path=/; Secure; SameSite=None",
        );
    });
    let crumb = server.mock(|when, then| {
        when.method(GET).path("/v1/test/getcrumb");
        then.status(200).body(CRUMB);
    });
    (cookie, crumb)
}

#[must_use]
pub fn mock_history_chart<'a>(server: &'a MockServer, symbol: &'a str) -> Mock<'a> {
    server.mock(|when, then| {
        when.method(GET).path(format!("/v8/finance/chart/{symbol}"));
        then.status(200)
            .header("content-type", "application/json")
            .body(fixture("history_chart", symbol, "json"));
    })
}

#[must_use]
pub fn mock_options_v7<'a>(server: &'a MockServer, symbol: &'a str) -> Mock<'a> {
    server.mock(|when, then| {
        when.method(GET)
            .path(format!("/v7/finance/options/{symbol}"));
        then.status(200)
            .header("content-type", "application/json")
            .body(fixture("options_v7", symbol, "json"));
    })
}

#[must_use]
pub fn mock_quote_v7<'a>(server: &'a MockServer, symbol: &'a str) -> Mock<'a> {
    server.mock(|when, then| {
        when.method(GET)
            .path("/v7/finance/quote")
            .query_param("symbols", symbol);
        then.status(200)
            .header("content-type", "application/json")
            .body(fixture("quote_v7", symbol, "json"));
    })
}

/// Mock `quoteSummary/{symbol}` for any modules combination — useful for
/// `info` / `holders`. The fixture is keyed by `endpoint`/`symbol`.
#[must_use]
pub fn mock_quote_summary<'a>(
    server: &'a MockServer,
    symbol: &'a str,
    endpoint: &'a str,
) -> Mock<'a> {
    server.mock(|when, then| {
        when.method(GET)
            .path(format!("/v10/finance/quoteSummary/{symbol}"));
        then.status(200)
            .header("content-type", "application/json")
            .body(fixture(endpoint, symbol, "json"));
    })
}

#[must_use]
pub fn mock_fundamentals_timeseries<'a>(server: &'a MockServer, symbol: &'a str) -> Mock<'a> {
    server.mock(|when, then| {
        when.method(GET).path(format!(
            "/ws/fundamentals-timeseries/v1/finance/timeseries/{symbol}"
        ));
        then.status(200)
            .header("content-type", "application/json")
            .body(fixture("fundamentals_timeseries", symbol, "json"));
    })
}

#[must_use]
pub fn mock_news<'a>(server: &'a MockServer, symbol: &'a str, query_ref: &'a str) -> Mock<'a> {
    let endpoint = format!("news_{query_ref}");
    server.mock(move |when, then| {
        when.method(httpmock::Method::POST)
            .path("/xhr/ncp")
            .query_param("queryRef", query_ref);
        then.status(200)
            .header("content-type", "application/json")
            .body(fixture(&endpoint, symbol, "json"));
    })
}

#[must_use]
pub fn mock_isin_search<'a>(server: &'a MockServer, symbol: &'a str) -> Mock<'a> {
    server.mock(|when, then| {
        when.method(GET)
            .path("/ajax/SearchController_Suggest")
            .query_param("query", symbol);
        then.status(200)
            .header("content-type", "text/plain")
            .body(fixture("isin_search", symbol, "json"));
    })
}

#[must_use]
pub fn mock_search<'a>(server: &'a MockServer, query: &'a str) -> Mock<'a> {
    let label = yfinance::test_fixtures::safe_label(query);
    server.mock(move |when, then| {
        when.method(GET)
            .path("/v1/finance/search")
            .query_param("q", query.to_string());
        then.status(200)
            .header("content-type", "application/json")
            .body(fixture("search", &label, "json"));
    })
}
