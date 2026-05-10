//! Offline replay for `Ticker::info` (quoteSummary modules).

mod common;

use yfinance::Ticker;

#[tokio::test]
async fn offline_info_uses_recorded_fixture() {
    let symbol = "AAPL";
    if !common::fixture_exists("info_quoteSummary", symbol, "json") {
        eprintln!(
            "skipping — record the fixture first with \
             `just test-record offline_info_uses_recorded_fixture`"
        );
        return;
    }

    let server = common::setup_server();
    let _cookie_crumb = common::mock_cookie_crumb(&server);
    let mock = common::mock_quote_summary(&server, symbol, "info_quoteSummary");
    let client = common::build_test_client(&server);

    let info = Ticker::new(&client, symbol)
        .info()
        .await
        .expect("info parse");

    mock.assert();
    assert_eq!(info.symbol, symbol);
}
