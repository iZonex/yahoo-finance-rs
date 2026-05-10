//! Offline ISIN lookup test — replays a recorded suggestion fixture.

mod common;

use yfinance::{Ticker, YfClient};

#[tokio::test]
async fn offline_isin_uses_recorded_fixture() {
    let symbol = "AAPL";
    if !common::fixture_exists("isin_search", symbol, "json") {
        eprintln!(
            "skipping — record the fixture first with \
             `just test-record offline_isin_uses_recorded_fixture`"
        );
        return;
    }

    let server = common::setup_server();
    let mock = common::mock_isin_search(&server, symbol);

    let client = YfClient::builder()
        .isin_base_url(format!(
            "{}/ajax/SearchController_Suggest",
            server.base_url()
        ))
        .max_retries(0)
        .build()
        .unwrap();

    let isin = Ticker::new(&client, symbol)
        .isin()
        .await
        .expect("isin lookup")
        .expect("ISIN should be present in the recorded fixture");

    mock.assert();
    assert_eq!(isin, "US0378331005", "AAPL ISIN");
}
