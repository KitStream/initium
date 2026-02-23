# Architecture & Design

## Overview

Initium is a single Go binary with multiple subcommands, each addressing a common initContainer use case. It follows the Unix philosophy: each subcommand does one thing well, with composability via sequential initContainers in a pod spec.

```
┌─────────────────────────────────────────┐
│              cmd/initium/main.go        │
│          (CLI root, cobra commands)     │
├─────────────────────────────────────────┤
│            internal/cmd/                │
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
cmd/initium/          CLI entrypoint
internal/
  cmd/                Subcommand implementations (one file per command)
  retry/              Retry logic with exponential backoff and jitter
  render/             Template rendering (envsubst + Go templates)
  fetch/              HTTP fetch with auth support
  logging/            Structured logging (text + JSON)
  safety/             Path validation and security guardrails
charts/initium/       Helm chart
examples/             Ready-to-use Kubernetes manifests
docs/                 Documentation
tests/                Integration tests
```

## Design Principles

1. **Single binary, zero runtime dependencies**: Built `FROM scratch` with statically-linked Go binary
2. **Explicit over implicit**: All targets, paths, and behaviors must be explicitly configured
3. **Fail fast with actionable errors**: Errors include context about what went wrong and how to fix it
4. **Security by default**: Restrictive defaults that require opt-in for any relaxation
5. **Composable**: Each subcommand is independent; combine via multiple initContainers

## How to Add a New Subcommand

### 1. Create the command file

Create `internal/cmd/yourcommand.go`:

```go
package cmd

import (
    "github.com/kitstream/initium/internal/logging"
    "github.com/spf13/cobra"
)

func NewYourCommandCmd(log *logging.Logger) *cobra.Command {
    cmd := &cobra.Command{
        Use:   "your-command",
        Short: "One-line description",
        Long:  `Detailed description with usage context.`,
        Example: `  initium your-command --flag value`,
        RunE: func(cmd *cobra.Command, args []string) error {
            // Implementation here
            return nil
        },
    }

    // Add flags
    cmd.Flags().StringVar(&someVar, "flag", "default", "Description")

    return cmd
}
```

### 2. Register the command

In `cmd/initium/main.go`, add:

```go
root.AddCommand(cmd.NewYourCommandCmd(log))
```

### 3. Write tests

Create `internal/cmd/yourcommand_test.go` with:
- Unit tests for core logic
- Tests for invalid inputs and edge cases
- Tests for the cobra command execution

### 4. Add documentation

- Update `docs/usage.md` with flags, examples, and failure modes
- Add an example in `examples/`
- Update `CHANGELOG.md`

### 5. Verify

```bash
make test
make build
./bin/initium your-command --help
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

