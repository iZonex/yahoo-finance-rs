//! Real-time quote streaming (`stream` feature).
//!
//! Two implementations behind a single [`StreamBuilder`]:
//! - WebSocket via `streamer.finance.yahoo.com` — lowest latency, primary path.
//! - HTTP polling on `/v7/finance/quote` — fallback for environments where
//!   WebSockets are blocked.
//!
//! Yahoo's WebSocket sends base64-encoded protobuf frames; the schema is the
//! `Yaticker.PricingData` message, transcribed by hand below to avoid a
//! `protoc`/`prost-build` build dependency.
//!
//! ## Volume semantics
//! Yahoo reports cumulative intraday volume (`day_volume`). This module
//! converts that into per-update **deltas**:
//! - First tick per symbol → `volume = None`.
//! - Subsequent ticks → `volume = Some(current - previous)`.
//! - Detected resets (current < previous, e.g. midnight rollover) → `volume =
//!   Some(current)` so the new session starts fresh.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use base64::{engine::general_purpose, Engine as _};
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use prost::Message as ProstMessage;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::protocol::Message as WsMessage;

use crate::client::YfClient;
use crate::error::Result;

const DEFAULT_WS_URL: &str = "wss://streamer.finance.yahoo.com/?version=2";

/// Quote update emitted by the stream.
#[derive(Debug, Clone, PartialEq)]
pub struct QuoteUpdate {
    /// Symbol (e.g. `"AAPL"`).
    pub symbol: String,
    /// Last trade price.
    pub price: f64,
    /// Wall-clock timestamp of the tick (UNIX milliseconds).
    pub timestamp_ms: i64,
    /// Reporting currency (e.g. `"USD"`).
    pub currency: Option<String>,
    /// Exchange code.
    pub exchange: Option<String>,
    /// Day-over-day percentage change.
    pub change_percent: Option<f64>,
    /// Day-over-day absolute change.
    pub change: Option<f64>,
    /// Per-update volume **delta** (see module docs for semantics).
    pub volume: Option<i64>,
}

/// Transport for the stream.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StreamMethod {
    /// WebSocket (default).
    #[default]
    WebSocket,
    /// HTTP polling against `/v7/finance/quote`.
    HttpPolling,
}

/// Polling-mode configuration. Ignored for WebSocket transport.
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// How often to poll. Default: 1s.
    pub interval: Duration,
    /// Only emit when price *or* cumulative volume changes. Default: true.
    pub diff_only: bool,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(1),
            diff_only: true,
        }
    }
}

/// Handle to a running stream. Drop or call [`StreamHandle::stop`] to tear down.
#[derive(Debug)]
pub struct StreamHandle {
    join: Option<JoinHandle<()>>,
    stop_tx: Option<oneshot::Sender<()>>,
}

impl StreamHandle {
    /// Signal the background task to exit and wait for it.
    pub async fn stop(mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(j) = self.join.take() {
            let _ = j.await;
        }
    }
}

impl Drop for StreamHandle {
    fn drop(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(j) = self.join.take() {
            j.abort();
        }
    }
}

/// Builder for a streaming session.
#[derive(Debug, Clone)]
pub struct StreamBuilder {
    client: YfClient,
    symbols: Vec<String>,
    method: StreamMethod,
    config: StreamConfig,
    ws_url: String,
}

impl StreamBuilder {
    pub(crate) fn new(client: &YfClient, symbol: impl Into<String>) -> Self {
        Self {
            client: client.clone(),
            symbols: vec![symbol.into()],
            method: StreamMethod::default(),
            config: StreamConfig::default(),
            ws_url: DEFAULT_WS_URL.to_string(),
        }
    }

    /// Add another symbol to the subscription.
    pub fn add_symbol(mut self, symbol: impl Into<String>) -> Self {
        self.symbols.push(symbol.into());
        self
    }

    /// Replace the symbol list outright.
    pub fn symbols<I, S>(mut self, symbols: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.symbols = symbols.into_iter().map(Into::into).collect();
        self
    }

    /// Pick the transport. Default: WebSocket.
    pub fn method(mut self, method: StreamMethod) -> Self {
        self.method = method;
        self
    }

