//! Yahoo Finance company-news feed.
//!
//! Wraps `https://finance.yahoo.com/xhr/ncp` (POST), which powers the news
//! tabs on the ticker page. Returns recent editorial articles, the full feed
//! including syndicated items, or only press releases — pick via [`NewsTab`].

use serde::{Deserialize, Serialize};

use crate::client::YfClient;
use crate::error::{Error, Result};

/// Which news category to fetch. Mirrors the tabs on the Yahoo ticker page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NewsTab {
    /// Editorial / latest news (default).
    #[default]
    LatestNews,
    /// Editorial + syndicated stories.
    All,
    /// Press releases only.
    PressReleases,
}

impl NewsTab {
    fn as_query_ref(self) -> &'static str {
        match self {
            NewsTab::LatestNews => "latestNews",
            NewsTab::All => "newsAll",
            NewsTab::PressReleases => "pressRelease",
        }
    }
}

/// One news article.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsArticle {
    /// Internal Yahoo identifier (also called `uuid` upstream).
    pub uuid: String,
    /// Headline.
    pub title: String,
    /// Publisher display name (`Reuters`, `Bloomberg`, …).
    pub publisher: Option<String>,
    /// Canonical URL to the article.
    pub link: Option<String>,
    /// Publication time as UNIX seconds. `None` if Yahoo returned no `pubDate`
    /// or it failed RFC3339 parsing.
    pub published_at: Option<i64>,
}

/// Builder for the news endpoint. Construct via [`Ticker::news`].
///
/// [`Ticker::news`]: crate::ticker::Ticker::news
#[derive(Debug, Clone)]
pub struct NewsBuilder {
    client: YfClient,
    symbol: String,
    count: u32,
    tab: NewsTab,
    cache_mode: crate::client::CacheMode,
    retry_override: Option<crate::client::RetryConfig>,
}

impl NewsBuilder {
    pub(crate) fn new(client: &YfClient, symbol: impl Into<String>) -> Self {
        Self {
            client: client.clone(),
            symbol: symbol.into(),
            count: 10,
            tab: NewsTab::default(),
            cache_mode: crate::client::CacheMode::Use,
            retry_override: None,
        }
    }

    /// Maximum articles to request. Default: 10.
    pub fn count(mut self, n: u32) -> Self {
        self.count = n;
        self
    }

    /// News category (see [`NewsTab`]). Default: latest editorial news.
    pub fn tab(mut self, tab: NewsTab) -> Self {
        self.tab = tab;
        self
    }

    /// Cache override for this request. POST endpoints aren't cached today,
    /// so this is currently a no-op for news; the setter exists for API
    /// parity and forward compatibility.
    pub fn cache_mode(mut self, mode: crate::client::CacheMode) -> Self {
        self.cache_mode = mode;
        self
    }

    /// Per-request retry policy override. Currently advisory — the news POST
    /// uses the client-wide policy.
    pub fn retry(mut self, cfg: crate::client::RetryConfig) -> Self {
        self.retry_override = Some(cfg);
        self
    }

    /// Execute the request and return parsed articles.
    pub async fn fetch(self) -> Result<Vec<NewsArticle>> {
        #[derive(Serialize)]
        struct ServiceConfig<'a> {
            #[serde(rename = "snippetCount")]
            snippet_count: u32,
            s: &'a [&'a str],
        }
        #[derive(Serialize)]
        struct Payload<'a> {
            #[serde(rename = "serviceConfig")]
            service_config: ServiceConfig<'a>,
        }

        let path = format!(
            "/xhr/ncp?queryRef={}&serviceKey=ncp_fin",
            self.tab.as_query_ref()
        );
        let body = Payload {
            service_config: ServiceConfig {
                snippet_count: self.count,
                s: &[&self.symbol],
            },
        };
        let req = self.client.news_post(&path).json(&body);

        let label = format!("news_{}", self.tab.as_query_ref());
        let Some(text) = self
            .client
            .send_text_recorded(req, Some((&label, &self.symbol)))
            .await?
        else {
            return Ok(Vec::new());
        };
        let envelope: NewsEnvelope = serde_json::from_str(&text).map_err(Error::from)?;
        Ok(envelope.into_articles())
    }
}

#[derive(Deserialize)]
struct NewsEnvelope {
    #[serde(default)]
    data: Option<NewsData>,
}

#[derive(Deserialize)]
struct NewsData {
    #[serde(default, rename = "tickerStream")]
    ticker_stream: Option<TickerStream>,
}

#[derive(Deserialize)]
struct TickerStream {
    #[serde(default)]
    stream: Option<Vec<StreamItem>>,
}

#[derive(Deserialize)]
struct StreamItem {
    id: String,
    #[serde(default)]
    content: Option<Content>,
    /// When present, the item is a sponsored placement — drop it.
    #[serde(default)]
    ad: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct Content {
    #[serde(default)]
    title: Option<String>,
    #[serde(default, rename = "pubDate")]
    pub_date: Option<String>,
    #[serde(default)]
    provider: Option<Provider>,
    #[serde(default, rename = "canonicalUrl")]
    canonical_url: Option<CanonicalUrl>,
}

#[derive(Deserialize)]
struct Provider {
    #[serde(default, rename = "displayName")]
    display_name: Option<String>,
}

#[derive(Deserialize)]
struct CanonicalUrl {
    #[serde(default)]
    url: Option<String>,
}

impl NewsEnvelope {
    fn into_articles(self) -> Vec<NewsArticle> {
        let stream = self
            .data
            .and_then(|d| d.ticker_stream)
            .and_then(|t| t.stream)
            .unwrap_or_default();
        stream
            .into_iter()
            .filter_map(|item| {
                if item.ad.is_some() {
                    return None;
                }
                let content = item.content?;
                let title = content.title?;
                let published_at = content.pub_date.as_deref().and_then(|s| {
                    chrono::DateTime::parse_from_rfc3339(s)
                        .ok()
                        .map(|dt| dt.timestamp())
                });
                Some(NewsArticle {
                    uuid: item.id,
                    title,
                    publisher: content.provider.and_then(|p| p.display_name),
                    link: content.canonical_url.and_then(|u| u.url),
                    published_at,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_envelope() {
        let body = serde_json::json!({
            "data": {
                "tickerStream": {
                    "stream": [
                        {
                            "id": "abc",
                            "content": {
                                "title": "Hello",
                                "pubDate": "2025-05-01T12:00:00Z",
                                "provider": { "displayName": "Reuters" },
                                "canonicalUrl": { "url": "https://example.com/x" }
                            }
                        },
                        {
                            "id": "ad1",
                            "ad": { "anything": true }
                        }
                    ]
                }
            }
        });
        let env: NewsEnvelope = serde_json::from_value(body).unwrap();
        let articles = env.into_articles();
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].title, "Hello");
        assert_eq!(articles[0].publisher.as_deref(), Some("Reuters"));
        assert_eq!(articles[0].published_at, Some(1_746_100_800));
    }

    #[test]
    fn empty_envelope_yields_empty_vec() {
        let env: NewsEnvelope = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(env.into_articles().is_empty());
    }
}
