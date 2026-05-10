//! Shared enums: [`Period`] and [`Interval`].

use std::fmt;
use std::str::FromStr;

use crate::error::Error;

/// A relative time window passed to Yahoo's chart endpoint.
///
/// Mutually exclusive with explicit `start`/`end` timestamps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Period {
    /// 1 day.
    D1,
    /// 5 days.
    D5,
    /// 1 month.
    M1,
    /// 3 months.
    M3,
    /// 6 months.
    M6,
    /// 1 year.
    Y1,
    /// 2 years.
    Y2,
    /// 5 years.
    Y5,
    /// 10 years.
    Y10,
    /// Year-to-date.
    Ytd,
    /// All available history.
    Max,
}

impl Period {
    /// Wire-format string accepted by Yahoo (e.g. `"1mo"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Period::D1 => "1d",
            Period::D5 => "5d",
            Period::M1 => "1mo",
            Period::M3 => "3mo",
            Period::M6 => "6mo",
            Period::Y1 => "1y",
            Period::Y2 => "2y",
            Period::Y5 => "5y",
            Period::Y10 => "10y",
            Period::Ytd => "ytd",
            Period::Max => "max",
        }
    }
}

impl fmt::Display for Period {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Period {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "1d" => Period::D1,
            "5d" => Period::D5,
            "1mo" => Period::M1,
            "3mo" => Period::M3,
            "6mo" => Period::M6,
            "1y" => Period::Y1,
            "2y" => Period::Y2,
            "5y" => Period::Y5,
            "10y" => Period::Y10,
            "ytd" => Period::Ytd,
            "max" => Period::Max,
            other => return Err(Error::invalid(format!("unknown period `{other}`"))),
        })
    }
}

/// Sampling interval for the chart endpoint.
///
/// Note: Yahoo restricts intraday intervals to recent history only. For example,
/// `1m` is limited to the last 7 days. Combining a long [`Period`] with a small
/// `Interval` may yield [`Error::InvalidPeriod`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Interval {
    /// 1 minute.
    M1,
    /// 2 minutes.
    M2,
    /// 5 minutes.
    M5,
    /// 15 minutes.
    M15,
    /// 30 minutes.
    M30,
    /// 60 minutes.
    M60,
    /// 90 minutes.
    M90,
    /// 1 hour (alias of `60m`).
    H1,
    /// 1 day.
    D1,
    /// 5 days.
    D5,
    /// 1 week.
    W1,
    /// 1 month.
    Mo1,
    /// 3 months.
    Mo3,
}

impl Interval {
    /// Wire-format string accepted by Yahoo (e.g. `"1d"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Interval::M1 => "1m",
            Interval::M2 => "2m",
            Interval::M5 => "5m",
            Interval::M15 => "15m",
            Interval::M30 => "30m",
            Interval::M60 => "60m",
            Interval::M90 => "90m",
            Interval::H1 => "1h",
            Interval::D1 => "1d",
            Interval::D5 => "5d",
            Interval::W1 => "1wk",
            Interval::Mo1 => "1mo",
            Interval::Mo3 => "3mo",
        }
    }

    /// Whether the interval samples *within* a trading day.
    pub fn is_intraday(self) -> bool {
        matches!(
            self,
            Interval::M1
                | Interval::M2
                | Interval::M5
                | Interval::M15
                | Interval::M30
                | Interval::M60
                | Interval::M90
                | Interval::H1
        )
    }
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Interval {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "1m" => Interval::M1,
            "2m" => Interval::M2,
            "5m" => Interval::M5,
            "15m" => Interval::M15,
            "30m" => Interval::M30,
            "60m" => Interval::M60,
            "90m" => Interval::M90,
            "1h" => Interval::H1,
            "1d" => Interval::D1,
            "5d" => Interval::D5,
            "1wk" => Interval::W1,
            "1mo" => Interval::Mo1,
            "3mo" => Interval::Mo3,
            other => return Err(Error::invalid(format!("unknown interval `{other}`"))),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn period_round_trip() {
        for p in [Period::D1, Period::M1, Period::Y1, Period::Ytd, Period::Max] {
            assert_eq!(Period::from_str(p.as_str()).unwrap(), p);
        }
    }

    #[test]
    fn interval_round_trip() {
        for i in [
            Interval::M1,
            Interval::M30,
            Interval::H1,
            Interval::D1,
            Interval::Mo3,
        ] {
            assert_eq!(Interval::from_str(i.as_str()).unwrap(), i);
        }
    }

    #[test]
    fn rejects_unknown() {
        assert!(Period::from_str("42x").is_err());
        assert!(Interval::from_str("17s").is_err());
    }
}
