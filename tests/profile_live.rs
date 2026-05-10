//! Live test for `Ticker::profile`.

mod common;

use yfinance::{Ticker, YfClient};

#[tokio::test]
#[ignore = "exercises live Yahoo Finance"]
async fn live_profile_smoke() {
    if !common::live_or_record_enabled() {
        return;
    }
    let client = YfClient::builder().build().unwrap();
    let _ = Ticker::new(&client, "AAPL").profile().await;
}