    /// Configure polling-mode behavior.
    pub fn config(mut self, config: StreamConfig) -> Self {
        self.config = config;
        self
    }

    /// Override the WebSocket URL (default: `wss://streamer.finance.yahoo.com/?version=2`).
    /// Mostly useful for tests with a local WS server.
    pub fn ws_url(mut self, url: impl Into<String>) -> Self {
        self.ws_url = url.into();
        self
    }

    /// Spawn the stream and return a sender of [`QuoteUpdate`]s.
    ///
    /// The receiver yields `None` when the stream stops (either via
    /// [`StreamHandle::stop`] or upstream connection failure that exhausted
    /// internal recovery).
    pub fn start(self) -> Result<(mpsc::Receiver<QuoteUpdate>, StreamHandle)> {
        let (tx, rx) = mpsc::channel::<QuoteUpdate>(64);
        let (stop_tx, stop_rx) = oneshot::channel::<()>();
        let join = match self.method {
            StreamMethod::WebSocket => spawn_ws(self, tx, stop_rx),
            StreamMethod::HttpPolling => spawn_polling(self, tx, stop_rx),
        };
        Ok((
            rx,
            StreamHandle {
                join: Some(join),
                stop_tx: Some(stop_tx),
            },
        ))
    }
}

#[derive(Default)]
struct VolumeTracker {
    last: HashMap<String, i64>,
}

impl VolumeTracker {
    fn delta(&mut self, symbol: &str, current: Option<i64>) -> Option<i64> {
        let current = current?;
        let entry = self.last.get(symbol).copied();
        self.last.insert(symbol.to_string(), current);
        match entry {
            None => None,
            Some(prev) if current < prev => Some(current),
            Some(prev) => Some(current - prev),
        }
    }
}

/// Hand-transcribed equivalent of Yahoo's `Yaticker.PricingData` protobuf.
/// Field tags are stable; only update if the live wire format changes.
#[derive(Clone, PartialEq, ::prost::Message)]
struct PricingData {
    #[prost(string, tag = "1")]
    id: ::prost::alloc::string::String,
    #[prost(float, tag = "2")]
    price: f32,
    #[prost(sint64, tag = "3")]
    time: i64,
    #[prost(string, tag = "4")]
    currency: ::prost::alloc::string::String,
    #[prost(string, tag = "5")]
    exchange: ::prost::alloc::string::String,
    #[prost(int32, tag = "6")]
    quote_type: i32,
    #[prost(int32, tag = "7")]
    market_hours: i32,
    #[prost(float, tag = "8")]
    change_percent: f32,
    #[prost(sint64, tag = "9")]
    day_volume: i64,
    #[prost(float, tag = "10")]
    day_high: f32,
    #[prost(float, tag = "11")]
    day_low: f32,
    #[prost(float, tag = "12")]
    change: f32,
    #[prost(string, tag = "13")]
    short_name: ::prost::alloc::string::String,
    #[prost(sint64, tag = "14")]
    expire_date: i64,
    #[prost(float, tag = "15")]
    open_price: f32,
    #[prost(float, tag = "16")]
    previous_close: f32,
    #[prost(float, tag = "17")]
    strike_price: f32,
    #[prost(string, tag = "18")]
    underlying_symbol: ::prost::alloc::string::String,
    #[prost(sint64, tag = "19")]
    open_interest: i64,
    #[prost(sint64, tag = "20")]
    options_type: i64,
    #[prost(sint64, tag = "21")]
    mini_option: i64,
    #[prost(sint64, tag = "22")]
    last_size: i64,
    #[prost(float, tag = "23")]
    bid: f32,
    #[prost(sint64, tag = "24")]
    bid_size: i64,
    #[prost(float, tag = "25")]
    ask: f32,
    #[prost(sint64, tag = "26")]
    ask_size: i64,
    #[prost(sint64, tag = "27")]
    price_hint: i64,
    #[prost(sint64, tag = "28")]
    vol_24hr: i64,
    #[prost(sint64, tag = "29")]
    vol_all_currencies: i64,
    #[prost(string, tag = "30")]
    from_currency: ::prost::alloc::string::String,
    #[prost(string, tag = "31")]
    last_market: ::prost::alloc::string::String,
    #[prost(double, tag = "32")]
    circulating_supply: f64,
    #[prost(double, tag = "33")]
    market_cap: f64,
}

