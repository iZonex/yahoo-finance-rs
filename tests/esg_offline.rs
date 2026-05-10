//! Offline replay for `Ticker::sustainability` (ESG quoteSummary module).

mod common;

use yfinance::Ticker;

#[tokio::test]
async fn offline_esg_uses_recorded_fixture() {
    let symbol = "AAPL";
    if !common::fixture_exists("esg_quoteSummary", symbol, "json") {
        eprintln!(
            "skipping — record the fixture first with \
             `just test-record offline_esg_uses_recorded_fixture` \
             (note: Yahoo has been retiring this endpoint)"
        );
        return;
    }

    let server = common::setup_server();
    let _cookie_crumb = common::mock_cookie_crumb(&server);
    let mock = common::mock_quote_summary(&server, symbol, "esg_quoteSummary");
    let client = common::build_test_client(&server);

    let result = Ticker::new(&client, symbol)
        .sustainability()
        .await
        .expect("esg parse");

    mock.assert();
    if let Some(summary) = result {
        assert_eq!(summary.symbol, symbol);
    } else {
        eprintln!("ESG endpoint returned an empty payload — that's expected for some symbols");
    }
}
