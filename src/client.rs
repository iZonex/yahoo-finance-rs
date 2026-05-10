//! HTTP client with cookie + crumb session management.
//!
//! Yahoo's data endpoints require a `crumb` token tied to a session cookie that
//! must be acquired in advance. [`YfClient`] handles the full handshake (basic
//! cookie strategy with CSRF consent fallback), retries transient failures, and
//! caches the crumb for the configured TTL.

use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use rand::seq::SliceRandom;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, REFERER, USER_AGENT};
use reqwest::{Client, Response, StatusCode};
use serde::de::DeserializeOwned;
use tokio::sync::Mutex;
use tokio::time::sleep;

use crate::error::{Error, Result};

/// Default User-Agent strings rotated per session.
///
/// Modern desktop Chrome on Windows/macOS — Yahoo blocks obvious bots, but is
/// permissive of typical browsers.
const USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:128.0) Gecko/20100101 Firefox/128.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15",
];

/// Yahoo Finance host for primary data endpoints.
pub(crate) const QUERY1_HOST: &str = "https://query1.finance.yahoo.com";
/// Alternate host. Yahoo treats query1/query2 as interchangeable; some endpoints
/// favor query2 historically.
pub(crate) const QUERY2_HOST: &str = "https://query2.finance.yahoo.com";

/// Pulled from yfinance's behavior — crumbs typically remain valid for ~1 hour.
const CRUMB_TTL: Duration = Duration::from_secs(60 * 30);

/// Builder for [`YfClient`].
#[derive(Debug)]
pub struct YfClientBuilder {
    user_agent: Option<String>,
    timeout: Duration,
    base_query_host: String,
    max_retries: u32,
    retry_backoff: Duration,
    underlying: Option<Client>,
}

impl Default for YfClientBuilder {
    fn default() -> Self {
        Self {
            user_agent: None,
            timeout: Duration::from_secs(30),
            base_query_host: QUERY2_HOST.to_string(),
            max_retries: 3,
            retry_backoff: Duration::from_millis(500),
            underlying: None,
        }
    }
}

impl YfClientBuilder {
    /// Override the User-Agent header. By default a desktop browser UA is picked at random.
    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = Some(ua.into());
        self
    }

    /// Per-request timeout. Default: 30 seconds.
    pub fn timeout(mut self, dur: Duration) -> Self {
        self.timeout = dur;
        self
    }

    /// Maximum retries on transient errors (5xx, timeouts, 429 with backoff).
    pub fn max_retries(mut self, n: u32) -> Self {
        self.max_retries = n;
        self
    }

    /// Initial backoff between retries (doubled each attempt).
    pub fn retry_backoff(mut self, dur: Duration) -> Self {
        self.retry_backoff = dur;
        self
    }

    /// Override the base host used for data requests. Useful for tests against
    /// a mock server.
    pub fn base_host(mut self, host: impl Into<String>) -> Self {
        self.base_query_host = host.into();
        self
    }

    /// Inject a pre-configured `reqwest::Client` (e.g. with custom proxy/TLS).
    ///
    /// The client must have a cookie store enabled.
    pub fn with_client(mut self, client: Client) -> Self {
        self.underlying = Some(client);
        self
    }

    /// Build the client.
    pub fn build(self) -> Result<YfClient> {
        let user_agent = self.user_agent.unwrap_or_else(|| {
            USER_AGENTS
                .choose(&mut rand::thread_rng())
                .copied()
                .unwrap_or(USER_AGENTS[0])
                .to_string()
        });

        let inner = if let Some(c) = self.underlying {
            c
        } else {
            Client::builder()
                .cookie_store(true)
                .gzip(true)
                .timeout(self.timeout)
                .user_agent(&user_agent)
                .build()?
        };

        Ok(YfClient {
            inner: Arc::new(Inner {
                http: inner,
                user_agent,
                base_host: self.base_query_host,
                max_retries: self.max_retries,
                retry_backoff: self.retry_backoff,
                crumb_state: Mutex::new(()),
                crumb: RwLock::new(None),
            }),
        })
    }
}

/// Asynchronous HTTP client that authenticates against Yahoo Finance and
/// performs JSON requests with retry/backoff.
///
/// Cheap to clone — internally an `Arc`.
#[derive(Clone, Debug)]
pub struct YfClient {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    http: Client,
    user_agent: String,
    base_host: String,
    max_retries: u32,
    retry_backoff: Duration,
    /// Serializes crumb refreshes so concurrent callers share one handshake.
    crumb_state: Mutex<()>,
    crumb: RwLock<Option<CrumbState>>,
}

#[derive(Debug, Clone)]
struct CrumbState {
    crumb: String,
    fetched_at: std::time::Instant,
}

impl YfClient {
    /// Build a client with default settings.
    pub fn new() -> Result<Self> {
        YfClientBuilder::default().build()
    }

    /// Returns a fresh builder.
    pub fn builder() -> YfClientBuilder {
        YfClientBuilder::default()
    }

    /// The base data host (`query2.finance.yahoo.com` by default).
    #[allow(dead_code)]
    pub(crate) fn base_host(&self) -> &str {
        &self.inner.base_host
    }

