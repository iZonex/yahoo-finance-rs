//! Error and result types.

use std::fmt;

/// Convenient `Result` alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// All errors produced by this crate.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Underlying HTTP transport failure.
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON decoding failure.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// URL parsing failure.
    #[error("url error: {0}")]
    Url(#[from] url::ParseError),

    /// Yahoo returned a non-success status code.
    #[error("yahoo returned status {status}: {message}")]
    Status {
        /// HTTP status code.
        status: u16,
        /// Best-effort message extracted from the response body.
        message: String,
    },

    /// Yahoo's chart/v8 endpoint returned an explicit error payload.
    #[error("yahoo error [{code}] for {symbol}: {description}")]
    Yahoo {
        /// Symbol that triggered the error.
        symbol: String,
        /// Yahoo error code (e.g. `Not Found`).
        code: String,
        /// Human-readable description.
        description: String,
    },

    /// We got a 429 from Yahoo and exhausted retry budget.
    #[error("rate limited by Yahoo (HTTP 429)")]
    RateLimited,

    /// Could not obtain an authenticated session (cookie + crumb).
    #[error("could not authenticate with Yahoo: {0}")]
    Auth(String),

    /// The ticker symbol is missing or delisted.
    #[error("ticker `{ticker}` not found: {reason}")]
    TickerMissing {
        /// The requested symbol.
        ticker: String,
        /// Reason supplied by the upstream API or our parser.
        reason: String,
    },

    /// The ticker has no timezone info — usually means delisted.
    #[error("ticker `{0}` has no timezone metadata")]
    TimezoneMissing(String),

    /// No price data returned for the requested period/interval.
    #[error("no prices returned for `{ticker}` ({hint})")]
    PricesMissing {
        /// The requested symbol.
        ticker: String,
        /// Human-readable hint about why data was missing.
        hint: String,
    },

    /// The requested period/interval combination is not allowed.
    #[error("invalid period/interval for `{ticker}`: {reason}")]
    InvalidPeriod {
        /// The requested symbol.
        ticker: String,
        /// Why the combination is invalid.
        reason: String,
    },

    /// A response field had an unexpected shape.
    #[error("unexpected response shape: {0}")]
    UnexpectedResponse(String),

    /// A required argument was invalid.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    /// Catch-all for unexpected I/O.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Error {
    /// Construct an [`Error::UnexpectedResponse`] with a formatted message.
    pub fn unexpected(msg: impl fmt::Display) -> Self {
        Error::UnexpectedResponse(msg.to_string())
    }

    /// Construct an [`Error::InvalidArgument`] with a formatted message.
    pub fn invalid(msg: impl fmt::Display) -> Self {
        Error::InvalidArgument(msg.to_string())
    }
}
