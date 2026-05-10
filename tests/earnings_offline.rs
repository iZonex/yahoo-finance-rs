//! Offline replay for `Ticker::earnings()`.

mod common;

use yfinance::Ticker;

#[tokio::test]
async fn offline_earnings_uses_recorded_fixture() {
    let symbol = "AAPL";
    if !common::fixture_exists("earnings_quoteSummary", symbol, "json") {
        eprintln!("skipping — earnings fixture missing");
        return;
    }
    let server = common::setup_server();
    let _cookie_crumb = common::mock_cookie_crumb(&server);
    let mock = common::mock_quote_summary(&server, symbol, "earnings_quoteSummary");
    let client = common::build_test_client(&server);

    let e = Ticker::new(&client, symbol)
        .earnings()
        .await
        .expect("earnings parse");
    mock.assert();
    assert_eq!(e.symbol, symbol);
    assert_eq!(e.yearly.len(), 4);
    assert_eq!(e.quarterly.len(), 4);
    assert_eq!(e.eps_quarterly.len(), 4);
    assert_eq!(e.yearly[0].year, 2021);
    assert!(e.eps_quarterly[0].actual.unwrap() > 1.5);
}
