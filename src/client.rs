//! HTTP client with cookie + crumb session management.
//!
//! Yahoo's data endpoints require a `crumb` token tied to a session cookie that
//! must be acquired in advance. [`YfClient`] handles the full handshake (basic
//! cookie strategy with CSRF consent fallback), retries transient failures, and
//! caches the crumb for the configured TTL.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

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
/// Default base URL for the ISIN suggest endpoint hosted by Business Insider.
/// It's the same source the Python yfinance library queries.
pub(crate) const DEFAULT_ISIN_BASE: &str =
    "https://markets.businessinsider.com/ajax/SearchController_Suggest";
/// Default base URL for Yahoo Finance's news XHR endpoint family.
pub(crate) const DEFAULT_NEWS_BASE: &str = "https://finance.yahoo.com";
/// Default base URL for the human-facing quote page (used by the profile
/// scrape fallback).
pub(crate) const DEFAULT_QUOTE_PAGE_BASE: &str = "https://finance.yahoo.com/quote";

/// Pulled from yfinance's behavior — crumbs typically remain valid for ~1 hour.
const CRUMB_TTL: Duration = Duration::from_secs(60 * 30);

/// Hint for which Yahoo API surface to prefer when more than one is
/// available (e.g. legacy `v8/finance/chart` vs the newer `v7/finance/quote`
/// for snapshot data). The default is [`ApiPreference::Auto`] which lets the
/// crate pick per-endpoint.
///
/// Today this is plumbed through the client builder for parity with the
/// gramistella upstream; individual endpoints don't yet branch on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ApiPreference {
    /// Pick the best surface per endpoint (default).
    #[default]
    Auto,
    /// Prefer the legacy `v8/finance/chart` family.
    Chart,
    /// Prefer the newer `v7/finance/quote` family.
    Quote,
}

/// Whether a request should consult / write the in-memory response cache.
///
/// The cache is only active when [`YfClientBuilder::cache_ttl`] is non-zero;
/// otherwise the variants are a no-op and the request always hits the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CacheMode {
    /// Read from the cache if a fresh entry exists, otherwise fetch and store.
    #[default]
    Use,
    /// Skip cache reads but still write the fetched response.
    Refresh,
    /// Skip both reads and writes for this request.
    Bypass,
}

/// Per-request override for the retry policy. `None` means inherit the
/// client-wide configuration set via the builder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryConfig {
    /// Maximum retries on transient failures (5xx, 429, transport errors).
    pub max_retries: u32,
    /// Initial backoff between retries; doubled each attempt.
    pub initial_backoff: Duration,
}

impl RetryConfig {
    /// Build a new policy.
    pub fn new(max_retries: u32, initial_backoff: Duration) -> Self {
        Self {
            max_retries,
            initial_backoff,
        }
    }
}

/// Builder for [`YfClient`].
#[derive(Debug)]
pub struct YfClientBuilder {
    user_agent: Option<String>,
    timeout: Duration,
    base_query_host: String,
    crumb_url: Option<String>,
    cookie_prime_url: Option<String>,
    isin_base_url: Option<String>,
    news_base_url: Option<String>,
    quote_page_base_url: Option<String>,
    session_cookie: Option<String>,
    session_crumb: Option<String>,
    max_retries: u32,
    retry_backoff: Duration,
    cache_ttl: Duration,
    api_preference: ApiPreference,
    underlying: Option<Client>,
}

