//! Live test for `Ticker::sustainability`.
//!
//! Yahoo has been deprecating this endpoint — a successful response is no
//! longer guaranteed even for large-cap symbols. The smoke test allows
//! `Ok(None)` to pass.

mod common;

use yfinance::{Ticker, YfClient};

#[tokio::test]
#[ignore = "exercises live Yahoo Finance"]
async fn live_esg_smoke() {
    if !common::live_or_record_enabled() {
        return;
    }

    let client = YfClient::builder().build().unwrap();
    let res = Ticker::new(&client, "AAPL").sustainability().await;

    if !common::is_recording() {
        // Allow either Some(...) or Ok(None) — both indicate a parsed response.
        assert!(res.is_ok(), "expected Ok, got {res:?}");
    }
}
