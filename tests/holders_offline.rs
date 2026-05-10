//! Offline replay for `Ticker::holders` (quoteSummary holders modules).

mod common;

use yfinance::Ticker;

#[tokio::test]
async fn offline_holders_uses_recorded_fixture() {
    let symbol = "AAPL";
    if !common::fixture_exists("holders_quoteSummary", symbol, "json") {
        eprintln!(
            "skipping — record the fixture first with \
             `just test-record offline_holders_uses_recorded_fixture`"
        );
        return;
    }

    let server = common::setup_server();
    let _cookie_crumb = common::mock_cookie_crumb(&server);
    let mock = common::mock_quote_summary(&server, symbol, "holders_quoteSummary");
    let client = common::build_test_client(&server);

    let h = Ticker::new(&client, symbol)
        .holders()
        .await
        .expect("holders parse");

    mock.assert();
    assert_eq!(h.symbol, symbol);
}
