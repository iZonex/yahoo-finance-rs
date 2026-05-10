//! Offline replay for `batch_quotes()`.

mod common;

use httpmock::Method::GET;
use yfinance::batch_quotes;

#[tokio::test]
async fn offline_batch_quotes_uses_recorded_fixture() {
    let label = "MULTI_2";
    if !common::fixture_exists("quote_v7", label, "json") {
        eprintln!("skipping — batch quotes fixture missing");
        return;
    }
    let server = common::setup_server();
    let _cookie_crumb = common::mock_cookie_crumb(&server);
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/v7/finance/quote")
            .query_param("symbols", "AAPL,MSFT");
        then.status(200)
            .header("content-type", "application/json")
            .body(common::fixture("quote_v7", label, "json"));
    });
    let client = common::build_test_client(&server);

    let rows = batch_quotes(&client, &["AAPL", "MSFT"])
        .await
        .expect("batch_quotes parse");
    mock.assert();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].symbol, "AAPL");
    assert_eq!(rows[1].symbol, "MSFT");
    assert!(rows[0].last_price.unwrap() > 100.0);
}
