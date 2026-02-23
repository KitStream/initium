# Usage Guide

## Installation

### Container Image

Pull the image directly:

```bash
docker pull ghcr.io/kitstream/initium:latest
```

### Helm Chart

```bash
helm install my-release charts/initium \
  --set sampleDeployment.enabled=true \
  --set 'initContainers[0].name=wait-for-db' \
  --set 'initContainers[0].command[0]=wait-for' \
  --set 'initContainers[0].args[0]=--target' \
  --set 'initContainers[0].args[1]=tcp://postgres:5432'
```

### Binary

Build from source:

```bash
make build
./bin/initium --help
```

## Subcommands

### wait-for

Wait for TCP or HTTP(S) endpoints to become reachable.

```bash
initium wait-for --target tcp://postgres:5432
initium wait-for --target http://api:8080/healthz --http-status 200
initium wait-for --target https://vault:8200/v1/sys/health --insecure-tls
```

**Flags:**

| Flag | Default | Env Var | Description |
|------|---------|---------|-------------|
| `--target` | _(required)_ | | Target URL (`tcp://`, `http://`, `https://`) |
| `--timeout` | `5m` | `INITIUM_TIMEOUT` | Overall timeout |
| `--max-attempts` | `60` | `INITIUM_MAX_ATTEMPTS` | Max retry attempts |
| `--initial-delay` | `1s` | `INITIUM_INITIAL_DELAY` | Initial retry delay |
| `--max-delay` | `30s` | `INITIUM_MAX_DELAY` | Max retry delay |
| `--backoff-factor` | `2.0` | `INITIUM_BACKOFF_FACTOR` | Exponential backoff multiplier |
| `--jitter` | `0.1` | `INITIUM_JITTER` | Jitter fraction (0.0–1.0) |
| `--http-status` | `200` | `INITIUM_HTTP_STATUS` | Expected HTTP status code |
| `--insecure-tls` | `false` | `INITIUM_INSECURE_TLS` | Skip TLS verification |

**Multiple targets:**

```bash
initium wait-for \
  --target tcp://postgres:5432 \
  --target tcp://redis:6379 \
  --target http://config-service:8080/healthz
```

Targets are checked sequentially. All must become reachable before the command succeeds.

### migrate

Run a database migration command with structured logging, exit code forwarding,
and optional idempotency via a lock file.

The command is executed directly via `execve` (no shell). Use `--` to separate
initium flags from the migration command and its arguments.

```bash
# Run a flyway migration
initium migrate -- flyway migrate

# Run with JSON logs
initium migrate --json -- /app/migrate -path /migrations up

# Idempotent: skip if already migrated
initium migrate --lock-file .migrated --workdir /work -- /app/migrate up
```

**Flags:**

| Flag | Default | Description |
|------|---------|-------------|
| `--workdir` | `/work` | Working directory for file operations |
| `--lock-file` | _(none)_ | Skip migration if this file exists in workdir (idempotency) |
| `--json` | `false` | Enable JSON log output |

**Behavior:**

- stdout and stderr from the migration command are captured and logged with timestamps
- The child process exit code is forwarded: a non-zero exit code causes `migrate` to fail
- When `--lock-file` is set:
  - If the lock file exists in `--workdir`, the migration is skipped (exit 0)
  - On successful completion, the lock file is created so subsequent runs become no-ops
  - If the migration fails, no lock file is created
- Lock file paths are validated against `--workdir` to prevent path traversal
- No shell is used: the command is executed directly via `execve`

**Exit codes:**

| Code | Meaning |
|------|---------|
| `0` | Migration succeeded (or skipped via lock file) |
| `1` | Migration command failed, or invalid arguments |
| _N_ | Forwarded from the migration command |

### seed

Run a database seed command with structured logging and exit code forwarding.

The command is executed directly via `execve` (no shell). Use `--` to separate
initium flags from the seed command and its arguments.

Unlike `migrate`, `seed` has no idempotency hints — it is the caller's
responsibility to ensure seed operations are safe to repeat or are only run once.

```bash
# Seed from a SQL file
initium seed -- psql -f /seeds/data.sql

# Seed with a custom script
initium seed -- /app/seed --file /seeds/data.sql

# Seed with JSON logs
initium seed --json -- python3 /scripts/seed.py
```

**Flags:**

| Flag | Default | Description |
|------|---------|-------------|
| `--json` | `false` | Enable JSON log output |

**Behavior:**

- stdout and stderr from the seed command are captured and logged with timestamps
- The child process exit code is forwarded: a non-zero exit code causes `seed` to fail
- No shell is used: the command is executed directly via `execve`

**Exit codes:**

