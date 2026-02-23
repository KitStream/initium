# Architecture & Design

## Overview

Initium is a single Rust binary with multiple subcommands, each addressing a common initContainer use case. It follows the Unix philosophy: each subcommand does one thing well, with composability via sequential initContainers in a pod spec.

```
┌─────────────────────────────────────────┐
│              src/main.rs                │
│          (CLI root, clap commands)      │
├─────────────────────────────────────────┤
│               src/cmd/                  │
│   ┌──────────┬──────────┬────────────┐  │
│   │ wait-for │ migrate  │  render    │  │
│   │ seed     │  fetch   │   exec     │  │
│   └──────────┴──────────┴────────────┘  │
├─────────────────────────────────────────┤
│          Shared Libraries               │
│  ┌────────┬────────┬────────┬────────┐  │
│  │ retry  │ render │ fetch  │logging │  │
│  │        │        │        │safety  │  │
│  └────────┴────────┴────────┴────────┘  │
└─────────────────────────────────────────┘
```

## Directory Structure

```
src/
  main.rs             CLI entrypoint
  cmd/                Subcommand implementations (one file per command)
    mod.rs            Shared command execution helpers
    wait_for.rs       Wait for endpoints
    migrate.rs        Database migrations
    seed.rs           Database seeding
    render.rs         Template rendering
    fetch.rs          HTTP fetch
    exec.rs           Arbitrary command execution
  retry.rs            Retry logic with exponential backoff and jitter
  render.rs           Template rendering (envsubst + Jinja2 templates)
  logging.rs          Structured logging (text + JSON)
  safety.rs           Path validation and security guardrails
charts/initium/       Helm chart
examples/             Ready-to-use Kubernetes manifests
docs/                 Documentation
tests/                Integration tests
```

## Design Principles

1. **Single binary, zero runtime dependencies**: Built `FROM scratch` with statically-linked Rust binary (musl)
2. **Explicit over implicit**: All targets, paths, and behaviors must be explicitly configured
3. **Fail fast with actionable errors**: Errors include context about what went wrong and how to fix it
4. **Security by default**: Restrictive defaults that require opt-in for any relaxation
5. **Composable**: Each subcommand is independent; combine via multiple initContainers

## How to Add a New Subcommand

### 1. Create the command file

Create `src/cmd/your_command.rs`:

```rust
use crate::logging::Logger;

pub fn run(log: &Logger, /* flags */) -> Result<(), String> {
    log.info("starting your-command", &[]);
    // Implementation here
    Ok(())
}
```

### 2. Register the module

In `src/cmd/mod.rs`, add:

```rust
pub mod your_command;
```

### 3. Add the subcommand variant

In `src/main.rs`, add a variant to the `Commands` enum and dispatch it in the `match`:

```rust
#[derive(Subcommand)]
enum Commands {
    // ...existing variants...
    /// Your command description
    YourCommand {
        #[arg(long, help = "Description")]
        flag: String,
    },
}
```

### 4. Write tests

Add `#[cfg(test)]` tests in the relevant module, or create integration tests.

### 5. Add documentation

- Update `docs/usage.md` with flags, examples, and failure modes
- Add an example in `examples/`
- Update `CHANGELOG.md`

### 6. Verify

```bash
make test
make build
./target/release/initium your-command --help
```

## Retry System

The `internal/retry` package provides configurable retry logic used by `wait-for` and `fetch`:

- **Exponential backoff**: `delay = initial_delay * backoff_factor ^ attempt`
- **Jitter**: Random additive jitter as a fraction of the computed delay
- **Cap**: Delay is capped at `max_delay`
- **Context-aware**: Respects context cancellation and deadlines

## Logging

The `internal/logging` package provides:

- **Text mode**: `2025-01-15T10:30:00Z [INFO] message key=value`
- **JSON mode**: `{"time":"...","level":"INFO","msg":"message","key":"value"}`
- **Redaction**: Keys matching sensitive patterns are automatically redacted
- **Thread-safe**: Mutex-protected writes

## Safety

The `internal/safety` package enforces:

- **Path validation**: All file writes must target paths within `--workdir`
- **Absolute path rejection**: Target paths must be relative
- **Traversal detection**: `..` sequences that escape workdir are rejected

