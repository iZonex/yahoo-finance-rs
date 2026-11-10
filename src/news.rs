//! Yahoo Finance company-news feed.
//!
//! Wraps the GraphQL query (POST to `https://nexus-finance-prod.media.yahoo.com/`)
//! behind the news tabs on the ticker page; Yahoo retired the older
//! `finance.yahoo.com/xhr/ncp` in October 2026. Returns recent editorial
//! articles, the full feed including syndicated items, or only press
//! releases — pick via [`NewsTab`].

use serde::Deserialize;

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
    fn as_list(self) -> &'static str {
        match self {
            NewsTab::LatestNews => "news",
            NewsTab::All => "all",
            NewsTab::PressReleases => "pressrelease",
        }
    }
}

/// The GraphQL query behind the news list on
/// `finance.yahoo.com/quote/<symbol>/news/`. Assets resolve only when a
/// field takes `$clientContext`; without one Yahoo returns `AssetError`
/// for every item.
const NEWS_QUERY: &str = r#"query FinanceTickerListNews($listInput: LightyearListInput!, $clientContext: ClientContext!, $mlRecsInput: MLRecsInput!, $gqlContext: [GqlContext] = [], $mlRecsFirst: Int) {
  lightyearList: financeTickerList(list_input: $listInput, cc: $clientContext) {
    main: mlRecsStream(mlRecsInput: $mlRecsInput, first: $mlRecsFirst) {
      edges { node { asset(gqlContext: $gqlContext) {
        __typename
        ... on Story { id: uuid title previewUrl(cc: $clientContext) contentAttributes { ...Attrs } }
        ... on Video { id: uuid title previewUrl(cc: $clientContext) contentAttributes { ...Attrs } }
        ... on Outlink { id: uuid title: headline displayTime url provider { displayName } }
      } } }
    }
  }
}
fragment Attrs on ContentAttributes { publishTime canonicalUrl provider { displayName } }"#;

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
    /// Publication time as UNIX seconds. `None` if Yahoo returned no
    /// publish time or it failed RFC3339 parsing.
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
        let slug = format!("list=finance-US-en-US-ticker-{}", self.tab.as_list());
        let symbols = [self.symbol.as_str()];
        // aboutnessWeight 0 ranks by recency alone, so items come back newest
        // first (the news page itself mixes in relevance with 0.05).
        let body = serde_json::json!({
            "operationName": "FinanceTickerListNews",
            "query": NEWS_QUERY,
            "variables": {
                "clientContext": {
                    "device": "desktop", "lang": "en-US", "region": "US", "site": "finance"
                },
                "gqlContext": [{ "listAlias": slug }],
                "listInput": {
                    "disableDedupe": false,
                    "enableBlockedContent": false,
                    "enableQueryTimeLicenseCheck": true,
                    "filterClientContext": false,
                    "getFullList": true,
                    "queryVariables": { "tickerSymbol": symbols },
                    "slug": slug
                },
                "mlRecsInput": {
                    "count": 200,
                    "instance": "FINANCE",
                    "rankInput": { "financeQuoteNewsV2": {
                        "aboutnessWeight": 0,
                        "boostDurationHours": 16,
                        "halfLifeHours": 24,
                        "recencyWeight": 1,
                        "tickerSymbol": symbols
                    } }
                },
                "mlRecsFirst": self.count
            }
        });
        let req = self
            .client
            .news_post("/")
            .header("x-yahoo-cg-client-name", "finance")
            .json(&body);

        let label = format!("news_{}", self.tab.as_list());
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
    #[serde(default, rename = "lightyearList")]
    list: Option<NewsList>,
}

#[derive(Deserialize)]
struct NewsList {
    #[serde(default)]
    main: Option<NewsStream>,
}

#[derive(Deserialize)]
struct NewsStream {
    #[serde(default)]
    edges: Vec<Edge>,
}

#[derive(Deserialize)]
struct Edge {
    #[serde(default)]
    node: Option<Node>,
}

