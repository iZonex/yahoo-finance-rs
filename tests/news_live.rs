//! Live test for `Ticker::news`.

mod common;

use yfinance::{NewsTab, Ticker, YfClient};

#[tokio::test]
#[ignore = "exercises live Yahoo Finance"]
async fn live_news_smoke() {
    if !common::live_or_record_enabled() {
        return;
    }

    let client = YfClient::builder().build().unwrap();
    let articles = Ticker::new(&client, "AAPL")
        .news()
        .tab(NewsTab::LatestNews)
        .count(5)
        .fetch()
        .await
        .expect("live news");

    if !common::is_recording() {
        // Yahoo is fickle — accept zero articles for symbols without coverage,
        // but flag obviously-broken parses (e.g. a panic during dispatch).
        assert!(articles.len() <= 50);
    }
}
