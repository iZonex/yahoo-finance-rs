//! End-to-end test of the chart endpoint against an `httpmock` server.
//!
//! Verifies the wiring:
//!   1. The history request hits `/v8/finance/chart/AAPL` with the right query.
//!   2. We parse the chart response (timestamps, OHLCV, dividends).

use httpmock::prelude::*;
use yfinance::{Interval, Period, Ticker, YfClient};

#[tokio::test]
async fn history_round_trip() {
    let server = MockServer::start_async().await;

    let chart = serde_json::json!({
        "chart": {
            "result": [{
                "meta": {
                    "symbol": "AAPL",
                    "currency": "USD",
                    "exchangeName": "NMS",
                    "exchangeTimezoneName": "America/New_York"
                },
                "timestamp": [1_700_000_000_i64, 1_700_086_400_i64],
                "indicators": {
                    "quote": [{
                        "open":   [100.0, 102.0],
                        "high":   [101.5, 103.5],
                        "low":    [ 99.5, 101.0],
                        "close":  [101.0, 103.0],
                        "volume": [1000000, 1500000]
                    }],
                    "adjclose": [{ "adjclose": [101.0, 103.0] }]
                },
                "events": {
                    "dividends": {
                        "1700000000": { "date": 1_700_000_000_i64, "amount": 0.25 }
                    }
                }
            }],
            "error": null
        }
    });

    let mock = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/v8/finance/chart/AAPL")
                .query_param("interval", "1d")
                .query_param("range", "1mo");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(chart);
        })
        .await;

    let client = YfClient::builder()
        .base_host(server.base_url())
        .max_retries(0)
        .build()
        .unwrap();

    let t = Ticker::new(&client, "AAPL");
    let h = t
        .history()
        .period(Period::M1)
        .interval(Interval::D1)
        .auto_adjust(false)
        .fetch()
        .await
        .expect("history");

    assert_eq!(h.symbol, "AAPL");
    assert_eq!(h.rows.len(), 2);
    assert!((h.rows[0].open - 100.0).abs() < 1e-9);
    assert_eq!(h.actions.len(), 1);
    mock.assert_async().await;
}

#[tokio::test]
async fn yahoo_error_propagates_through_pipeline() {
    let server = MockServer::start_async().await;
    let body = serde_json::json!({
        "chart": {
            "result": [],
            "error": { "code": "Not Found", "description": "No data" }
        }
    });
    let _mock = server
        .mock_async(|when, then| {
            when.method(GET).path("/v8/finance/chart/BOGUS");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(body);
        })
        .await;

    let client = YfClient::builder()
        .base_host(server.base_url())
        .max_retries(0)
        .build()
        .unwrap();

    let err = Ticker::new(&client, "BOGUS")
        .history()
        .period(Period::D5)
        .fetch()
        .await
        .expect_err("should error");

    let msg = err.to_string();
    assert!(msg.contains("Not Found"), "unexpected: {msg}");
}