impl Default for YfClientBuilder {
    fn default() -> Self {
        Self {
            user_agent: None,
            timeout: Duration::from_secs(30),
            // Match Python yfinance and the gramistella upstream: query1 is the
            // primary host, query2 the fallback. Some endpoints (notably the
            // crumb fetch) are stricter on query2.
            base_query_host: QUERY1_HOST.to_string(),
            crumb_url: None,
            cookie_prime_url: None,
            isin_base_url: None,
            news_base_url: None,
            quote_page_base_url: None,
            session_cookie: None,
            session_crumb: None,
            max_retries: 3,
            retry_backoff: Duration::from_millis(500),
            cache_ttl: Duration::ZERO,
            api_preference: ApiPreference::default(),
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

    /// Apply a [`RetryConfig`] (sugar for `max_retries` + `retry_backoff`).
    pub fn retry(mut self, cfg: RetryConfig) -> Self {
        self.max_retries = cfg.max_retries;
        self.retry_backoff = cfg.initial_backoff;
        self
    }

    /// Set the in-memory response cache TTL. `Duration::ZERO` (default)
    /// disables caching entirely. Cache keys are URL+query so the same
    /// request from different builders is shared.
    pub fn cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = ttl;
        self
    }

    /// Set the [`ApiPreference`] hint for endpoints that have more than one
    /// implementation. Currently advisory.
    pub fn api_preference(mut self, pref: ApiPreference) -> Self {
        self.api_preference = pref;
        self
    }

    /// Override the base host used for data requests. Useful for tests against
    /// a mock server.
    pub fn base_host(mut self, host: impl Into<String>) -> Self {
        self.base_query_host = host.into();
        self
    }

    /// Override the crumb-fetch URL (full URL, e.g.
    /// `http://127.0.0.1:1234/v1/test/getcrumb`). When set, the default
    /// query1 → query2 fallback is bypassed. Used by tests that mock Yahoo.
    pub fn crumb_url(mut self, url: impl Into<String>) -> Self {
        self.crumb_url = Some(url.into());
        self
    }

    /// Override the cookie-priming URL (default: `https://fc.yahoo.com/consent`).
    /// Pass an empty string to skip priming entirely — useful in offline tests.
    pub fn cookie_prime_url(mut self, url: impl Into<String>) -> Self {
        self.cookie_prime_url = Some(url.into());
        self
    }

    /// Inject a raw `Cookie:` header value (semicolon-separated `key=value`
    /// pairs). When set, it's sent on every request to Yahoo hosts in
    /// addition to the `reqwest` cookie store. Combine with
    /// `cookie_prime_url("")` to skip the consent fetch when you already
    /// have a browser session.
    ///
    /// ```no_run
    /// # use yfinance::YfClient;
    /// // Cookies copied from your browser's DevTools Network tab.
    /// let client = YfClient::builder()
    ///     .cookie_prime_url("")
    ///     .session_cookie("A1=d=...; A3=d=...; gpp=DBAA")
    ///     .build()?;
    /// # Ok::<(), yfinance::Error>(())
    /// ```
    pub fn session_cookie(mut self, cookie_header: impl Into<String>) -> Self {
        self.session_cookie = Some(cookie_header.into());
        self
    }

    /// Inject a pre-fetched crumb token. When set, the crumb-fetch handshake
    /// (`fc.yahoo.com/consent` + `query1/v1/test/getcrumb`) is skipped on
    /// authenticated requests — the supplied crumb is used directly.
    ///
    /// Pair with [`Self::session_cookie`] to bring an entire browser session
    /// — paste your `crumb` token from a `query1/v1/test/getcrumb` request
    /// in DevTools.
    pub fn session_crumb(mut self, crumb: impl Into<String>) -> Self {
        self.session_crumb = Some(crumb.into());
        self
    }

    /// Override the base URL of the ISIN suggest service (default:
    /// `https://markets.businessinsider.com/ajax/SearchController_Suggest`).
    pub fn isin_base_url(mut self, url: impl Into<String>) -> Self {
        self.isin_base_url = Some(url.into());
        self
    }

    /// Override the base URL of Yahoo's news XHR endpoint family
    /// (default: `https://finance.yahoo.com`).
    pub fn news_base_url(mut self, url: impl Into<String>) -> Self {
        self.news_base_url = Some(url.into());
        self
    }

    /// Override the base URL of the human-facing quote page used by the
    /// profile scrape fallback (default: `https://finance.yahoo.com/quote`).
    pub fn quote_page_base_url(mut self, url: impl Into<String>) -> Self {
        self.quote_page_base_url = Some(url.into());
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
                crumb_url: self.crumb_url,
                cookie_prime_url: self.cookie_prime_url,
                isin_base_url: self
                    .isin_base_url
                    .unwrap_or_else(|| DEFAULT_ISIN_BASE.to_string()),
                news_base_url: self
                    .news_base_url
                    .unwrap_or_else(|| DEFAULT_NEWS_BASE.to_string()),
                quote_page_base_url: self
                    .quote_page_base_url
                    .unwrap_or_else(|| DEFAULT_QUOTE_PAGE_BASE.to_string()),
                session_cookie: self.session_cookie,
                session_crumb: self.session_crumb,
                max_retries: self.max_retries,
                retry_backoff: self.retry_backoff,
                cache_ttl: self.cache_ttl,
                api_preference: self.api_preference,
                crumb_state: Mutex::new(()),
                crumb: RwLock::new(None),
                cache: RwLock::new(HashMap::new()),
            }),
        })
    }
}

