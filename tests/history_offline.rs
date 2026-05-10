//! Replays a recorded chart fixture through `httpmock` and verifies parsing.
//!
//! The fixture is captured by the live test in `history_live.rs` when run with
//! `YF_RECORD=1` (and the `test-mode` cargo feature). Run `just test-record`
//! the first time, then `just test-offline` for fast iteration.

mod common;

use yfinance::{Interval, Ticker};

#[tokio::test]
async fn offline_history_uses_recorded_fixture() {
    let symbol = "AAPL";
    if !common::fixture_exists("history_chart", symbol, "json") {
        eprintln!(
            "skipping offline test — record the fixture first with \
             `just test-record offline_history_uses_recorded_fixture` \
             (or run with YF_RECORD=1 against a live network)"
        );
        return;
    }

    let server = common::setup_server();
    let mock = common::mock_history_chart(&server, symbol);
    let client = common::build_test_client(&server);

    let history = Ticker::new(&client, symbol)
        .history()
        .interval(Interval::D1)
        .auto_adjust(false)
        .fetch()
        .await
        .expect("history parse");

    mock.assert();
    assert_eq!(history.symbol, symbol);
    assert!(!history.rows.is_empty(), "fixture produced no rows");
    assert_eq!(history.currency.as_deref(), Some("USD"));
    assert_eq!(history.exchange_name.as_deref(), Some("NMS"));
    assert_eq!(history.timezone.as_deref(), Some("America/New_York"));
    // Sanity: bars are chronologically ordered, OHLC ranges are well-formed.
    let mut prev = 0;
    for r in &history.rows {
        let ts = r.timestamp.timestamp();
        assert!(ts > prev, "timestamps should be ascending");
        prev = ts;
        assert!(r.high >= r.low, "high {} < low {}", r.high, r.low);
        assert!(
            r.open > 0.0 && r.close > 0.0,
            "non-positive prices in real Yahoo data shouldn't happen"
        );
    }
}
