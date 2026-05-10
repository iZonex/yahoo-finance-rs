//! Offline replay for `Ticker::fast_info` (`/v7/finance/quote`).

mod common;

use yfinance::Ticker;

#[tokio::test]
async fn offline_fast_info_uses_recorded_fixture() {
    let symbol = "AAPL";
    if !common::fixture_exists("quote_v7", symbol, "json") {
        eprintln!(
            "skipping — record the fixture first with \
             `just test-record offline_fast_info_uses_recorded_fixture`"
        );
        return;
    }

    let server = common::setup_server();
    let _cookie_crumb = common::mock_cookie_crumb(&server);
    let mock = common::mock_quote_v7(&server, symbol);
    let client = common::build_test_client(&server);

    let info = Ticker::new(&client, symbol)
        .fast_info()
        .await
        .expect("fast_info parse");

    mock.assert();
    assert_eq!(info.symbol, symbol);
}