/// Decode one base64-encoded protobuf frame into our public [`QuoteUpdate`].
/// Returns `None` if the payload doesn't decode.
pub(crate) fn decode_frame(b64: &str) -> Option<(String, QuoteUpdate)> {
    let bytes = general_purpose::STANDARD.decode(b64.trim()).ok()?;
    let raw = PricingData::decode(bytes.as_slice()).ok()?;
    let symbol = raw.id.clone();
    let day_volume = if raw.day_volume == 0 {
        None
    } else {
        Some(raw.day_volume)
    };
    Some((
        symbol.clone(),
        QuoteUpdate {
            symbol,
            price: f64::from(raw.price),
            timestamp_ms: raw.time,
            currency: opt_str(raw.currency),
            exchange: opt_str(raw.exchange),
            change_percent: finite(f64::from(raw.change_percent)),
            change: finite(f64::from(raw.change)),
            // Volume delta is calculated by the dispatcher, not here.
            volume: day_volume,
        },
    ))
}

fn opt_str(s: String) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Derive `(change, change_percent)` from a current price and a previous
/// close, when known. Used by both transports so they emit the same shape.
fn derive_change(price: f64, previous_close: Option<f64>) -> (Option<f64>, Option<f64>) {
    let Some(prev) = previous_close else {
        return (None, None);
    };
    let abs = price - prev;
    let pct = (prev.abs() > f64::EPSILON).then(|| abs / prev * 100.0);
    (Some(abs), pct)
}

fn finite(v: f64) -> Option<f64> {
    if v.is_finite() && v != 0.0 {
        Some(v)
    } else {
        None
    }
}

fn spawn_ws(
    builder: StreamBuilder,
    tx: mpsc::Sender<QuoteUpdate>,
    mut stop_rx: oneshot::Receiver<()>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let StreamBuilder {
            symbols, ws_url, ..
        } = builder;
        let tracker = Arc::new(Mutex::new(VolumeTracker::default()));

        let (ws, _) = match tokio_tungstenite::connect_async(&ws_url).await {
            Ok(v) => v,
            Err(e) => {
                log::warn!("stream: ws connect failed: {e}");
                return;
            }
        };
        let (mut sink, mut source) = ws.split();

        let subscribe = serde_json::json!({ "subscribe": symbols });
        if let Err(e) = sink.send(WsMessage::Text(subscribe.to_string())).await {
            log::warn!("stream: ws subscribe failed: {e}");
            return;
        }

        loop {
            tokio::select! {
                _ = &mut stop_rx => {
                    let _ = sink.send(WsMessage::Close(None)).await;
                    break;
                }
                msg = source.next() => {
                    match msg {
                        Some(Ok(WsMessage::Text(t))) => {
                            if let Some((sym, mut up)) = decode_frame(&t) {
                                up.volume = tracker.lock().delta(&sym, up.volume);
                                if tx.send(up).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Some(Ok(WsMessage::Binary(b))) => {
                            // Some servers send raw base64 string as binary
                            if let Ok(s) = std::str::from_utf8(&b) {
                                if let Some((sym, mut up)) = decode_frame(s) {
                                    up.volume = tracker.lock().delta(&sym, up.volume);
                                    if tx.send(up).await.is_err() {
                                        break;
                                    }
                                }
                            }
                        }
                        Some(Ok(WsMessage::Ping(p))) => {
                            let _ = sink.send(WsMessage::Pong(p)).await;
                        }
                        Some(Ok(WsMessage::Close(_))) | None => break,
                        Some(Ok(_)) => {}
                        Some(Err(e)) => {
                            log::warn!("stream: ws read error: {e}");
                            break;
                        }
                    }
                }
            }
        }
    })
}

