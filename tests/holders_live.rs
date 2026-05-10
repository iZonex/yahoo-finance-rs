//! Live test for `Ticker::holders`.

mod common;

use yfinance::{Ticker, YfClient};

#[tokio::test]
#[ignore = "exercises live Yahoo Finance"]
async fn live_holders_smoke() {
    if !common::live_or_record_enabled() {
        return;
    }

    let client = YfClient::builder().build().unwrap();
    let h = Ticker::new(&client, "AAPL")
        .holders()
        .await
        .expect("live holders");

    if !common::is_recording() {
        assert_eq!(h.symbol, "AAPL");
    }
}