| Code | Meaning |
|------|---------|
| `0` | Seed succeeded |
| `1` | Seed command failed, or invalid arguments |
| _N_ | Forwarded from the seed command |

### render

Render a template file into a config file using environment variable substitution.

Two modes are supported:

- **envsubst** (default) — replaces `${VAR}` and `$VAR` patterns with environment variable values. Missing variables are left as-is.
- **gotemplate** — full Go `text/template` support with environment variables as `.VarName`. Missing variables produce empty strings.

Output files are written relative to `--workdir` with path traversal prevention. Intermediate directories are created automatically.

```bash
# envsubst mode (default)
initium render --template /templates/app.conf.tmpl --output app.conf

# Go template mode
initium render --mode gotemplate --template /templates/app.conf.tmpl --output app.conf

# Custom workdir
initium render --template /tpl/nginx.conf.tmpl --output nginx.conf --workdir /etc/nginx

# Nested output directory (created automatically)
initium render --template /tpl/db.conf.tmpl --output config/db.conf --workdir /work
```

**Flags:**

| Flag | Default | Description |
|------|---------|-------------|
| `--template` | _(required)_ | Path to template file |
| `--output` | _(required)_ | Output file path relative to workdir |
| `--workdir` | `/work` | Working directory for output files |
| `--mode` | `envsubst` | Template mode: `envsubst` or `gotemplate` |
| `--json` | `false` | Enable JSON log output |

**Exit codes:**

| Code | Meaning |
|------|---------|
| `0` | Render succeeded |
| `1` | Invalid arguments, missing template, template syntax error, or path traversal |

### fetch

Fetch a resource from an HTTP(S) endpoint and write the response body to a file.

Supports optional authentication via an environment variable (to avoid leaking
credentials in process argument lists), TLS verification skipping, redirect
control, and retries with exponential backoff.

```bash
# Fetch a config file
initium fetch --url http://config-service:8080/app.json --output app.json

# Fetch from Vault with auth token
initium fetch --url https://vault:8200/v1/secret/data/app --output secrets.json \
  --auth-env VAULT_TOKEN --insecure-tls

# Fetch with retries
initium fetch --url http://api:8080/config --output config.json \
  --max-attempts 10 --initial-delay 2s

# Follow redirects (same-site only by default)
initium fetch --url http://cdn/config --output config.json --follow-redirects

# Allow cross-site redirects
initium fetch --url http://cdn/config --output config.json \
  --follow-redirects --allow-cross-site-redirects
```

**Flags:**

| Flag | Default | Description |
|------|---------|-------------|
| `--url` | _(required)_ | Target URL to fetch |
| `--output` | _(required)_ | Output file path relative to workdir |
| `--workdir` | `/work` | Working directory for output files |
| `--auth-env` | _(none)_ | Name of env var containing the Authorization header value |
| `--insecure-tls` | `false` | Skip TLS certificate verification |
| `--follow-redirects` | `false` | Follow HTTP redirects |
| `--allow-cross-site-redirects` | `false` | Allow cross-site redirects (requires `--follow-redirects`) |
| `--timeout` | `5m` | Overall timeout |
| `--max-attempts` | `3` | Maximum retry attempts |
| `--initial-delay` | `1s` | Initial delay between retries |
| `--max-delay` | `30s` | Maximum delay between retries |
| `--backoff-factor` | `2.0` | Backoff multiplier |
| `--jitter` | `0.1` | Jitter fraction (0.0–1.0) |
| `--json` | `false` | Enable JSON log output |

**Security notes:**

- The `--auth-env` flag takes the **name** of an environment variable, not the token itself, to avoid leaking credentials in process argument lists or shell history.
- Redirects are disabled by default. When enabled with `--follow-redirects`, cross-site redirects are blocked unless `--allow-cross-site-redirects` is also set.
- TLS verification is enabled by default; `--insecure-tls` must be explicitly set.

**Exit codes:**

| Code | Meaning |
|------|---------|
| `0` | Fetch succeeded |
| `1` | Invalid arguments, HTTP error, timeout, or path traversal |

### exec _(coming soon)_

Run arbitrary commands with structured logging.

```bash
initium exec -- /bin/setup.sh
initium exec --json -- python3 /scripts/init.py
```

## Global Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--json` | `false` | Enable JSON-formatted log output |

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | General error (invalid args, timeout, unreachable target) |

## Security Defaults

All subcommands apply these security defaults:

- File writes constrained to `--workdir` (default `/work`)
- Path traversal prevention on all file operations
- No shell invocation by default (direct `execve`)
- Sensitive values redacted in log output
- Conservative network timeouts (5s per request)
- TLS verification enabled by default

See [security.md](security.md) for the full threat model.

