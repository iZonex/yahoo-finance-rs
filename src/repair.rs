//! Local-only repair passes for [`History`].
//!
//! Yahoo's chart endpoint occasionally returns rows with one of these defects:
//!
//! 1. **100× currency mixup** — single bars whose OHLC are off by a factor of
//!    100 because Yahoo flipped between pence and pounds (or cents and
//!    dollars).
//! 2. **Zero / NaN closes** — bars with traded volume but missing prices.
//! 3. **Bad split adjustment** — Yahoo failed to back-apply a recent split,
//!    so old bars are off by exactly the split ratio in one direction.
//!
//! Each pass is conservative: it requires *strong* evidence before mutating
//! values, and it never makes a network call. The
//! [`History::repair`](crate::History::repair) method runs all of them on an
//! existing payload; [`HistoryBuilder::repair`](crate::HistoryBuilder::repair)
//! does the same automatically after fetching.
//!
//! These checks are inspired by `yfinance/scrapers/history.py` (the
//! `_fix_unit_*`, `_fix_zeroes`, `_fix_bad_stock_splits` family). They are
//! deliberately a strict subset — the upstream library performs additional
//! passes that require re-fetching data at a finer interval, which we don't
//! attempt here.

use crate::history::{Action, History, OhlcvRow};

/// Counters describing what one pass of repair changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RepairReport {
    /// Bars whose OHLC was scaled by 100×/0.01× to fix a currency mixup.
    pub currency_unit_fixes: u32,
    /// Zero/NaN bars filled by interpolating from neighbors.
    pub zero_fills: u32,
    /// Pre-split bars rescaled to undo a missing split adjustment.
    pub split_fixes: u32,
}

impl RepairReport {
    /// Did any pass make a change?
    pub fn any(&self) -> bool {
        self.currency_unit_fixes > 0 || self.zero_fills > 0 || self.split_fixes > 0
    }
}

/// Run all repair passes against `history` in place.
///
/// Order matters: split fix runs first (so subsequent unit/zero passes see the
/// rescaled series), then currency-unit, then zero-fill.
pub fn repair_history(history: &mut History) -> RepairReport {
    let split_fixes = fix_bad_splits(&mut history.rows, &history.actions);
    let currency_unit_fixes = fix_unit_mixups(&mut history.rows);
    let zero_fills = fix_zero_closes(&mut history.rows);
    RepairReport {
        currency_unit_fixes,
        zero_fills,
        split_fixes,
    }
}

/// Pass 1: detect bars where OHLC is ~100× off the local 5-bar median and rescale them.
///
/// Conservative: a bar is only flagged if **all four** of OHLC are within 5%
/// of the 100× threshold and the next/prev bar are both within normal range.
#[allow(clippy::needless_range_loop)] // index needed for mutable borrow inside loop
fn fix_unit_mixups(rows: &mut [OhlcvRow]) -> u32 {
    if rows.len() < 5 {
        return 0;
    }
    let mut fixed = 0u32;
    let medians = rolling_median_close(rows, 5);
    for i in 0..medians.len() {
        let local = medians[i];
        if !local.is_finite() || local <= 0.0 {
            continue;
        }
        let row = &rows[i];
        if !row.close.is_finite() || row.close <= 0.0 {
            continue;
        }
        let ratio = row.close / local;
        if let Some(scale) = unit_scale(ratio) {
            // Confirm with open/high/low.
            let confirms = [row.open / local, row.high / local, row.low / local]
                .iter()
                .filter(|x| x.is_finite() && (**x).abs() > 0.0)
                .filter(|x| unit_scale(**x) == Some(scale))
                .count();
            if confirms >= 2 {
                let r = &mut rows[i];
                r.open *= scale;
                r.high *= scale;
                r.low *= scale;
                r.close *= scale;
                if r.adj_close.is_finite() {
                    r.adj_close *= scale;
                }
                fixed += 1;
            }
        }
    }
    fixed
}

/// If `ratio` is within ±5% of 100 or 0.01, return the inverse factor; else `None`.
fn unit_scale(ratio: f64) -> Option<f64> {
    if !ratio.is_finite() || ratio <= 0.0 {
        return None;
    }
    if (ratio - 100.0).abs() / 100.0 < 0.05 {
        return Some(0.01);
    }
    if (ratio - 0.01).abs() / 0.01 < 0.05 {
        return Some(100.0);
    }
    None
}

/// Pass 2: bars where OHLC is zero/NaN but adj_close is valid get linearly
/// interpolated from neighbors.
fn fix_zero_closes(rows: &mut [OhlcvRow]) -> u32 {
    if rows.len() < 3 {
        return 0;
    }
    let mut fixed = 0u32;
    for i in 1..rows.len().saturating_sub(1) {
        if is_bad(&rows[i]) {
            // Find nearest valid prev and next.
            let prev = (0..i).rev().find(|&j| !is_bad(&rows[j]));
            let next = (i + 1..rows.len()).find(|&j| !is_bad(&rows[j]));
            if let (Some(p), Some(n)) = (prev, next) {
                let denom = (n - p) as f64;
                let frac = (i - p) as f64 / denom;
                let interp = |a: f64, b: f64| -> f64 {
                    if a.is_finite() && b.is_finite() {
                        a + (b - a) * frac
                    } else {
                        f64::NAN
                    }
                };
                let pp = rows[p].clone();
                let nn = rows[n].clone();
                let r = &mut rows[i];
                r.open = interp(pp.open, nn.open);
                r.high = interp(pp.high, nn.high);
                r.low = interp(pp.low, nn.low);
                r.close = interp(pp.close, nn.close);
                if !r.adj_close.is_finite() {
                    r.adj_close = interp(pp.adj_close, nn.adj_close);
                }
                fixed += 1;
            }
        }
    }
    fixed
}

