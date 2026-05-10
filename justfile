set shell := ["bash", "-cu"]
set dotenv-load := true

# Cargo features tests are compiled with. `test-mode` enables fixture recording.
FEATURES := 'test-mode'
TEST_THREADS := '1'

default: check

# fmt --check + clippy + offline tests, exactly what CI runs
check:
    cargo fmt --all -- --check
    cargo clippy --all-targets --all-features -- -D warnings
    just test-offline

fmt:
    cargo fmt --all

# Replay recorded fixtures — fast, no network, runs in CI
test-offline +args='':
    cargo test --features {{FEATURES}} -- {{args}}

# Hit the live Yahoo API without writing fixtures (verifies parsers still match)
test-live +args='':
    YF_LIVE=1 cargo test --features {{FEATURES}} -- --include-ignored --test-threads={{TEST_THREADS}} {{args}}

# Hit the live Yahoo API and (re)record fixtures into tests/fixtures/
test-record +args='':
    YF_RECORD=1 cargo test --features {{FEATURES}} -- --ignored --test-threads={{TEST_THREADS}} {{args}}

# Two-phase: record live, then replay offline
test-full +args='':
    @set -euo pipefail; \
    echo "▶ Phase 1/2 — recording live"; \
    YF_RECORD=1 cargo test --features {{FEATURES}} -- --ignored --test-threads={{TEST_THREADS}} {{args}}; \
    echo "▶ Phase 2/2 — replaying offline"; \
    cargo test --features {{FEATURES}} -- {{args}}

doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features

# usage: just example ticker_history AAPL
example name *args:
    cargo run --example {{name}} -- {{args}}
