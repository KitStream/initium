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

| Flag               | Default      | Env Var                  | Description                                  |
| ------------------ | ------------ | ------------------------ | -------------------------------------------- |
| `--target`         | _(required)_ |                          | Target URL (`tcp://`, `http://`, `https://`) |
| `--timeout`        | `5m`         | `INITIUM_TIMEOUT`        | Overall timeout                              |
| `--max-attempts`   | `60`         | `INITIUM_MAX_ATTEMPTS`   | Max retry attempts                           |
| `--initial-delay`  | `1s`         | `INITIUM_INITIAL_DELAY`  | Initial retry delay                          |
| `--max-delay`      | `30s`        | `INITIUM_MAX_DELAY`      | Max retry delay                              |
| `--backoff-factor` | `2.0`        | `INITIUM_BACKOFF_FACTOR` | Exponential backoff multiplier               |
| `--jitter`         | `0.1`        | `INITIUM_JITTER`         | Jitter fraction (0.0–1.0)                    |
| `--http-status`    | `200`        | `INITIUM_HTTP_STATUS`    | Expected HTTP status code                    |
| `--insecure-tls`   | `false`      | `INITIUM_INSECURE_TLS`   | Skip TLS verification                        |

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

| Flag          | Default   | Description                                                 |
| ------------- | --------- | ----------------------------------------------------------- |
| `--workdir`   | `/work`   | Working directory for file operations                       |
| `--lock-file` | _(none)_  | Skip migration if this file exists in workdir (idempotency) |
| `--json`      | `false`   | Enable JSON log output                                      |

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

| Code   | Meaning                                        |
| ------ | ---------------------------------------------- |
| `0`    | Migration succeeded (or skipped via lock file) |
| `1`    | Migration command failed, or invalid arguments |
| _N_    | Forwarded from the migration command           |

### seed

Apply structured database seeds from a YAML or JSON spec file.

The `seed` subcommand reads a declarative seed plan, connects to the target database,
and applies data with full idempotency via a tracking table and unique key detection.
Supports PostgreSQL, MySQL, and SQLite.

```bash
# Apply seeds from a YAML spec
initium seed --spec /seeds/seed.yaml

# Reset and re-apply (deletes existing seeded data first)
initium seed --spec /seeds/seed.yaml --reset

# With JSON logs
initium seed --spec /seeds/seed.yaml --json
```

**Flags:**

| Flag      | Default      | Description                             |
| --------- | ------------ | --------------------------------------- |
| `--spec`  | _(required)_ | Path to seed spec file (YAML or JSON)   |
| `--reset` | `false`      | Delete existing data and re-apply seeds |
| `--json`  | `false`      | Enable JSON log output                  |

**Behavior:**

- Seed spec files are MiniJinja templates rendered with env vars before parsing (`{{ env.VAR }}`)
- Reads a YAML/JSON seed spec defining phases, seed sets, tables, rows, and ordering
- Creates a tracking table (default: `initium_seed`) to record applied seed sets
- Skips already-applied seed sets unless `--reset` is used
- Supports unique key detection to prevent duplicate row insertion
- Supports auto-generated IDs and cross-table references via `_ref` / `@ref:`
- Supports environment variable substitution via `$env:VAR_NAME` or MiniJinja `{{ env.VAR }}`
- Each seed set is applied in a transaction; failures trigger rollback
- In reset mode, tables are deleted in reverse order to respect foreign keys
- Ordered phases with `create_if_missing` (database/schema creation), `wait_for` (poll for objects with timeout), and seed data
- Wait-for supports `table`, `view`, `schema`, `database` object types (driver-dependent)

**Exit codes:**

| Code   | Meaning                                             |
| ------ | --------------------------------------------------- |
| `0`    | Seed plan applied successfully                      |
| `1`    | Invalid spec, database error, or missing references |

See [seeding.md](seeding.md) for the full schema reference, features, and Kubernetes examples.

### render

Render a template file into a config file using environment variable substitution.

Two modes are supported:

- **envsubst** (default) — replaces `${VAR}` and `$VAR` patterns with environment variable values. Missing variables are left as-is.
- **gotemplate** — Jinja2-style templates via minijinja with environment variables accessible as `{{ env.VAR }}`. Missing variables produce empty strings.

Output files are written relative to `--workdir` with path traversal prevention. Intermediate directories are created automatically.

```bash
# envsubst mode (default)
initium render --template /templates/app.conf.tmpl --output app.conf

# Jinja2 template mode
initium render --mode gotemplate --template /templates/app.conf.tmpl --output app.conf

# Custom workdir
initium render --template /tpl/nginx.conf.tmpl --output nginx.conf --workdir /etc/nginx

# Nested output directory (created automatically)
initium render --template /tpl/db.conf.tmpl --output config/db.conf --workdir /work
```

**Flags:**

| Flag         | Default      | Description                               |
| ------------ | ------------ | ----------------------------------------- |
| `--template` | _(required)_ | Path to template file                     |
| `--output`   | _(required)_ | Output file path relative to workdir      |
| `--workdir`  | `/work`      | Working directory for output files        |
| `--mode`     | `envsubst`   | Template mode: `envsubst` or `gotemplate` |
| `--json`     | `false`      | Enable JSON log output                    |

**Exit codes:**

| Code   | Meaning                                                                       |
| ------ | ----------------------------------------------------------------------------- |
| `0`    | Render succeeded                                                              |
| `1`    | Invalid arguments, missing template, template syntax error, or path traversal |

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

