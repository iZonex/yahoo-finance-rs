# Security Policy

## Supported versions

This crate is pre-1.0 and only the latest published version on crates.io
receives security fixes.

| Version  | Supported |
| -------- | --------- |
| latest   | ✅        |
| older    | ❌        |

## Reporting a vulnerability

**Please do not open a public GitHub issue for security problems.**

Use GitHub's private vulnerability reporting:

1. Go to <https://github.com/iZonex/yahoo-finance-rs/security/advisories/new>.
2. Describe the issue with enough detail to reproduce — affected version,
   minimal repro, and the impact you observed.

You should receive an acknowledgement within a few business days. We'll work
with you on a fix and a coordinated disclosure timeline (typically 30–90 days
depending on severity), and credit you in the advisory unless you'd rather
stay anonymous.

## Out of scope

This crate is a read-only client for the public Yahoo Finance HTTP API. The
following are explicitly **not** vulnerabilities in this project:

- Yahoo rate-limiting, throttling, or response changes.
- Issues in upstream dependencies — please report those to their respective
  projects (we'll bump the version once a fix lands).
- Denial-of-service caused by passing untrusted input directly to API
  parameters; sanitize before use.
