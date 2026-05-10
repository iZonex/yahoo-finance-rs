//! Offline replay for `Ticker::income_stmt` (fundamentals-timeseries).

mod common;

use yfinance::Ticker;

#[tokio::test]
async fn offline_income_stmt_uses_recorded_fixture() {
    let symbol = "AAPL";
    if !common::fixture_exists("fundamentals_timeseries", symbol, "json") {
        eprintln!(
            "skipping — record the fixture first with \
             `just test-record offline_income_stmt_uses_recorded_fixture`"
        );
        return;
    }

    let server = common::setup_server();
    let _cookie_crumb = common::mock_cookie_crumb(&server);
    let mock = common::mock_fundamentals_timeseries(&server, symbol);
    let client = common::build_test_client(&server);

    let stmt = Ticker::new(&client, symbol)
        .income_stmt()
        .await
        .expect("fundamentals parse");

    mock.assert();
    assert_eq!(stmt.symbol, symbol);
}