/// Asynchronous HTTP client that authenticates against Yahoo Finance and
/// performs JSON requests with retry/backoff.
///
/// Cheap to clone — internally an `Arc`.
#[derive(Clone)]
pub struct YfClient {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for YfClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("YfClient")
            .field("base_host", &self.inner.base_host)
            .field("max_retries", &self.inner.max_retries)
            .field("cache_ttl", &self.inner.cache_ttl)
            .finish()
    }
}

struct Inner {
    http: Client,
    user_agent: String,
    base_host: String,
    crumb_url: Option<String>,
    cookie_prime_url: Option<String>,
    isin_base_url: String,
    news_base_url: String,
    quote_page_base_url: String,
    session_cookie: Option<String>,
    session_crumb: Option<String>,
    max_retries: u32,
    retry_backoff: Duration,
    cache_ttl: Duration,
    api_preference: ApiPreference,
    /// Serializes crumb refreshes so concurrent callers share one handshake.
    crumb_state: Mutex<()>,
    crumb: RwLock<Option<CrumbState>>,
    cache: RwLock<HashMap<String, CacheEntry>>,
}

#[derive(Clone)]
struct CacheEntry {
    body: bytes::Bytes,
    inserted: Instant,
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

    /// The base data host (`query1.finance.yahoo.com` by default).
    #[allow(dead_code)]
    pub(crate) fn base_host(&self) -> &str {
        &self.inner.base_host
    }

    /// The configured ISIN suggest endpoint base URL.
    pub(crate) fn isin_base_url(&self) -> &str {
        &self.inner.isin_base_url
    }

    /// The configured base URL for the quote-page scrape (no trailing `/`).
    pub(crate) fn quote_page_base_url(&self) -> &str {
        &self.inner.quote_page_base_url
    }

    /// The configured API preference hint.
    pub fn api_preference(&self) -> ApiPreference {
        self.inner.api_preference
    }

    /// Construct a POST `RequestBuilder` to `path` on the news host with the
    /// standard browser headers attached.
    pub(crate) fn news_post(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.inner.news_base_url, path);
        self.inner.http.post(url).headers(self.std_headers())
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

    /// Send a `RequestBuilder` to completion, optionally recording the body as
    /// a fixture under `(endpoint, symbol)`. Returns `Ok(None)` for non-success
    /// statuses where callers want to degrade gracefully (e.g. ISIN lookup),
    /// otherwise the response body. Used by modules that don't go through the
    /// JSON helpers — `isin` (text response from a 3rd-party host) and `news`
    /// (POST with JSON body).
    pub(crate) async fn send_text_recorded(
        &self,
        req: reqwest::RequestBuilder,
        record_as: Option<(&str, &str)>,
    ) -> Result<Option<String>> {
        let resp = req.send().await?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if status.is_success() {
            Self::maybe_record(record_as, text.as_bytes());
            return Ok(Some(text));
        }
        if status == StatusCode::TOO_MANY_REQUESTS {
            return Err(Error::RateLimited);
        }
        if status.is_server_error() || status.as_u16() == 404 {
            return Err(Error::Status {
                status: status.as_u16(),
                message: text.chars().take(200).collect(),
            });
        }
        Ok(None)
    }

