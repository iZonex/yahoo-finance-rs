//! Offline replay for `search()` — backed by a real Yahoo response.

mod common;

use yfinance::YfClient;

#[tokio::test]
async fn offline_search_uses_recorded_fixture() {
    let query = "AAPL";
    if !common::fixture_exists("search", query, "json") {
        eprintln!("skipping — search fixture missing");
        return;
    }

    let server = common::setup_server();
    let mock = common::mock_search(&server, query);
    let client = YfClient::builder()
        .base_host(server.base_url())
        .max_retries(0)
        .build()
        .unwrap();

    let result = yfinance::search::search(&client, query)
        .await
        .expect("search parse");
    mock.assert();
    assert_eq!(result.query, query);
    assert!(!result.quotes.is_empty(), "expected at least one quote");
    let aapl = result
        .quotes
        .iter()
        .find(|q| q.symbol == "AAPL")
        .expect("AAPL row");
    assert_eq!(aapl.long_name.as_deref(), Some("Apple Inc."));
    assert_eq!(aapl.sector.as_deref(), Some("Technology"));
    assert!(
        !result.news.is_empty(),
        "fixture has at least one news item"
    );
}
