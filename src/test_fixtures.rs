//! Fixture recording helpers — gated behind the `test-mode` feature.
//!
//! When `YF_RECORD=1` is set in the environment and the crate is built with
//! `--features test-mode`, every HTTP response body that the client labels with
//! `(endpoint, symbol)` is written to `tests/fixtures/{endpoint}_{symbol}.{ext}`
//! so it can be replayed by offline tests via `httpmock`. `YF_FIXDIR` overrides
//! the destination directory.

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Sanitize an arbitrary string into a filename-safe label by replacing every
/// non-alphanumeric character with `_`. Used to derive fixture file names from
/// free-form inputs such as search queries.
pub fn safe_label(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// Returns the directory fixtures are read from / written to.
///
/// Defaults to `tests/fixtures/` under the crate root and can be overridden via
/// the `YF_FIXDIR` environment variable.
pub fn fixture_dir() -> PathBuf {
    env::var("YF_FIXDIR").map_or_else(
        |_| Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures"),
        PathBuf::from,
    )
}

/// Path of the fixture file for `(endpoint, symbol, ext)`.
pub fn fixture_path(endpoint: &str, symbol: &str, ext: &str) -> PathBuf {
    fixture_dir().join(format!("{endpoint}_{symbol}.{ext}"))
}

/// True when `YF_RECORD=1` is set.
pub fn is_recording() -> bool {
    env::var("YF_RECORD").ok().as_deref() == Some("1")
}

/// Persist a response body as a fixture. Creates the directory if needed.
pub fn record_fixture(endpoint: &str, symbol: &str, ext: &str, body: &str) -> io::Result<()> {
    let path = fixture_path(endpoint, symbol, ext);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, body)?;
    if env::var("YF_DEBUG").ok().as_deref() == Some("1") {
        eprintln!("YF_RECORD: wrote fixture to {}", path.display());
    }
    Ok(())
}
