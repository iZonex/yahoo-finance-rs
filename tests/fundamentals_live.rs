//! Live test for `Ticker::income_stmt` (fundamentals-timeseries).

mod common;

use yfinance::{Ticker, YfClient};

#[tokio::test]
#[ignore = "exercises live Yahoo Finance"]
async fn live_income_stmt_smoke() {
    if !common::live_or_record_enabled() {
        return;
    }

    let client = YfClient::builder().build().unwrap();
    let stmt = Ticker::new(&client, "AAPL")
        .income_stmt()
        .await
        .expect("live income_stmt");

    if !common::is_recording() {
        assert_eq!(stmt.symbol, "AAPL");
    }
}