fn is_bad(r: &OhlcvRow) -> bool {
    let any_zero = [r.open, r.high, r.low, r.close]
        .iter()
        .any(|x| !x.is_finite() || *x == 0.0);
    any_zero
}

/// Pass 3: detect a single huge ratio change between consecutive bars matching
/// a recorded split, in the wrong direction; rescale the older portion.
fn fix_bad_splits(rows: &mut [OhlcvRow], actions: &[Action]) -> u32 {
    let mut fixed = 0u32;
    if rows.len() < 4 {
        return 0;
    }
    // Estimate baseline daily volatility (median absolute return over 30 bars).
    let baseline = baseline_volatility(rows);
    let trigger = (5.0 * baseline).max(0.50);

    for action in actions {
        let Action::Split {
            date,
            numerator,
            denominator,
        } = action
        else {
            continue;
        };
        if *denominator <= 0.0 || *numerator <= 0.0 {
            continue;
        }
        let factor = numerator / denominator;
        if !(2.0..=20.0).contains(&factor) {
            continue;
        }
        // Find the bar right after the split.
        let Some(idx) = rows
            .iter()
            .position(|r| r.timestamp.timestamp() >= date.timestamp())
        else {
            continue;
        };
        if idx == 0 || idx >= rows.len() {
            continue;
        }
        let prev = rows[idx - 1].close;
        let cur = rows[idx].close;
        if !prev.is_finite() || !cur.is_finite() || prev <= 0.0 || cur <= 0.0 {
            continue;
        }
        let ratio = prev / cur;
        // Yahoo applied the split correctly: pre and post bars are on the
        // same scale, so prev/cur ≈ 1.
        // Yahoo failed to apply: pre-split rows show the *old* (large) price
        // while post-split rows show the new price, so prev/cur ≈ factor and
        // the gap is many times the baseline volatility.
        if ratio > factor * 0.95 && ratio < factor * 1.05 && (factor - 1.0) > trigger {
            for r in rows.iter_mut().take(idx) {
                r.open /= factor;
                r.high /= factor;
                r.low /= factor;
                r.close /= factor;
                if r.adj_close.is_finite() {
                    r.adj_close /= factor;
                }
            }
            fixed += 1;
        }
    }
    fixed
}

fn baseline_volatility(rows: &[OhlcvRow]) -> f64 {
    let n = rows.len().min(30).saturating_sub(1);
    if n < 2 {
        return 0.05;
    }
    let mut returns: Vec<f64> = Vec::with_capacity(n);
    let start = rows.len() - n - 1;
    for w in rows[start..].windows(2) {
        let (a, b) = (w[0].close, w[1].close);
        if a.is_finite() && b.is_finite() && a > 0.0 {
            returns.push(((b - a) / a).abs());
        }
    }
    if returns.is_empty() {
        return 0.05;
    }
    returns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    returns[returns.len() / 2]
}

fn rolling_median_close(rows: &[OhlcvRow], window: usize) -> Vec<f64> {
    let half = window / 2;
    let mut out = vec![f64::NAN; rows.len()];
    for (i, slot) in out.iter_mut().enumerate() {
        let lo = i.saturating_sub(half);
        let hi = (i + half + 1).min(rows.len());
        let mut buf: Vec<f64> = rows[lo..hi]
            .iter()
            .map(|r| r.close)
            .filter(|x| x.is_finite() && *x > 0.0)
            .collect();
        if buf.is_empty() {
            continue;
        }
        buf.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mid = buf.len() / 2;
        *slot = if buf.len() % 2 == 0 {
            0.5 * (buf[mid - 1] + buf[mid])
        } else {
            buf[mid]
        };
    }
    out
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn row(ts: i64, close: f64) -> OhlcvRow {
        OhlcvRow {
            timestamp: chrono::Utc.timestamp_opt(ts, 0).unwrap(),
            open: close,
            high: close,
            low: close,
            close,
            adj_close: close,
            volume: 1_000_000,
            dividend: 0.0,
            split: 0.0,
            capital_gain: 0.0,
        }
    }

    #[test]
    fn fixes_100x_spike() {
        let mut rows = vec![
            row(100, 100.0),
            row(200, 101.0),
            row(300, 100.5),
            // Spike — Yahoo accidentally reported in pence.
            row(400, 10_050.0),
            row(500, 100.6),
            row(600, 100.7),
            row(700, 100.8),
        ];
        let n = fix_unit_mixups(&mut rows);
        assert_eq!(n, 1);
        assert!((rows[3].close - 100.5).abs() < 1.0);
    }

    #[test]
    fn fills_bad_close() {
        let mut rows = vec![row(100, 10.0), row(200, 0.0), row(300, 12.0)];
        let n = fix_zero_closes(&mut rows);
        assert_eq!(n, 1);
        assert!((rows[1].close - 11.0).abs() < 1e-9);
    }

    #[test]
    fn fixes_unapplied_split() {
        let action = Action::Split {
            date: chrono::Utc.timestamp_opt(400, 0).unwrap(),
            numerator: 4.0,
            denominator: 1.0,
        };
        // Pre-split closes around 400 — should be 100 (post-split adjusted).
        let mut rows = vec![
            row(100, 400.0),
            row(200, 408.0),
            row(300, 412.0),
            row(400, 100.0),
            row(500, 102.0),
            row(600, 101.5),
        ];
        let n = fix_bad_splits(&mut rows, &[action]);
        assert_eq!(n, 1);
        assert!((rows[0].close - 100.0).abs() < 1e-9);
        assert!((rows[3].close - 100.0).abs() < 1e-9);
    }
}
