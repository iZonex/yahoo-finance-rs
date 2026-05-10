# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Initial public release.
- `Ticker` API with `history`, `info`, `fast_info`, `dividends`, `splits`,
  `actions`, `capital_gains`, `financials`, `balance_sheet`, `cashflow`,
  `quarterly_*` variants, `holders` family, `recommendations`,
  `analyst_price_targets`, `earnings_*` estimates, `option_chain`,
  `calendar`, `isin`, `news`.
- `download` for multi-ticker batch OHLCV.
- `Search` and `Lookup` clients.
- `Market`, `Sector`, `Industry` domain objects.
- `YfClient` with cookie/crumb session, rate-limit handling, retries.
- Apache-2.0 licensed.
