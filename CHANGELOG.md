# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `exec` subcommand: run arbitrary commands with structured logging, exit code forwarding, and optional `--workdir` for child process working directory
- `jyq.Dockerfile` and `initium-jyq` container image variant with pre-built `jq` and `yq` tools
- Documentation for building custom images using Initium as a base
- `fetch` subcommand and `internal/fetch` package: fetch secrets/config from HTTP(S) endpoints with auth header via env var, retry with backoff, TLS options, redirect control (same-site by default), and path traversal prevention
- `render` subcommand and `internal/render` package: render templates into config files with `envsubst` (default) and Go `text/template` modes, path traversal prevention, and automatic intermediate directory creation
- `seed` subcommand: run database seed commands with structured logging and exit code forwarding (no idempotency — distinct from `migrate`)
- `migrate` subcommand: run database migration commands with structured logging, exit code forwarding, and optional idempotency via `--lock-file`
- FAQ.md with functionality, security, and deployment questions for junior-to-mid-level engineers
- Project scaffolding with Go module, CLI framework (cobra), and repo layout
- `wait-for` subcommand: wait for TCP and HTTP(S) endpoints with retries, exponential backoff, and jitter
- `internal/retry` package with configurable retry logic, backoff, and jitter
- `internal/logging` package with text and JSON structured logging, automatic secret redaction
- `internal/safety` package with path traversal prevention for file writes
- Dockerfile for multi-arch scratch-based builds (runs as non-root UID 65534)
- Makefile with build, test, lint, and Docker targets
- Helm chart skeleton with security-hardened initContainer templates
- GitHub Actions CI workflow (lint, test, build) and release workflow (container build/push with SBOM)
- Unit tests for retry logic, logging, safety path validation, and wait-for subcommand
- Examples for nginx-waitfor, postgres-migrate-seed, and config-render use cases
- Documentation: README, usage guide, security threat model, and architecture/design docs
- SECURITY.md with vulnerability reporting instructions
- Apache 2.0 LICENSE

### Security
- All file operations constrained to --workdir with path traversal prevention
- Automatic redaction of sensitive keys (token, password, secret, etc.) in logs
- Container runs as non-root with read-only root filesystem and all capabilities dropped