fn spawn_polling(
    builder: StreamBuilder,
    tx: mpsc::Sender<QuoteUpdate>,
    mut stop_rx: oneshot::Receiver<()>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let StreamBuilder {
            client,
            symbols,
            config,
            ..
        } = builder;
        let mut last_price: HashMap<String, f64> = HashMap::new();
        let tracker = Arc::new(Mutex::new(VolumeTracker::default()));
        loop {
            // Fan out a fetch per symbol so total tick latency is `RTT`, not
            // `N * RTT`.
            let fetches = symbols
                .iter()
                .map(|sym| {
                    let client = client.clone();
                    let sym = sym.clone();
                    async move {
                        let info = crate::quote::FastInfo::fetch(&client, &sym).await.ok();
                        (sym, info)
                    }
                })
                .collect::<Vec<_>>();
            let results = futures::future::join_all(fetches).await;
            for (sym, info) in results {
                let Some(info) = info else { continue };
                let Some(price) = info.last_price else {
                    continue;
                };
                let cum_volume = info.last_volume.and_then(|v| i64::try_from(v).ok());
                let volume = tracker.lock().delta(&sym, cum_volume);
                let price_changed = match last_price.get(&sym) {
                    None => true,
                    Some(p) => (p - price).abs() > f64::EPSILON,
                };
                let volume_changed = matches!(volume, Some(v) if v != 0);
                if !config.diff_only || price_changed || volume_changed {
                    last_price.insert(sym.clone(), price);
                    let (change, change_percent) = derive_change(price, info.previous_close);
                    let up = QuoteUpdate {
                        symbol: sym,
                        price,
                        timestamp_ms: chrono::Utc::now().timestamp_millis(),
                        currency: info.currency,
                        exchange: info.exchange,
                        change_percent,
                        change,
                        volume,
                    };
                    if tx.send(up).await.is_err() {
                        return;
                    }
                }
            }
            tokio::select! {
                _ = &mut stop_rx => return,
                () = tokio::time::sleep(config.interval) => {}
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_tracker_first_tick_is_none() {
        let mut t = VolumeTracker::default();
        assert_eq!(t.delta("AAPL", Some(1000)), None);
    }

    #[test]
    fn volume_tracker_emits_delta() {
        let mut t = VolumeTracker::default();
        t.delta("AAPL", Some(1000));
        assert_eq!(t.delta("AAPL", Some(1500)), Some(500));
        assert_eq!(t.delta("AAPL", Some(1500)), Some(0));
    }

    #[test]
    fn volume_tracker_handles_reset() {
        let mut t = VolumeTracker::default();
        t.delta("AAPL", Some(10_000));
        // After-hours rollover — counter resets to a small value.
        assert_eq!(t.delta("AAPL", Some(200)), Some(200));
    }

    #[test]
    fn decode_frame_round_trip() {
        let raw = PricingData {
            id: "AAPL".into(),
            price: 150.5,
            time: 1_700_000_000_000,
            currency: "USD".into(),
            exchange: "NMS".into(),
            quote_type: 8,
            market_hours: 1,
            change_percent: 1.25,
            day_volume: 12_345_678,
            day_high: 151.0,
            day_low: 149.0,
            change: 1.85,
            short_name: "Apple".into(),
            expire_date: 0,
            open_price: 149.5,
            previous_close: 148.65,
            strike_price: 0.0,
            underlying_symbol: String::new(),
            open_interest: 0,
            options_type: 0,
            mini_option: 0,
            last_size: 0,
            bid: 0.0,
            bid_size: 0,
            ask: 0.0,
            ask_size: 0,
            price_hint: 0,
            vol_24hr: 0,
            vol_all_currencies: 0,
            from_currency: String::new(),
            last_market: String::new(),
            circulating_supply: 0.0,
            market_cap: 0.0,
        };
        let mut buf = Vec::with_capacity(raw.encoded_len());
        raw.encode(&mut buf).unwrap();
        let b64 = general_purpose::STANDARD.encode(&buf);
        let (sym, up) = decode_frame(&b64).expect("decoded");
        assert_eq!(sym, "AAPL");
        assert_eq!(up.symbol, "AAPL");
        assert!((up.price - 150.5).abs() < 1e-3);
        assert_eq!(up.timestamp_ms, 1_700_000_000_000);
        assert_eq!(up.currency.as_deref(), Some("USD"));
        assert_eq!(up.volume, Some(12_345_678));
    }
}
