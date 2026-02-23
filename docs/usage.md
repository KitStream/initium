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

### migrate _(coming soon)_

Run a database migration command with structured logging.

```bash
initium migrate -- flyway migrate
initium migrate --json -- /app/migrate -path /migrations up
```

### seed _(coming soon)_

Run a database seed command.

```bash
initium seed -- /app/seed --file /seeds/data.sql
```

### render _(coming soon)_

Render templates into config files.

```bash
initium render --template /templates/app.conf.tmpl --output config/app.conf --workdir /work
```

### fetch _(coming soon)_

Fetch secrets or config from HTTP endpoints.

```bash
initium fetch --url https://vault:8200/v1/secret/data/app --output secrets.json --workdir /work
```

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

