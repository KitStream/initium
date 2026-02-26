# Contributing to Initium

Contributions are welcome! This guide covers how to build, test, and submit changes.

## Prerequisites

- Rust 1.88+ (stable)
- Docker (for integration tests)
- Helm + helm-unittest plugin (for Helm chart tests)

## Build

```bash
make build
# or directly:
cargo build --release
```

## Test

```bash
# Unit tests
cargo test --all-features

# Clippy lints (must pass with zero warnings)
cargo clippy --all-targets --all-features -- -D warnings

# Format check
cargo fmt -- --check

# Integration tests (requires Docker)
docker compose -f tests/docker-compose.yml up -d
INTEGRATION=1 cargo test --all-features -- --ignored
docker compose -f tests/docker-compose.yml down

# Helm chart tests
helm unittest charts/initium
```

## Adding a new subcommand

See [docs/design.md](docs/design.md) for the architecture and step-by-step guide.

In short:

1. Create `src/cmd/yourcommand.rs` with a `pub fn run(log: &Logger, ...) -> Result<(), String>`
2. Add the variant to the `Commands` enum in `src/main.rs`
3. Wire it up in the `match cli.command` block in `main()`
4. Add flags with `#[arg(...)]` and env var support via `env = "INITIUM_*"`
5. Add unit tests in the same file
6. Add integration tests in `tests/integration_test.rs`
7. Document in `docs/usage.md` and `README.md`
8. Update `Changelog.md` under `[Unreleased]`

## Pull request expectations

- All CI checks must pass (clippy, fmt, tests, helm-lint, build)
- Include a "How to verify" section in the PR description
- Keep diffs small and focused — separate refactors from features
- Update docs and CHANGELOG for user-visible changes

## Code style

- Prefer clear code over comments
- Propagate errors with context (`map_err(|e| format!("...: {}", e))`)
- Use `clippy` lints and `rustfmt` defaults
- Follow existing patterns in the codebase

## Security

- Never log secrets — use the redaction built into `Logger`
- Constrain file writes to `--workdir` via `safety::validate_file_path`
- Default to the most restrictive option

## Reporting vulnerabilities

See [SECURITY.md](SECURITY.md).
