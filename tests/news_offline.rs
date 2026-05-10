//! Offline replay for `Ticker::news` (Yahoo `xhr/ncp`).

mod common;

use yfinance::{NewsTab, Ticker, YfClient};

#[tokio::test]
async fn offline_news_uses_recorded_fixture() {
    let symbol = "AAPL";
    if !common::fixture_exists("news_latestNews", symbol, "json") {
        eprintln!(
            "skipping — record the fixture first with \
             `just test-record offline_news_uses_recorded_fixture`"
        );
        return;
    }

    let server = common::setup_server();
    let mock = common::mock_news(&server, symbol, "latestNews");

    let client = YfClient::builder()
        .news_base_url(server.base_url())
        .max_retries(0)
        .build()
        .unwrap();

    let articles = Ticker::new(&client, symbol)
        .news()
        .tab(NewsTab::LatestNews)
        .fetch()
        .await
        .expect("news parse");

    mock.assert();
    // We don't pin a count — Yahoo's feed is volatile — but a recorded payload
    // should yield at least one parsed item.
    assert!(!articles.is_empty(), "expected at least one news article");
}
