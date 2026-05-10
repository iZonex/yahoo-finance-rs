//! Offline replay for `Ticker::shares()`.

mod common;

use httpmock::Method::GET;
use yfinance::Ticker;

#[tokio::test]
async fn offline_shares_uses_recorded_fixture() {
    let symbol = "AAPL";
    let endpoint = "shares_annualBasicAverageShares";
    if !common::fixture_exists(endpoint, symbol, "json") {
        eprintln!("skipping — shares fixture missing");
        return;
    }
    let server = common::setup_server();
    let _cookie_crumb = common::mock_cookie_crumb(&server);
    let mock = server.mock(|when, then| {
        when.method(GET).path(format!(
            "/ws/fundamentals-timeseries/v1/finance/timeseries/{symbol}"
        ));
        then.status(200)
            .header("content-type", "application/json")
            .body(common::fixture(endpoint, symbol, "json"));
    });
    let client = common::build_test_client(&server);

    let rows = Ticker::new(&client, symbol)
        .shares()
        .await
        .expect("shares parse");
    mock.assert();
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[0].date.to_string(), "2020-09-26");
    assert_eq!(rows[0].shares, 17_352_119_000);
}
