# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Custom MiniJinja template filters: `sha256`, `base64_encode`, `base64_decode` available in all templates (render and seed spec files)
- `sha256` filter with optional `mode` parameter (`"hex"` default, `"bytes"` for byte array output)
- `base64_encode` / `base64_decode` filters for standard Base64 encoding and decoding with error handling for invalid input
- Filters are chainable: e.g. `{{ "data" | sha256 | base64_encode }}`
- `src/template_funcs.rs` dedicated module for template utility functions, designed for easy extension
- `docs/templating.md` documenting all template filters with usage patterns, chaining examples, and error reference
- `examples/template-functions/config.tmpl` example demonstrating sha256 and base64 filters
- Unit tests for sha256 (hex, bytes, empty, invalid mode), base64 (encode, decode, roundtrip, invalid input), and template integration (filter chaining)

### Changed
- Clarified that seed phases with only `create_if_missing` can omit the `seed_sets` field entirely (`seed_sets` defaults to empty via `#[serde(default)]`); updated integration test YAML specs accordingly

### Added
- Integration tests with docker-compose for end-to-end testing against real Postgres 16, MySQL 8.0, and nginx services (`tests/integration_test.rs`): wait-for TCP/HTTP/timeout/multiple targets, render template, fetch HTTP, exec command, seed PostgreSQL and MySQL with cross-table reference verification, create database/schema, idempotency, and reset mode
- Additional create-if-missing integration tests: 2 PostgreSQL and 2 MySQL tests using known non-existing database names (`initium_noexist_alpha`, `initium_noexist_beta`) to verify database creation, existence checks, and idempotent re-runs
- `tests/docker-compose.yml` with Postgres, MySQL, and HTTP health-check server definitions
- `tests/input/` with seed spec files and template for integration tests
- Separate GitHub Actions workflow (`.github/workflows/integration.yml`) for integration tests with service containers
- Helm chart unit tests using helm-unittest plugin (`charts/initium/tests/deployment_test.yaml`) covering deployment rendering, securityContext enforcement, disabled sampleDeployment, multiple initContainers, extraVolumes/extraVolumeMounts, image configuration, workdir mount, and labels
- `helm unittest` step added to CI helm-lint job with automatic plugin installation
- Duration unit support for all time parameters (`--timeout`, `--initial-delay`, `--max-delay`, seed phase `timeout`, seed wait-for `timeout`): accepts `ms`, `s`, `m`, `h` suffixes with decimal values (e.g. `1.5m`, `2.7s`) and combined units (e.g. `1m30s`, `2s700ms`, `18h36m4s200ms`); bare numbers default to seconds
- `src/duration.rs` module with `parse_duration` and `format_duration` utilities
- Environment variable support for all CLI flags via `INITIUM_*` prefix (e.g., `--json` → `INITIUM_JSON`, `--timeout` → `INITIUM_TIMEOUT`); flag values take precedence over env vars
- Comma-separated `INITIUM_TARGET` env var for specifying multiple wait-for endpoints
- Env var column added to all flag tables in `docs/usage.md`
- Integration tests verifying env var fallback behavior and flag precedence over env vars

### Fixed
- Added Cargo dependency caching (`Swatinem/rust-cache@v2`) to all CI and release workflow jobs for faster builds
- Added Docker BuildKit layer caching (`type=gha`) to release workflow for both main and jyq image builds
- Replaced Dockerfile stub-build caching layer with BuildKit `--mount=type=cache` for cargo registry and target directory, enabling cross-build cache reuse

### Changed
- CLI time parameter defaults now use duration units: `--timeout` default `5m` (was `300`), `--initial-delay` default `1s` (was `1000`), `--max-delay` default `30s` (was `30000`); seed phase timeout default `30s` (was `30`)
- Replaced `regex` crate with manual envsubst parser in render module for smaller binary
- Replaced `chrono` crate with `std::time::SystemTime` and Hinnant's civil calendar algorithm in logging module
- Switched rustls from default crypto backends (aws-lc-rs + ring) to ring-only
- Disabled ureq default features (gzip/brotli) to reduce dependency tree
- Database drivers (sqlite, postgres, mysql) are now optional Cargo features (all enabled by default); build with `--no-default-features --features sqlite` for minimal binary

### Removed
- Seed schema version 1 (flat `seed_sets` without phases): all seed specs now use phase-based structure
- `version` field from seed spec schema: no longer required or accepted

