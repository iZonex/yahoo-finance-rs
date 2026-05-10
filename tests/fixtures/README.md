# Test fixtures

Recorded HTTP responses replayed by offline integration tests.

Layout: `{endpoint}_{symbol}.{ext}`. Today's `endpoint` labels:

| Endpoint                  | Used by                |
| ------------------------- | ---------------------- |
| `history_chart`           | `history` module       |
| `quote_v7`                | `quote` (FastInfo)     |
| `info_quoteSummary`       | `info`                 |
| `holders_quoteSummary`    | `holders`              |
| `fundamentals_timeseries` | `fundamentals`         |
| `options_v7`              | `options`              |
| `search`                  | `search`               |
| `lookup`                  | `lookup`               |
| `domain_quoteSummary`     | `domain` (sector/ind.) |
| `market_summary`          | `domain::Market`       |

## Recording

```bash
just test-record offline_history_uses_recorded_fixture   # one test
just test-record                                          # all live tests
```

Recording requires `--features test-mode` (the justfile sets it for you) and
hits the real Yahoo API. Override the destination with `YF_FIXDIR=/tmp/yf`.

## Replay

```bash
just test-offline    # default — used by CI
```
