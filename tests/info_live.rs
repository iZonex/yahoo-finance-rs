//! Live test for `Ticker::info` (`/v10/finance/quoteSummary`).

mod common;

use yfinance::{Ticker, YfClient};

#[tokio::test]
#[ignore = "exercises live Yahoo Finance"]
async fn live_info_smoke() {
    if !common::live_or_record_enabled() {
        return;
    }

    let client = YfClient::builder().build().unwrap();
    let info = Ticker::new(&client, "AAPL")
        .info()
        .await
        .expect("live info");

    if !common::is_recording() {
        assert_eq!(info.symbol, "AAPL");
    }
}
