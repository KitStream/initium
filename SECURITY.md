# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| latest  | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability in Initium, please report it responsibly.

**Do NOT open a public GitHub issue for security vulnerabilities.**

Instead, email: **security@kitstream.dev**

Include:
- Description of the vulnerability
- Steps to reproduce
- Impact assessment
- Suggested fix (if any)

We will acknowledge receipt within 48 hours and aim to provide a fix within 7 days for critical issues.

## Security Design

Initium is designed with security as a first-class concern:

- **No privilege escalation**: runs as non-root (UID 65534) by default
- **Read-only root filesystem**: compatible with `readOnlyRootFilesystem: true`
- **Dropped capabilities**: all Linux capabilities are dropped
- **Path traversal prevention**: all file writes are constrained to `--workdir`
- **No secret leakage**: sensitive values in logs are automatically redacted
- **Explicit network targets**: no default outbound connections; all targets must be user-specified
- **Conservative timeouts**: 5s default timeout, 60 max retries, capped backoff
- **Minimal base image**: built `FROM scratch` with only CA certificates

See [docs/security.md](docs/security.md) for the full threat model.

