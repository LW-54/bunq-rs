# Changelog

## 0.1.0 - Unreleased

### Added

- API-key/device installation, registration, sessions, reconnect, and renewal.
- Explicit persisted installation records with key/configuration validation and redacted debugging.
- Sandbox person/company user creation and opt-in live integration coverage.
- User, bank-account, external-account, and savings-account reads.
- Payment listing, detail, bounded history collection, creation, and batches.
- Request-inquiry creation, list, and detail reads.
- RSA-SHA256 request signing and server response verification.
- Request/response correlation and structured API error classification.
- Exact minor-unit money representation without floating-point request arithmetic.
- Deterministic transport success, signature, failure, size-limit, and correlation tests.

### Scope

This release targets private API-key/device integrations. OAuth, cards, webhooks,
draft/scheduled payments, and the broader bunq API are intentionally deferred.
