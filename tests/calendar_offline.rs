//! Offline replay for `Ticker::calendar()`.

mod common;

use yfinance::Ticker;

#[tokio::test]
async fn offline_calendar_uses_recorded_fixture() {
    let symbol = "AAPL";
    if !common::fixture_exists("calendar_quoteSummary", symbol, "json") {
        eprintln!("skipping — calendar fixture missing");
        return;
    }
    let server = common::setup_server();
    let _cookie_crumb = common::mock_cookie_crumb(&server);
    let mock = common::mock_quote_summary(&server, symbol, "calendar_quoteSummary");
    let client = common::build_test_client(&server);

    let cal = Ticker::new(&client, symbol)
        .calendar()
        .await
        .expect("calendar parse");
    mock.assert();
    assert_eq!(cal.earnings_dates.len(), 2);
    assert!(cal.ex_dividend_date.is_some());
    assert!(cal.dividend_date.is_some());
}
