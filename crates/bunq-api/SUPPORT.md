# Supported API Surface

`bunq-api` currently targets the private API-key/device integration workflow.
OAuth and PSD2 public-project flows are intentionally out of scope for the
first release.

The current `0.1.0` release candidate is suitable for private sandbox and
API-key integrations covering the supported operations below. It should not
be treated as a complete general-purpose bunq SDK.

## Supported

| Area | Operations |
| --- | --- |
| Installation | Installation, device registration, session creation, reconnect, renewal |
| Sandbox | Disposable person/company users |
| Users | Current user lookup |
| Accounts | Bank, external, and savings account list/detail reads |
| Payments | List/detail, bounded history collection, single creation, payment batches, typed IBAN counterparties |
| Requests | Request-inquiry creation, list/detail |
| Pagination | Payment, request-inquiry, and payment-batch pagination metadata |
| Security | RSA-SHA256 signing, response signatures, request correlation |
| Persistence | Explicit credential-bearing installation JSON record |

## Not Yet Supported

- Draft payments
- Scheduled payments
- Payment/request batch update operations
- Cards and card transactions
- Attachments and note attachments
- Webhooks and notification filters
- Statements, events, exports, and currency conversion
- bunq.me, invoices, whitelists, and payment-service-provider APIs
- OAuth and PSD2 provisioning flows

Unsupported operations should be treated as absent from the public API. The
low-level transport is not a guarantee that an arbitrary endpoint has been
modeled or tested.
