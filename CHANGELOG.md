# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `urlencode` template filter for percent-encoding strings in URLs. Useful for embedding passwords or other values containing URL-reserved characters (`@`, `%`, `:`, `/`, etc.) in connection strings.
- `dprint` formatter for Markdown, JSON, TOML, YAML, and Dockerfile with CI check (`dprint/check@v2.2`) and definition-of-done gate.
- Structured database connection config as an alternative to URL (`host`, `port`, `user`, `password`, `name`, `options` fields). Passwords with URL-reserved characters (`@`, `%`, `:`, etc.) work without encoding. Connections are built using driver-native APIs (PostgreSQL key-value DSN, MySQL `OptsBuilder`), bypassing URL parsing entirely. The `url`/`url_env` fields remain supported for backward compatibility. See [#39](https://github.com/KitStream/initium/issues/39).

### Removed

- `migrate` subcommand: removed the thin command-execution wrapper. Use `exec` subcommand instead if you need to run an external migration tool with structured logging.

### Changed

- Renamed `jyq.Dockerfile` to `Dockerfile.jyq` to follow the `Dockerfile.<variant>` convention.
- Docker dependency cache now survives version-only bumps by normalizing the root package version in a preparatory build stage.

### Fixed

- Render tests now use an RAII `EnvGuard` to restore environment variables on drop, preventing cross-test interference when tests run in parallel.
- Auto Tag workflow now uses `RELEASE_TOKEN` instead of `GITHUB_TOKEN` so the pushed tag triggers the Release workflow. Tags pushed by the default `GITHUB_TOKEN` do not trigger other workflows (GitHub Actions security feature).

## [1.3.0] - 2026-03-12

### Added

- Auto-tag workflow: CI automatically creates a git tag when `Cargo.toml` version changes on main, triggering the release workflow.
- `/release` skill for Claude Code: guided release preparation with version determination, confirmation, and PR creation.
- `ignore_columns` option for reconcile mode tables: columns listed in `ignore_columns` are included in the initial INSERT but excluded from change detection, UPDATE statements, and content hash computation. Useful for timestamps, tokens, or values managed by database triggers.

### Changed

- Moved `/release` from Claude Code command to skill (directory-based `SKILL.md` format).

### Fixed

- Replaced Dockerfile `--mount=type=cache` with dependency layer caching ("empty main" trick) for reliable Docker build caching in GitHub Actions, where `--mount=type=cache` does not persist across runners.

## [1.2.0] - 2026-03-11

### Added

- Reconcile mode for seed sets (`mode: reconcile`): declarative seeding where the spec is the source of truth. Changed rows are updated, new rows are inserted, and removed rows are deleted automatically.
- `--reconcile-all` CLI flag to override all seed sets to reconcile mode for a single run.
- `--dry-run` CLI flag to preview what changes reconciliation would make without modifying the database.
- Per-row tracking table (`initium_seed_rows`) for change detection and orphan deletion in reconcile mode.
- Content hash (`content_hash` column) on the seed tracking table for fast "anything changed?" checks before row-by-row comparison.
- Automatic migration of existing tracking tables: the `content_hash` column is added transparently on first run. Existing seed sets remain in `once` mode with no behavior change.
- CI summary job (`ci`) for branch ruleset status check compatibility.

### Changed

- Reconcile hash-skip now only applies to seed sets without `@ref:` expressions. Seed sets containing `@ref:` references always run row-level reconciliation to prevent stale foreign keys when upstream auto-generated IDs shift.
- Hash computation sorts tables by `(order, table_name)` instead of just `order` for deterministic hashing when multiple tables share the same order value.
- Dry-run mode treats `@ref:` expressions as literals to avoid failures when references haven't been populated yet (e.g., auto_id + refs within the same seed set).

### Fixed

- `--reconcile-all` now rejects seed sets where any table is missing `unique_key`, preventing reconciliation from generating identical row keys and updating/deleting wrong rows.
- Reconcile mode validation now rejects empty/whitespace-only `unique_key` entries and reserved column names like `_ref`.
- Reconcile mode validation now checks that every row contains all `unique_key` columns, preventing incomplete row keys during reconciliation.
- MySQL row tracking table now uses SHA-256 generated column (`row_key_hash`) for the primary key instead of `row_key(255)` prefix, preventing key collisions for JSON keys exceeding 255 bytes.

## [1.1.0] - 2026-02-26

### Added

- Project scaffolding with Rust/Cargo, CLI framework (clap), and repo layout
- `wait-for` subcommand: wait for TCP and HTTP(S) endpoints with retries, exponential backoff, and jitter
- `exec` subcommand: run arbitrary commands with structured logging, exit code forwarding, and optional `--workdir` for child process working directory
- `fetch` subcommand and `internal/fetch` package: fetch secrets/config from HTTP(S) endpoints with auth header via env var, retry with backoff, TLS options, redirect control (same-site by default), and path traversal prevention
- `render` subcommand and `internal/render` package: render templates into config files with `envsubst` (default) and Jinja2 template modes, path traversal prevention, and automatic intermediate directory creation
- `seed` subcommand: run database seed commands with structured logging and exit code forwarding
- Structured database seeding via `seed` subcommand with YAML/JSON spec files
- Seed tracking table (`initium_seed` by default) for idempotent seed application
- Support for PostgreSQL, MySQL, and SQLite database drivers
- Auto-generated IDs and cross-table references via `_ref` / `@ref:` syntax
- Environment variable substitution in seed values via `$env:VAR_NAME`
- Unique key detection to prevent duplicate row insertion
- Reset mode (`--reset`) to delete existing data and re-apply seeds
- Transaction safety: each seed set is applied atomically with rollback on failure
- Ordered seed sets and tables via `order` field
- Seed schema with phase-based execution: ordered phases with create → wait → seed lifecycle
- MiniJinja template rendering for seed spec files: dynamic values, conditionals, loops via `{{ env.VAR }}`
- Embedded database/schema creation via `create_if_missing` in seed phases (PostgreSQL, MySQL)
- Embedded wait-for logic: poll for tables, views, schemas, or databases with configurable per-phase and per-object timeouts
- Database trait methods: `create_database`, `create_schema`, `object_exists`, `driver_name` for SQLite, PostgreSQL, MySQL
- Cross-phase `@ref:` references: references defined in earlier phases resolve in later phases
- Custom MiniJinja template filters: `sha256`, `base64_encode`, `base64_decode` available in all templates (render and seed spec files)
- `sha256` filter with optional `mode` parameter (`"hex"` default, `"bytes"` for byte array output)
- `base64_encode` / `base64_decode` filters for standard Base64 encoding and decoding with error handling for invalid input
- Filters are chainable: e.g. `{{ "data" | sha256 | base64_encode }}`
- `src/template_funcs.rs` dedicated module for template utility functions, designed for easy extension
- Duration unit support for all time parameters (`--timeout`, `--initial-delay`, `--max-delay`, seed phase `timeout`, seed wait-for `timeout`): accepts `ms`, `s`, `m`, `h` suffixes with decimal values (e.g. `1.5m`, `2.7s`) and combined units (e.g. `1m30s`, `2s700ms`, `18h36m4s200ms`); bare numbers default to seconds
- `src/duration.rs` module with `parse_duration` and `format_duration` utilities
- Environment variable support for all CLI flags via `INITIUM_*` prefix (e.g., `--json` → `INITIUM_JSON`, `--timeout` → `INITIUM_TIMEOUT`); flag values take precedence over env vars
- Comma-separated `INITIUM_TARGET` env var for specifying multiple wait-for endpoints
- `internal/retry` package with configurable retry logic, backoff, and jitter
- `internal/logging` package with text and JSON structured logging, automatic secret redaction
- `internal/safety` package with path traversal prevention for file writes
- Dockerfile for multi-arch scratch-based builds (runs as non-root UID 65534)
- `jyq.Dockerfile` and `initium-jyq` container image variant with pre-built `jq` and `yq` tools
- Makefile with build, test, lint, and Docker targets
- Helm chart skeleton with security-hardened initContainer templates
- GitHub Actions CI workflow (lint, test, build) and release workflow (container build/push with SBOM)
- FAQ.md with functionality, security, and deployment questions for junior-to-mid-level engineers
- Documentation: README, usage guide, security threat model, architecture/design docs, seeding guide, templating guide
- Documentation for building custom images using Initium as a base
- SECURITY.md with vulnerability reporting instructions
- Apache 2.0 LICENSE
- Examples for nginx-waitfor, postgres-seed, config-render, template-functions, and phased-seed use cases
- Unit tests for retry logic, logging, safety path validation, wait-for, sha256, base64, template integration, seed schema parsing, database operations, executor logic, references, idempotency, reset, edge cases, duration parsing, and env var support
- Integration tests with docker-compose for end-to-end testing against real Postgres 16, MySQL 8.0, and nginx services
- Helm chart unit tests using helm-unittest plugin covering deployment rendering, securityContext, and configuration
- Separate GitHub Actions workflow for integration tests with service containers

### Changed

- Complete rewrite from Go to Rust for smaller Docker images (7.4MB → ~5MB)
- CLI framework changed from cobra to clap
- Template engine changed from Go text/template to minijinja (Jinja2-style); access env vars via `{{ env.VAR }}`
- CI/CD workflows updated for Rust toolchain (cargo test, clippy, rustfmt)
- Dockerfiles updated to use rust:1.88-alpine builder with musl static linking
- CLI time parameter defaults now use duration units: `--timeout` default `5m` (was `300`), `--initial-delay` default `1s` (was `1000`), `--max-delay` default `30s` (was `30000`); seed phase timeout default `30s` (was `30`)
- Replaced `regex` crate with manual envsubst parser in render module for smaller binary
- Replaced `chrono` crate with `std::time::SystemTime` and Hinnant's civil calendar algorithm in logging module
- Switched rustls from default crypto backends (aws-lc-rs + ring) to ring-only
- Disabled ureq default features (gzip/brotli) to reduce dependency tree
- Database drivers (sqlite, postgres, mysql) are now optional Cargo features (all enabled by default); build with `--no-default-features --features sqlite` for minimal binary
- Seed executor tests now verify data actually arrived in the database after execution
- Clarified that seed phases with only `create_if_missing` can omit the `seed_sets` field entirely (`seed_sets` defaults to empty via `#[serde(default)]`)
- Improved crates.io metadata: keyword-rich description, `rust-version = "1.88"` MSRV, authors, `documentation` pointing to docs.rs, and `exclude` to reduce published crate size
- Added `#![doc = include_str!("../README.md")]` to `src/main.rs` so docs.rs renders the README as the crate landing page
- Release workflow now publishes to crates.io automatically on tag push (requires `CARGO_REGISTRY_TOKEN` secret)

### Fixed

- Updated Dockerfiles (`Dockerfile`, `jyq.Dockerfile`) from `rust:1.85-alpine` to `rust:1.88-alpine` to fix release workflow failure caused by `time@0.3.47` requiring rustc 1.88.0
- Aligned all markdown table columns across documentation files
- Fixed clippy `collapsible_if` lint in seed executor's unique key check
- Removed dead code: unused `src/cmd/seed.rs` module (replaced by `src/seed/`)
- Suppressed unused field warning on `AutoIdConfig.id_type` (reserved for future use)
- Removed unused imports (`Arc`, `Mutex`) and unused mutable binding in seed executor tests
- Added Cargo dependency caching (`Swatinem/rust-cache@v2`) to all CI and release workflow jobs for faster builds
- Added Docker BuildKit layer caching (`type=gha`) to release workflow for both main and jyq image builds
- Replaced Dockerfile stub-build caching layer with BuildKit `--mount=type=cache` for cargo registry and target directory, enabling cross-build cache reuse

### Removed

- Seed schema version 1 (flat `seed_sets` without phases): all seed specs now use phase-based structure
- `version` field from seed spec schema: no longer required or accepted

### Security

- All file operations constrained to --workdir with path traversal prevention
- Automatic redaction of sensitive keys (token, password, secret, etc.) in logs
- Container runs as non-root with read-only root filesystem and all capabilities dropped