| Flag                           | Default      | Description                                                |
| ------------------------------ | ------------ | ---------------------------------------------------------- |
| `--url`                        | _(required)_ | Target URL to fetch                                        |
| `--output`                     | _(required)_ | Output file path relative to workdir                       |
| `--workdir`                    | `/work`      | Working directory for output files                         |
| `--auth-env`                   | _(none)_     | Name of env var containing the Authorization header value  |
| `--insecure-tls`               | `false`      | Skip TLS certificate verification                          |
| `--follow-redirects`           | `false`      | Follow HTTP redirects                                      |
| `--allow-cross-site-redirects` | `false`      | Allow cross-site redirects (requires `--follow-redirects`) |
| `--timeout`                    | `5m`         | Overall timeout                                            |
| `--max-attempts`               | `3`          | Maximum retry attempts                                     |
| `--initial-delay`              | `1s`         | Initial delay between retries                              |
| `--max-delay`                  | `30s`        | Maximum delay between retries                              |
| `--backoff-factor`             | `2.0`        | Backoff multiplier                                         |
| `--jitter`                     | `0.1`        | Jitter fraction (0.0–1.0)                                  |
| `--json`                       | `false`      | Enable JSON log output                                     |

**Security notes:**

- The `--auth-env` flag takes the **name** of an environment variable, not the token itself, to avoid leaking credentials in process argument lists or shell history.
- Redirects are disabled by default. When enabled with `--follow-redirects`, cross-site redirects are blocked unless `--allow-cross-site-redirects` is also set.
- TLS verification is enabled by default; `--insecure-tls` must be explicitly set.

**Exit codes:**

| Code   | Meaning                                                   |
| ------ | --------------------------------------------------------- |
| `0`    | Fetch succeeded                                           |
| `1`    | Invalid arguments, HTTP error, timeout, or path traversal |

### exec

Run an arbitrary command with structured logging and exit code forwarding.

The command is executed directly via `execve` (no shell). Use `--` to separate
initium flags from the command and its arguments.

stdout and stderr are captured and logged with timestamps. The child process
exit code is forwarded. If `--workdir` is set, the child process working
directory is changed accordingly.

```bash
# Run a setup script
initium exec -- /bin/setup.sh

# Run with JSON logs
initium exec --json -- python3 /scripts/init.py

# Run in a specific directory
initium exec --workdir /app -- ./prepare.sh

# Generate a private key with openssl
initium exec --workdir /certs -- openssl genrsa -out key.pem 4096
```

**Flags:**

| Flag        | Default     | Description                             |
| ----------- | ----------- | --------------------------------------- |
| `--workdir` | _(inherit)_ | Working directory for the child process |
| `--json`    | `false`     | Enable JSON log output                  |

**Behavior:**

- stdout and stderr from the command are captured and logged with timestamps
- The child process exit code is forwarded: a non-zero exit code causes `exec` to fail
- No shell is used: the command is executed directly via `execve`
- The `--workdir` flag sets the child's working directory; it does not constrain file writes (unlike other subcommands)

**Exit codes:**

| Code   | Meaning                              |
| ------ | ------------------------------------ |
| `0`    | Command succeeded                    |
| `1`    | Command failed, or invalid arguments |
| _N_    | Forwarded from the command           |

## Building Custom Images with Initium

Initium ships as a minimal `scratch`-based image. For use cases that need
additional tools (e.g., `openssl`, `curl`, database clients), build a custom
image using Initium as a base:

```dockerfile
FROM ghcr.io/kitstream/initium:latest AS initium

FROM alpine:3.21
COPY --from=initium /initium /usr/local/bin/initium

# Install the tools you need
RUN apk add --no-cache openssl

USER 65534:65534
ENTRYPOINT ["/usr/local/bin/initium"]
```

Then use `exec` to run your tool:

```yaml
initContainers:
  - name: generate-key
    image: my-registry/initium-openssl:latest
    command: ["initium"]
    args: ["exec", "--workdir", "/certs", "--", "openssl", "genrsa", "-out", "key.pem", "4096"]
    volumeMounts:
      - name: certs
        mountPath: /certs
```

### initium-jyq: pre-built image with jq and yq

For JSON/YAML processing, a pre-built variant is available:

```bash
docker pull ghcr.io/kitstream/initium-jyq:latest
```

Use it in initContainers to transform config files:

```yaml
initContainers:
  - name: transform-config
    image: ghcr.io/kitstream/initium-jyq:latest
    command: ["initium"]
    args: ["exec", "--", "jq", ".database.host = \"db.prod\"", "/config/app.json"]
```

## Global Flags

| Flag     | Default   | Description                      |
| -------- | --------- | -------------------------------- |
| `--json` | `false`   | Enable JSON-formatted log output |

## Exit Codes

| Code   | Meaning                                                   |
| ------ | --------------------------------------------------------- |
| `0`    | Success                                                   |
| `1`    | General error (invalid args, timeout, unreachable target) |

## Security Defaults

All subcommands apply these security defaults:

- File writes constrained to `--workdir` (default `/work`)
- Path traversal prevention on all file operations
- No shell invocation by default (direct `execve`)
- Sensitive values redacted in log output
- Conservative network timeouts (5s per request)
- TLS verification enabled by default

See [security.md](security.md) for the full threat model.