    /// Construct a `reqwest::RequestBuilder` for `path` on the data host with
    /// standard browser headers attached.
    pub(crate) fn data_get(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.inner.base_host, path);
        self.inner.http.get(url).headers(self.std_headers())
    }

    /// Construct a request against an arbitrary URL with browser headers.
    pub(crate) fn raw_get(&self, url: &str) -> reqwest::RequestBuilder {
        self.inner.http.get(url).headers(self.std_headers())
    }

    fn std_headers(&self) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(
            USER_AGENT,
            HeaderValue::from_str(&self.inner.user_agent).unwrap(),
        );
        h.insert(
            ACCEPT,
            HeaderValue::from_static("application/json, text/plain, */*"),
        );
        h.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
        h.insert(
            REFERER,
            HeaderValue::from_static("https://finance.yahoo.com/"),
        );
        h
    }

    /// Send a GET to `path` (relative to the data host) returning a parsed JSON body.
    ///
    /// Adds the current crumb to the query string when one is available, and
    /// applies retry/backoff to transient failures.
    pub(crate) async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T> {
        let body = self.get_bytes(path, query, /*needs_crumb=*/ false).await?;
        serde_json::from_slice(&body).map_err(Error::from)
    }

    /// Variant of [`Self::get_json`] that ensures a crumb is appended to the query.
    pub(crate) async fn get_json_crumb<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T> {
        let body = self.get_bytes(path, query, /*needs_crumb=*/ true).await?;
        serde_json::from_slice(&body).map_err(Error::from)
    }

    async fn get_bytes(
        &self,
        path: &str,
        query: &[(&str, String)],
        needs_crumb: bool,
    ) -> Result<bytes::Bytes> {
        let mut attempt: u32 = 0;
        let mut backoff = self.inner.retry_backoff;

        loop {
            let mut q: Vec<(String, String)> = query
                .iter()
                .map(|(k, v)| ((*k).to_string(), v.clone()))
                .collect();
            if needs_crumb {
                let crumb = self.ensure_crumb().await?;
                q.push(("crumb".to_string(), crumb));
            }

            let req = self.data_get(path).query(&q);
            let res = req.send().await;
            match res {
                Ok(r) => match self.handle_response(r).await {
                    Ok(body) => return Ok(body),
                    Err(e) if attempt < self.inner.max_retries && is_retryable(&e) => {
                        log::debug!("retry {}: {}", attempt + 1, e);
                        sleep(backoff).await;
                        attempt += 1;
                        backoff = backoff.saturating_mul(2);
                    }
                    Err(e) => return Err(e),
                },
                Err(e) if attempt < self.inner.max_retries => {
                    log::debug!("retry {} (transport): {}", attempt + 1, e);
                    sleep(backoff).await;
                    attempt += 1;
                    backoff = backoff.saturating_mul(2);
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    async fn handle_response(&self, res: Response) -> Result<bytes::Bytes> {
        let status = res.status();
        if status.is_success() {
            return Ok(res.bytes().await?);
        }
        if status == StatusCode::TOO_MANY_REQUESTS {
            return Err(Error::RateLimited);
        }
        let body = res.text().await.unwrap_or_default();
        let snippet = body.chars().take(200).collect::<String>();
        Err(Error::Status {
            status: status.as_u16(),
            message: snippet,
        })
    }

    /// Returns a valid crumb, fetching one if missing or stale.
    async fn ensure_crumb(&self) -> Result<String> {
        if let Some(state) = self.inner.crumb.read().clone() {
            if state.fetched_at.elapsed() < CRUMB_TTL {
                return Ok(state.crumb);
            }
        }

        // Serialize: we hold the mutex across the fetch so concurrent callers
        // wait and reuse the result.
        let _guard = self.inner.crumb_state.lock().await;
        // Re-check after acquiring the lock.
        if let Some(state) = self.inner.crumb.read().clone() {
            if state.fetched_at.elapsed() < CRUMB_TTL {
                return Ok(state.crumb);
            }
        }

        let crumb = self.fetch_crumb().await?;
        *self.inner.crumb.write() = Some(CrumbState {
            crumb: crumb.clone(),
            fetched_at: std::time::Instant::now(),
        });
        Ok(crumb)
    }

    async fn fetch_crumb(&self) -> Result<String> {
        // Step 1: prime cookies via fc.yahoo.com.
        let _ = self.raw_get("https://fc.yahoo.com").send().await.ok();

        // Step 2: try query1 first, fall back to query2.
        for url in [
            format!("{}/v1/test/getcrumb", QUERY1_HOST),
            format!("{}/v1/test/getcrumb", QUERY2_HOST),
        ] {
            let res = self.raw_get(&url).send().await;
            let r = match res {
                Ok(r) => r,
                Err(_) => continue,
            };
            if !r.status().is_success() {
                continue;
            }
            let text = r.text().await.unwrap_or_default();
            let crumb = text.trim().trim_matches('"').to_string();
            if !crumb.is_empty() && !crumb.contains('<') {
                return Ok(crumb);
            }
        }

        Err(Error::Auth(
            "could not fetch crumb token from either query1 or query2".into(),
        ))
    }
}

fn is_retryable(err: &Error) -> bool {
    match err {
        Error::Http(e) => {
            e.is_timeout() || e.is_connect() || matches!(e.status(), Some(s) if s.is_server_error())
        }
        Error::Status { status, .. } => *status >= 500,
        Error::RateLimited => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn build_default() {
        let c = YfClient::new().expect("build");
        assert!(c.base_host().contains("yahoo"));
    }

    #[test]
    fn retryable_classification() {
        assert!(is_retryable(&Error::RateLimited));
        assert!(is_retryable(&Error::Status {
            status: 502,
            message: String::new()
        }));
        assert!(!is_retryable(&Error::Status {
            status: 404,
            message: String::new()
        }));
    }
}
