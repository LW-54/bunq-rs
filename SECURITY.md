# Security Policy

## Scope

Security reports are welcome for the `bunq-api` crate, especially issues involving:

- Credential or private-key exposure
- Request signing or response verification
- Request/response correlation
- HTTPS enforcement
- Payment replay or retry behavior
- Malformed response handling

## Reporting

Please do not publish credential-related vulnerabilities or working payment
exploits in a public issue. Contact the repository maintainers privately through
GitHub before disclosure.

Do not include real API keys, private keys, session tokens, or production
account information in a report.

## Status

This project has not undergone an independent security audit. Sandbox and
local deterministic tests provide useful evidence but do not guarantee
production safety.
