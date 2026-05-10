//! Offline replay for the analysis quoteSummary endpoints.
//!
//! Five tests, one per endpoint. Each gracefully skips when its fixture is
//! missing, so you can record them piecemeal.

mod common;

use yfinance::Ticker;

fn label(modules: &str) -> String {
    format!("analysis_{modules}")
}

#[tokio::test]
async fn offline_recommendations() {
    let symbol = "AAPL";
    let endpoint = label("recommendationTrend");
    if !common::fixture_exists(&endpoint, symbol, "json") {
        eprintln!("skipping — record `just test-record offline_recommendations` first");
        return;
    }
    let server = common::setup_server();
    let _cookie_crumb = common::mock_cookie_crumb(&server);
    let mock = common::mock_quote_summary(&server, symbol, &endpoint);
    let client = common::build_test_client(&server);

    let rows = Ticker::new(&client, symbol)
        .recommendations()
        .await
        .expect("parse");
    mock.assert();
    assert!(rows.iter().all(|r| !r.period.is_empty()));
}

#[tokio::test]
async fn offline_recommendations_summary() {
    let symbol = "MSFT";
    let endpoint = label("recommendationTrend-financialData");
    if !common::fixture_exists(&endpoint, symbol, "json") {
        eprintln!("skipping — record `just test-record offline_recommendations_summary` first");
        return;
    }
    let server = common::setup_server();
    let _cookie_crumb = common::mock_cookie_crumb(&server);
    let mock = common::mock_quote_summary(&server, symbol, &endpoint);
    let client = common::build_test_client(&server);

    let summary = Ticker::new(&client, symbol)
        .recommendations_summary()
        .await
        .expect("parse");
    mock.assert();
    // mean, when present, is in [1.0, 5.0]
    if let Some(m) = summary.mean {
        assert!((1.0..=5.0).contains(&m), "unexpected mean: {m}");
    }
}

#[tokio::test]
async fn offline_upgrades_downgrades() {
    let symbol = "GOOGL";
    let endpoint = label("upgradeDowngradeHistory");
    if !common::fixture_exists(&endpoint, symbol, "json") {
        eprintln!("skipping — record `just test-record offline_upgrades_downgrades` first");
        return;
    }
    let server = common::setup_server();
    let _cookie_crumb = common::mock_cookie_crumb(&server);
    let mock = common::mock_quote_summary(&server, symbol, &endpoint);
    let client = common::build_test_client(&server);

    let rows = Ticker::new(&client, symbol)
        .upgrades_downgrades()
        .await
        .expect("parse");
    mock.assert();
    // Sorted ascending by timestamp.
    let ts: Vec<i64> = rows.iter().map(|r| r.timestamp).collect();
    assert!(ts.windows(2).all(|w| w[0] <= w[1]));
}

#[tokio::test]
async fn offline_price_target() {
    let symbol = "MSFT";
    let endpoint = label("financialData");
    if !common::fixture_exists(&endpoint, symbol, "json") {
        eprintln!("skipping — record `just test-record offline_price_target` first");
        return;
    }
    let server = common::setup_server();
    let _cookie_crumb = common::mock_cookie_crumb(&server);
    let mock = common::mock_quote_summary(&server, symbol, &endpoint);
    let client = common::build_test_client(&server);

    let target = Ticker::new(&client, symbol)
        .price_target()
        .await
        .expect("parse");
    mock.assert();
    if let Some(low) = target.low {
        if let Some(high) = target.high {
            assert!(low <= high);
        }
    }
}

#[tokio::test]
async fn offline_earnings_trend() {
    let symbol = "AAPL";
    let endpoint = label("earningsTrend");
    if !common::fixture_exists(&endpoint, symbol, "json") {
        eprintln!("skipping — record `just test-record offline_earnings_trend` first");
        return;
    }
    let server = common::setup_server();
    let _cookie_crumb = common::mock_cookie_crumb(&server);
    let mock = common::mock_quote_summary(&server, symbol, &endpoint);
    let client = common::build_test_client(&server);

    let rows = Ticker::new(&client, symbol)
        .earnings_trend()
        .await
        .expect("parse");
    mock.assert();
    assert!(rows.iter().all(|r| !r.period.is_empty()));
}
