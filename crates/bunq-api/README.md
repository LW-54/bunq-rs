# bunq-api

A typed, fallible Rust client for the bunq API.

See [SUPPORT.md](SUPPORT.md) for the supported endpoint surface and explicit
non-goals.

This crate is currently a focused `0.1.0` release candidate for private
API-key/device integrations. It is not a complete mirror of bunq's API.

## Security and development disclosure

This project was developed with AI assistance and reviewed with deterministic
transport tests and bunq Sandbox integration tests. It has not undergone an
independent security audit. Before using real funds, review the code, protect
all credentials with application-managed secret storage, and test with a
controlled account. Sandbox tests provide evidence of protocol compatibility,
not a guarantee of production safety.

The first implementation targets bunq's API-key/device flow. It includes:

- RSA-SHA256 PKCS#1 v1.5 request signing
- server response signature verification
- installation, device registration, and session creation
- redacted persisted installation context debugging
- strict bunq response and error envelope parsing
- configurable request timeout and required bunq headers

## Installation flow

```rust,no_run
use bunq_api::{Client, Method, install_device};

# async fn example() -> bunq_api::Result<()> {
let (installation, session) = install_device(
	std::env::var("BUNQ_API_KEY").map_err(|_| bunq_api::Error::InvalidConfiguration("missing API key".into()))?,
	"https://api.bunq.com/v1",
	"my-app/0.1",
	"my-device",
).await?;

// Persist installation.to_persisted() using an application-controlled secret store.
let client = Client::from_installation(&installation, session.token())?;
let user = client.get_user().await?;
let accounts = client
	.list_monetary_account_banks(session.user_id())
	.await?;
# Ok(())
# }
```

On later runs, keep the installation context and open a new temporary session
without reinstalling the device:

```rust,no_run
# async fn example(context: &bunq_api::InstallationContext) -> bunq_api::Result<()> {
let (mut client, session) = bunq_api::Client::connect(context).await?;
let _new_session = client.renew_session(context).await?;
# let _ = session;
# Ok(())
# }
```

The API key, private key, installation token, and session token are credentials.
Do not log them or store them in an unprotected location. Payment writes will
require exact-body signing and will not be automatically replayed after an
uncertain network outcome.

`InstallationContext` is runtime state. To persist it explicitly, call
`installation.to_persisted().to_json()`, protect the resulting credential-bearing
JSON with your application's secret storage, and restore it with
`PersistedInstallation::from_json(...).into_context()`. The SDK does not encrypt
this record or choose a storage backend for you.

OAuth, broad resource coverage, automatic payment retries, and live-network
examples are intentionally deferred until the protocol test suite is expanded.

The release-quality baseline is validated with workspace tests, deterministic
transport tests, strict clippy, rustfmt, documentation builds, and an opt-in
live sandbox workflow. See [CHANGELOG.md](CHANGELOG.md) for the current scope.

## Sandbox integration test

Normal tests do not contact bunq. To explicitly create a disposable sandbox
user and exercise installation, device registration, and session creation:

```bash
BUNQ_API_RUN_SANDBOX_TESTS=1 cargo test -p bunq-api --test sandbox_live -- --nocapture
```

The test creates a fresh sandbox user and does not print its API key. Sandbox
credentials must still be treated as secrets and should not be committed.

The deterministic mock transport test validates exact request headers, body
signatures, response signatures, and response IDs without network access:

```bash
cargo test -p bunq-api --test transport_mock
```

## Targeted test workflow

During development, run the smallest relevant layer first:

```bash
# Resource models, money, envelopes, and configuration
cargo test -p bunq-api --lib

# Signing and HTTP success behavior
cargo test -p bunq-api --test transport_mock

# HTTP failures, signatures, correlation, and response limits
cargo test -p bunq-api --test transport_failures --test transport_correlation

# Full live sandbox workflow, only when the changed feature touches sandbox behavior
BUNQ_API_RUN_SANDBOX_TESTS=1 cargo test -p bunq-api --test sandbox_live -- --nocapture

# Release checkpoint
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

The deterministic integration tests use cached 1024-bit test fixtures for
protocol mechanics. Production and sandbox installation always generate fresh
2048-bit RSA keys.

The standard CI workflow never creates sandbox users or contacts bunq. Run the
opt-in sandbox test separately when a sandbox-compatible feature changes.

The first typed resource methods are `Client::get_user` and
`Client::list_monetary_account_banks`. Direct detail methods are available as
`Client::get_monetary_account_bank` and `Client::get_payment`. Their response
models preserve unknown fields so the SDK can evolve with bunq's response
format.

For bounded transaction-history collection, use
`Client::list_all_payments(user_id, account_id, count, max_pages)`. It follows
only bunq's returned older-page URLs, performs no retries, and stops at the
caller-supplied page limit.

In bunq terminology, a `Payment` is the transaction record for money moving
into or out of a monetary account. A normal outgoing bank transfer is one
kind of payment. Bunq also models payment batches, draft payments, scheduled
payments, and incoming payments under related payment APIs. That is why this
crate uses `Payment` for transaction history while reserving more specific
request types for individual outgoing payments.

Payment creation is available through `Client::create_payment`. Build a
validated `PaymentRequest`; the client serializes and signs the exact request
body automatically. Payment creation makes one network attempt and does not
retry automatically. If the outcome is uncertain, list payments and inspect
the account before taking recovery action, since replaying a payment can
duplicate the transfer.

For IBAN-heavy workflows, parse and validate the IBAN once and reuse it:

```rust,no_run
# fn example() -> bunq_api::Result<()> {
let iban = bunq_api::Iban::parse("NL91 ABNA 0417 1643 00")?;
let payment = bunq_api::PaymentRequest::new_iban(
	"10.00",
	"EUR",
	&iban,
	Some("Invoice 123".to_owned()),
)?;
# let _ = payment;
# Ok(())
# }
```

IBAN parsing normalizes spaces and case and validates the MOD-97 checksum
before the value is serialized into a request.

To request money instead of sending it, use `RequestInquiryRequest` and
`Client::create_request_inquiry`. This creates a bunq request for a
counterparty to approve; it is a different operation from an outgoing
`Payment`, even though both are tied to a monetary account.

Request amounts use `Money`, which stores signed minor units and a decimal
scale rather than floating-point values. This avoids rounding errors in
financial calculations while still serializing to bunq's decimal wire format.

Multiple outgoing payments can be submitted with `PaymentBatchRequest` and
`Client::create_payment_batch`. The SDK enforces bunq's documented maximum of
350 payments per batch. Batch writes, like single payments, are one-shot and
are not automatically retried.

Errors expose operational classification helpers such as `is_rate_limited()`,
`is_authentication_failure()`, `is_server_failure()`, and
`is_retryable_transport_error()`. These describe what happened; they do not
automatically retry requests. The bunq documentation does not define a stable
`Retry-After` header, so callers should apply bounded backoff using the error
description and only repeat idempotent operations.