    /// Percent-encode characters that aren't safe in a Yahoo URL path segment.
    /// Yahoo accepts unencoded `. - = ^ _` in symbols (`BRK-B`, `^GSPC`, …).
    pub(crate) fn path_encode(s: &str) -> String {
        s.chars()
            .map(|c| match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '.' | '-' | '=' | '^' | '_' => c.to_string(),
                _ => format!("%{:02X}", c as u32),
            })
            .collect()
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
        if let Some(cookie) = self.inner.session_cookie.as_deref() {
            if let Ok(v) = HeaderValue::from_str(cookie) {
                h.insert(reqwest::header::COOKIE, v);
            }
        }
        h
    }

    /// Send a GET to `path` (relative to the data host) returning a parsed JSON body.
    ///
    /// Adds the current crumb to the query string when one is available, and
    /// applies retry/backoff to transient failures.
    ///
    /// `record_as` labels the response for fixture recording: when the crate is
    /// built with `test-mode` and `YF_RECORD=1` is set, the raw body is written
    /// to `tests/fixtures/{endpoint}_{symbol}.json`. Pass `None` to skip.
    pub(crate) async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
        record_as: Option<(&str, &str)>,
    ) -> Result<T> {
        self.get_json_cached(path, query, record_as, CacheMode::Use)
            .await
    }

    /// Like [`Self::get_json`] but consults the response cache per the given
    /// [`CacheMode`]. Used by builders that expose per-request overrides.
    pub(crate) async fn get_json_cached<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
        record_as: Option<(&str, &str)>,
        cache_mode: CacheMode,
    ) -> Result<T> {
        let body = self
            .get_bytes(path, query, /*needs_crumb=*/ false, cache_mode)
            .await?;
        Self::maybe_record(record_as, &body);
        serde_json::from_slice(&body).map_err(Error::from)
    }

    /// Variant of [`Self::get_json`] that ensures a crumb is appended to the query.
    pub(crate) async fn get_json_crumb<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
        record_as: Option<(&str, &str)>,
    ) -> Result<T> {
        let body = self
            .get_bytes(path, query, /*needs_crumb=*/ true, CacheMode::Use)
            .await?;
        Self::maybe_record(record_as, &body);
        serde_json::from_slice(&body).map_err(Error::from)
    }

    /// Fetch one or more `quoteSummary` modules for `symbol`. Returns the first
    /// `result[0]` Map (or `None` when Yahoo returned an empty result), with
    /// `quoteSummary.error` mapped to [`Error::Yahoo`].
    pub(crate) async fn fetch_quote_summary(
        &self,
        symbol: &str,
        modules: &str,
        fixture_label: &str,
    ) -> Result<Option<serde_json::Map<String, serde_json::Value>>> {
        #[derive(serde::Deserialize)]
        struct Envelope {
            #[serde(rename = "quoteSummary")]
            quote_summary: Inner,
        }
        #[derive(serde::Deserialize)]
        struct Inner {
            #[serde(default)]
            result: Vec<serde_json::Map<String, serde_json::Value>>,
            #[serde(default)]
            error: Option<serde_json::Value>,
        }

        let path = format!("/v10/finance/quoteSummary/{}", Self::path_encode(symbol));
        let q = vec![
            ("modules", modules.to_string()),
            ("formatted", "false".into()),
            ("corsDomain", "finance.yahoo.com".into()),
        ];
        let env: Envelope = self
            .get_json_crumb(&path, &q, Some((fixture_label, symbol)))
            .await?;
        if let Some(err) = env.quote_summary.error {
            return Err(Error::Yahoo {
                symbol: symbol.to_string(),
                code: format!("{fixture_label}_error"),
                description: err.to_string(),
            });
        }
        Ok(env.quote_summary.result.into_iter().next())
    }

    fn maybe_record(label: Option<(&str, &str)>, body: &[u8]) {
        #[cfg(feature = "test-mode")]
        {
            if let Some((endpoint, symbol)) = label {
                if crate::test_fixtures::is_recording() {
                    if let Ok(text) = std::str::from_utf8(body) {
                        if let Err(e) =
                            crate::test_fixtures::record_fixture(endpoint, symbol, "json", text)
                        {
                            eprintln!("YF_RECORD: failed to write fixture for {symbol}: {e}");
                        }
                    }
                }
            }
        }
        #[cfg(not(feature = "test-mode"))]
        {
            let _ = (label, body);
        }
    }

    async fn get_bytes(
        &self,
        path: &str,
        query: &[(&str, String)],
        needs_crumb: bool,
        cache_mode: CacheMode,
    ) -> Result<bytes::Bytes> {
        let mut attempt: u32 = 0;
        let mut backoff = self.inner.retry_backoff;

        let mut q: Vec<(&str, String)> = query.iter().map(|(k, v)| (*k, v.clone())).collect();
        if needs_crumb {
            let crumb = self.ensure_crumb().await?;
            q.push(("crumb", crumb));
        }

        let cache_active = self.inner.cache_ttl > Duration::ZERO && cache_mode != CacheMode::Bypass;
        let cache_key =
            cache_active.then(|| format!("{}{}?{}", self.inner.base_host, path, encode_query(&q)));
        if let Some(key) = &cache_key {
            if cache_mode == CacheMode::Use {
                if let Some(body) = self.cache_lookup(key) {
                    return Ok(body);
                }
            }
        }

        loop {
            let req = self.data_get(path).query(&q);
            let res = req.send().await;
            match res {
                Ok(r) => match self.handle_response(r).await {
                    Ok(body) => {
                        if let Some(key) = &cache_key {
                            self.cache_store(key.clone(), body.clone());
                        }
                        return Ok(body);
                    }
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

    fn cache_lookup(&self, key: &str) -> Option<bytes::Bytes> {
        let entry = self.inner.cache.read().get(key).cloned()?;
        if entry.inserted.elapsed() < self.inner.cache_ttl {
            Some(entry.body)
        } else {
            None
        }
    }

    fn cache_store(&self, key: String, body: bytes::Bytes) {
        self.inner.cache.write().insert(
            key,
            CacheEntry {
                body,
                inserted: Instant::now(),
            },
        );
    }

    /// Returns a valid crumb, fetching one if missing or stale.
    async fn ensure_crumb(&self) -> Result<String> {
        if let Some(injected) = self.inner.session_crumb.clone() {
            return Ok(injected);
        }
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
        // Step 1: prime cookies. Default points at fc.yahoo.com; tests
        // override (or set to empty to skip).
        // The `/consent` path is what other Yahoo clients (Python yfinance,
        // gramistella/yfinance-rs) hit — root `/` is more aggressively
        // throttled and sometimes returns 404 anyway.
        let prime = self
            .inner
            .cookie_prime_url
            .as_deref()
            .unwrap_or("https://fc.yahoo.com/consent");
        if !prime.is_empty() {
            let _ = self.raw_get(prime).send().await.ok();
        }

        // Step 2: explicit override wins, otherwise try query1 then query2.
        let urls: Vec<String> = if let Some(u) = &self.inner.crumb_url {
            vec![u.clone()]
        } else {
            vec![
                format!("{}/v1/test/getcrumb", QUERY1_HOST),
                format!("{}/v1/test/getcrumb", QUERY2_HOST),
            ]
        };
        for url in &urls {
            let res = self.raw_get(url).send().await;
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

fn encode_query(q: &[(&str, String)]) -> String {
    use std::fmt::Write;
    let mut buf = String::new();
    for (i, (k, v)) in q.iter().enumerate() {
        if i > 0 {
            buf.push('&');
        }
        let _ = write!(&mut buf, "{k}={v}");
    }
    buf
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
