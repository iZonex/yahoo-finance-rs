# Contributing to `yahoo-finance-rs`

Thanks for considering a contribution. This is a community-driven port of the
Python [`yfinance`](https://github.com/ranaroussi/yfinance) library — bug
reports, doc fixes, and feature PRs are all welcome.

For issues, use the [bug report](.github/ISSUE_TEMPLATE/bug_report.yml) or
[feature request](.github/ISSUE_TEMPLATE/feature_request.yml) templates.

## Code of Conduct

By participating you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).
Security issues go through [SECURITY.md](SECURITY.md), not public issues.

## Quick start

```bash
git clone https://github.com/iZonex/yahoo-finance-rs.git
cd yahoo-finance-rs
rustup component add rustfmt clippy
```

With [`just`](https://github.com/casey/just) installed:

```bash
just check     # fmt --check + clippy + test (run before every PR)
just fmt       # apply rustfmt
just example ticker_history AAPL
```

Or the raw cargo equivalents:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
```

These are exactly what CI runs.

## Pull-request checklist

- [ ] `just check` (or the three cargo commands above) is green.
- [ ] Public items have rustdoc comments.
- [ ] New endpoints / wire-format changes have a unit test parsing a
      representative payload — see `mod tests` in `src/history.rs` for the
      `serde_json::json!` pattern. Don't hit live Yahoo in unit tests.
- [ ] User-facing changes have an entry under `## [Unreleased]` in
      [`CHANGELOG.md`](CHANGELOG.md).

## Branches and commits

- Branch off `main` with a descriptive name (`feat/recommendations`,
  `fix/repair-split-edge-case`).
- Imperative subject line is preferred (`Add analyst recommendations endpoint`).
- PRs are squash-merged; multi-commit history on the branch is fine.

## Architecture

`src/lib.rs` is the public surface and the crate is one module per upstream
concept (`history.rs`, `quote.rs`, `info.rs`, `fundamentals.rs`, …). See
[README.md → Architecture](README.md#architecture) for the full map.

### Adding a new endpoint

1. Add a `serde::Deserialize` struct in the right module (or a new one).
2. Add a method to `Ticker` for per-symbol endpoints, or a free function /
   builder for cross-cutting ones.
3. Use `client.get_json_crumb(...)` if the endpoint needs a crumb,
   `client.get_json(...)` otherwise.
4. Add a `mod tests` block with a `serde_json::json!` fixture.
5. Re-export public types from `lib.rs`.

Integration tests in `tests/` use [`httpmock`](https://docs.rs/httpmock) and
inject a base host via `YfClient::builder().base_host(...)`. Live network
tests are not run in CI.

## License

By contributing, you agree your work is licensed under the
[Apache License 2.0](LICENSE).