#[derive(Deserialize)]
struct Node {
    #[serde(default)]
    asset: Option<Asset>,
}

/// Items Yahoo cannot serve come back as `AssetError` and are dropped.
#[derive(Deserialize)]
#[serde(tag = "__typename")]
enum Asset {
    Story(Article),
    Video(Article),
    Outlink(Outlink),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct Article {
    id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default, rename = "contentAttributes")]
    attrs: Option<Attrs>,
}

#[derive(Deserialize)]
struct Attrs {
    #[serde(default, rename = "publishTime")]
    publish_time: Option<String>,
    #[serde(default, rename = "canonicalUrl")]
    canonical_url: Option<String>,
    #[serde(default)]
    provider: Option<Provider>,
}

#[derive(Deserialize)]
struct Outlink {
    id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default, rename = "displayTime")]
    display_time: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    provider: Option<Provider>,
}

#[derive(Deserialize)]
struct Provider {
    #[serde(default, rename = "displayName")]
    display_name: Option<String>,
}

fn unix_seconds(rfc3339: Option<&str>) -> Option<i64> {
    rfc3339.and_then(|s| {
        chrono::DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|dt| dt.timestamp())
    })
}

impl NewsEnvelope {
    fn into_articles(self) -> Vec<NewsArticle> {
        let edges = self
            .data
            .and_then(|d| d.list)
            .and_then(|l| l.main)
            .map(|m| m.edges)
            .unwrap_or_default();
        edges
            .into_iter()
            .filter_map(|edge| match edge.node?.asset? {
                Asset::Story(a) | Asset::Video(a) => {
                    let attrs = a.attrs;
                    Some(NewsArticle {
                        uuid: a.id,
                        title: a.title?,
                        published_at: unix_seconds(
                            attrs.as_ref().and_then(|c| c.publish_time.as_deref()),
                        ),
                        link: attrs.as_ref().and_then(|c| c.canonical_url.clone()),
                        publisher: attrs.and_then(|c| c.provider).and_then(|p| p.display_name),
                    })
                }
                Asset::Outlink(o) => Some(NewsArticle {
                    uuid: o.id,
                    title: o.title?,
                    published_at: unix_seconds(o.display_time.as_deref()),
                    link: o.url,
                    publisher: o.provider.and_then(|p| p.display_name),
                }),
                Asset::Other => None,
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
            "data": { "lightyearList": { "main": { "edges": [
                { "node": { "asset": {
                    "__typename": "Story",
                    "id": "abc",
                    "title": "Hello",
                    "contentAttributes": {
                        "publishTime": "2025-05-01T12:00:00Z",
                        "canonicalUrl": "https://example.com/x",
                        "provider": { "displayName": "Reuters" }
                    }
                } } },
                { "node": { "asset": {
                    "__typename": "Outlink",
                    "id": "out",
                    "title": "Elsewhere",
                    "displayTime": "2025-05-01T13:00:00Z",
                    "url": "https://example.com/y",
                    "provider": { "displayName": "Barron's" }
                } } },
                { "node": { "asset": { "__typename": "AssetError" } } },
                { "node": { "asset": null } }
            ] } } }
        });
        let env: NewsEnvelope = serde_json::from_value(body).unwrap();
        let articles = env.into_articles();
        assert_eq!(articles.len(), 2);
        assert_eq!(articles[0].title, "Hello");
        assert_eq!(articles[0].publisher.as_deref(), Some("Reuters"));
        assert_eq!(articles[0].link.as_deref(), Some("https://example.com/x"));
        assert_eq!(articles[0].published_at, Some(1_746_100_800));
        assert_eq!(articles[1].link.as_deref(), Some("https://example.com/y"));
        assert_eq!(articles[1].published_at, Some(1_746_104_400));
    }

    #[test]
    fn empty_envelope_yields_empty_vec() {
        let env: NewsEnvelope = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(env.into_articles().is_empty());
    }
}