### Added
- Seed schema with phase-based execution: ordered phases with create → wait → seed lifecycle
- MiniJinja template rendering for seed spec files: dynamic values, conditionals, loops via `{{ env.VAR }}`
- Embedded database/schema creation via `create_if_missing` in seed phases (PostgreSQL, MySQL)
- Embedded wait-for logic: poll for tables, views, schemas, or databases with configurable per-phase and per-object timeouts
- Database trait methods: `create_database`, `create_schema`, `object_exists`, `driver_name` for SQLite, PostgreSQL, MySQL
- Cross-phase `@ref:` references: references defined in earlier phases resolve in later phases
- Documentation: schema reference, driver support tables, MiniJinja templating guide, failure modes in `docs/seeding.md`
- Example: `examples/seed/phased-seed.yaml` — multi-phase PostgreSQL seeding with wait-for, create-if-missing, and MiniJinja

### Changed
- Seed executor tests now verify data actually arrived in the database after execution, using file-based SQLite and post-execution queries to assert row counts, column values, cross-table references, env substitution, ordering, and edge cases

### Fixed
- Updated Dockerfiles (`Dockerfile`, `jyq.Dockerfile`) from `rust:1.85-alpine` to `rust:1.88-alpine` to fix release workflow failure caused by `time@0.3.47` requiring rustc 1.88.0
- Aligned all markdown table columns across documentation files (`FAQ.md`, `README.md`, `docs/security.md`, `docs/seeding.md`, `docs/usage.md`)
- Fixed clippy `collapsible_if` lint in seed executor's unique key check
- Removed dead code: unused `src/cmd/seed.rs` module (replaced by `src/seed/`)
- Suppressed unused field warning on `AutoIdConfig.id_type` (reserved for future use)
- Removed unused imports (`Arc`, `Mutex`) and unused mutable binding in seed executor tests

### Added
- Structured database seeding via `seed` subcommand with YAML/JSON spec files
- Seed tracking table (`initium_seed` by default) for idempotent seed application
- Support for PostgreSQL, MySQL, and SQLite database drivers
- Auto-generated IDs and cross-table references via `_ref` / `@ref:` syntax
- Environment variable substitution in seed values via `$env:VAR_NAME`
- Unique key detection to prevent duplicate row insertion
- Reset mode (`--reset`) to delete existing data and re-apply seeds
- Transaction safety: each seed set is applied atomically with rollback on failure
- Ordered seed sets and tables via `order` field
- Documentation: `docs/seeding.md` with full schema reference, Kubernetes usage (env vars and volume-mounted secrets), and failure modes
- Example seed specs: `examples/seed/basic-seed.yaml`, `examples/seed/sqlite-seed.yaml`, `examples/seed/env-credentials-seed.yaml`
- Unit tests for seed schema parsing, database operations, executor logic, references, idempotency, reset, and edge cases

### Changed
- Complete rewrite from Go to Rust for ~76% smaller Docker images (7.4MB → 1.8MB)
- CLI framework changed from cobra to clap
- Template engine changed from Go text/template to minijinja (Jinja2-style); access env vars via `{{ env.VAR }}`
- CI/CD workflows updated for Rust toolchain (cargo test, clippy, rustfmt)
- Dockerfiles updated to use rust:1.88-alpine builder with musl static linking

### Added
- `exec` subcommand: run arbitrary commands with structured logging, exit code forwarding, and optional `--workdir` for child process working directory
- `jyq.Dockerfile` and `initium-jyq` container image variant with pre-built `jq` and `yq` tools
- Documentation for building custom images using Initium as a base
- `fetch` subcommand and `internal/fetch` package: fetch secrets/config from HTTP(S) endpoints with auth header via env var, retry with backoff, TLS options, redirect control (same-site by default), and path traversal prevention
- `render` subcommand and `internal/render` package: render templates into config files with `envsubst` (default) and Jinja2 template modes, path traversal prevention, and automatic intermediate directory creation
- `seed` subcommand: run database seed commands with structured logging and exit code forwarding (no idempotency — distinct from `migrate`)
- `migrate` subcommand: run database migration commands with structured logging, exit code forwarding, and optional idempotency via `--lock-file`
- FAQ.md with functionality, security, and deployment questions for junior-to-mid-level engineers
- Project scaffolding with Rust/Cargo, CLI framework (clap), and repo layout
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

