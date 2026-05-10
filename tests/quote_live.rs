//! Live tests for `/v7/finance/quote`. See `history_live.rs` for the pattern.

mod common;

use yfinance::{Ticker, YfClient};

#[tokio::test]
#[ignore = "exercises live Yahoo Finance"]
async fn live_quote_smoke() {
    if !common::live_or_record_enabled() {
        return;
    }

    let client = YfClient::builder().build().unwrap();
    let info = Ticker::new(&client, "AAPL")
        .fast_info()
        .await
        .expect("live fast_info");

    if !common::is_recording() {
        assert_eq!(info.symbol, "AAPL");
    }
}
