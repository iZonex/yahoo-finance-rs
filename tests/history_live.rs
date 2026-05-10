//! Live tests against the real Yahoo Finance chart endpoint.
//!
//! These are `#[ignore]` so plain `cargo test` skips them. They run when:
//! - `YF_LIVE=1` is set — verifies the live API still parses;
//! - `YF_RECORD=1` is set together with `--features test-mode` — the HTTP
//!   layer writes the response to `tests/fixtures/history_chart_<SYMBOL>.json`
//!   so the offline test can replay it.

mod common;

use yfinance::{Ticker, YfClient};

#[tokio::test]
#[ignore = "exercises live Yahoo Finance"]
async fn live_history_smoke() {
    if !common::live_or_record_enabled() {
        return;
    }

    let client = YfClient::builder().build().unwrap();
    let h = Ticker::new(&client, "AAPL")
        .history()
        .fetch()
        .await
        .expect("live history");

    if !common::is_recording() {
        assert!(!h.rows.is_empty());
        assert!(h.rows[0].close > 0.0);
    }
}

#[tokio::test]
#[ignore = "exercises live Yahoo Finance"]
async fn live_history_for_record() {
    if !common::is_recording() {
        return;
    }

    let client = YfClient::builder().build().unwrap();
    for sym in ["AAPL", "MSFT"] {
        let _ = Ticker::new(&client, sym).history().fetch().await;
    }
}
