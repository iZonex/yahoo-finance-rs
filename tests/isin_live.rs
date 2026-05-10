//! Live ISIN lookup against `markets.businessinsider.com`.
//!
//! Note: this endpoint is not Yahoo and is rarely rate-limited, so unlike the
//! Yahoo live tests it usually works first try.

mod common;

use yfinance::{Ticker, YfClient};

#[tokio::test]
#[ignore = "exercises the live Business Insider suggest endpoint"]
async fn live_isin_smoke() {
    if !common::live_or_record_enabled() {
        return;
    }

    let client = YfClient::builder().build().unwrap();
    let isin = Ticker::new(&client, "AAPL")
        .isin()
        .await
        .expect("live isin");

    if !common::is_recording() {
        assert_eq!(isin.as_deref(), Some("US0378331005"));
    }
}
