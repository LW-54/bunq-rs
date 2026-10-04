# bunq-rs

A focused Rust client for private bunq API-key/device integrations.

The SDK lives in [`crates/bunq-api`](crates/bunq-api). It currently supports:

- Sandbox user creation
- Installation, device registration, sessions, reconnect, and renewal
- Bank, external, and savings account reads
- Payment history, details, single payments, and payment batches
- Request-inquiry creation and reads
- RSA request signing and response verification
- Exact minor-unit money handling
- Deterministic transport tests and opt-in live Sandbox validation

This is a focused `0.1.0` release candidate, not a complete mirror of bunq's
API. See [`crates/bunq-api/SUPPORT.md`](crates/bunq-api/SUPPORT.md) for scope,
non-goals, and unsupported domains.

## Quick Start

```toml
[dependencies]
bunq-api = "0.1"
```

Read [`crates/bunq-api/README.md`](crates/bunq-api/README.md) for installation,
persistence, payment, testing, and security guidance.

## Status

The project was developed with AI assistance and has deterministic transport
coverage plus live bunq Sandbox testing. It has not received an independent
security audit. Review credential handling and use controlled accounts before
using real funds.

## License

MIT. See [`LICENSE`](LICENSE).
