//! Internal helpers for parsing Yahoo's wire format.
//!
//! Yahoo wraps numeric fields as `{ "raw": <number>, "fmt": "<string>" }`.
//! Once `formatted=false` is requested they sometimes still arrive in the
//! wrapped shape. [`RawNum`] decodes either form, and the small `from_raw_*`
//! helpers project the common variants into plain numerics.

use serde::{de::DeserializeOwned, Deserialize};

/// `{ "raw": T, ... }` envelope. Yahoo emits this shape only with
/// `formatted=true`; with `formatted=false` it sends bare numbers. Both are
/// accepted transparently.
#[derive(Debug, Clone, Copy)]
pub struct RawNum<T> {
    pub raw: Option<T>,
}

impl<'de, T> Deserialize<'de> for RawNum<T>
where
    T: DeserializeOwned + Copy,
{
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // Accept either a bare T (or null), or `{raw: T, ...}`.
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Either<T> {
            Bare(T),
            Wrapped {
                #[serde(default = "Option::default")]
                raw: Option<T>,
            },
        }
        let v: Option<Either<T>> = Option::deserialize(d)?;
        Ok(match v {
            None => RawNum { raw: None },
            Some(Either::Bare(x)) => RawNum { raw: Some(x) },
            Some(Either::Wrapped { raw }) => RawNum { raw },
        })
    }
}

/// Project `Option<RawNum<T>>` to `Option<f64>` for any numeric `T`.
pub fn from_raw_f64<T: Into<f64> + Copy>(n: Option<RawNum<T>>) -> Option<f64> {
    n.and_then(|x| x.raw.map(Into::into))
}

/// Project `Option<RawNum<i64>>` to `Option<i64>`.
pub fn from_raw_i64(n: Option<RawNum<i64>>) -> Option<i64> {
    n.and_then(|x| x.raw)
}

/// Project an `Option<RawNum<f64>>` to a non-negative `Option<u32>`,
/// rounding the inner value. Returns `None` for negatives, NaN, or values
/// that don't fit.
pub fn from_raw_u32(n: Option<RawNum<f64>>) -> Option<u32> {
    let v = n?.raw?;
    if v.is_finite() && v >= 0.0 && v <= f64::from(u32::MAX) {
        Some(v.round() as u32)
    } else {
        None
    }
}
