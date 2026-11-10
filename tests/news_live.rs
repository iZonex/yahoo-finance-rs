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
        // AAPL always has coverage; an empty feed means the endpoint or the
        // parser broke (Yahoo retired the old one with a quiet 404).
        assert!(!articles.is_empty() && articles.len() <= 5);
        assert!(articles.iter().all(|a| a.published_at.is_some()));
    }
}
