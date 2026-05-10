//! Live tests for the analysis endpoints. `#[ignore]` — run with
//! `just test-live` or `just test-record`.

mod common;

use yfinance::{Ticker, YfClient};

#[tokio::test]
#[ignore = "exercises live Yahoo Finance"]
async fn live_recommendations() {
    if !common::live_or_record_enabled() {
        return;
    }
    let client = YfClient::builder().build().unwrap();
    let _ = Ticker::new(&client, "AAPL").recommendations().await;
}

#[tokio::test]
#[ignore = "exercises live Yahoo Finance"]
async fn live_recommendations_summary() {
    if !common::live_or_record_enabled() {
        return;
    }
    let client = YfClient::builder().build().unwrap();
    let _ = Ticker::new(&client, "MSFT").recommendations_summary().await;
}

#[tokio::test]
#[ignore = "exercises live Yahoo Finance"]
async fn live_upgrades_downgrades() {
    if !common::live_or_record_enabled() {
        return;
    }
    let client = YfClient::builder().build().unwrap();
    let _ = Ticker::new(&client, "GOOGL").upgrades_downgrades().await;
}

#[tokio::test]
#[ignore = "exercises live Yahoo Finance"]
async fn live_price_target() {
    if !common::live_or_record_enabled() {
        return;
    }
    let client = YfClient::builder().build().unwrap();
    let _ = Ticker::new(&client, "MSFT").price_target().await;
}

#[tokio::test]
#[ignore = "exercises live Yahoo Finance"]
async fn live_earnings_trend() {
    if !common::live_or_record_enabled() {
        return;
    }
    let client = YfClient::builder().build().unwrap();
    let _ = Ticker::new(&client, "AAPL").earnings_trend().await;
}
