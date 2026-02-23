# Initium

**Swiss-army toolbox for Kubernetes initContainers.**

Initium replaces fragile bash scripts in your initContainers with a single, security-hardened, multi-tool binary. Wait for dependencies, run migrations, render config files, fetch secrets, and more — all with structured logging, retries, and safe defaults.

[![CI](https://github.com/kitstream/initium/actions/workflows/ci.yml/badge.svg)](https://github.com/kitstream/initium/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)

## Quickstart

### Wait for Postgres before starting your app

```yaml
initContainers:
  - name: wait-for-postgres
    image: ghcr.io/kitstream/initium:latest
    args:
      - wait-for
      - --target
      - tcp://postgres:5432
      - --timeout
      - "120s"
    securityContext:
      runAsNonRoot: true
      runAsUser: 65534
      readOnlyRootFilesystem: true
      allowPrivilegeEscalation: false
      capabilities:
        drop: [ALL]
```

### Apply a full example

```bash
kubectl apply -f https://raw.githubusercontent.com/kitstream/initium/main/examples/nginx-waitfor/deployment.yaml
```

## Why Initium?

| | Bash scripts | Initium |
|---|---|---|
| **Retries with backoff** | DIY, error-prone | Built-in, configurable |
| **Structured logging** | `echo` statements | JSON or text with timestamps |
| **Security** | Runs as root, full shell | Non-root, no shell, read-only FS |
| **Secret handling** | Easily leaked in logs | Automatic redaction |
| **Multiple tools** | Install curl, netcat, psql… | Single 10MB image |
| **Reproducibility** | Shell differences across distros | Single Go binary, `FROM scratch` |
| **Vulnerability surface** | Full OS + shell utils | Zero OS packages |

## Subcommands

| Command | Description | Status |
|---------|-------------|--------|
| `wait-for` | Wait for TCP/HTTP/HTTPS endpoints | ✅ Available |
| `migrate` | Run database migrations | 🔜 Coming soon |
| `seed` | Run database seed commands | 🔜 Coming soon |
| `render` | Render config templates | 🔜 Coming soon |
| `fetch` | Fetch secrets/config from HTTP | 🔜 Coming soon |
| `exec` | Run commands with structured logging | 🔜 Coming soon |

### wait-for

```bash
# Wait for a TCP endpoint
initium wait-for --target tcp://postgres:5432

# Wait for an HTTP health check
initium wait-for --target http://api:8080/healthz

# Wait for multiple endpoints
initium wait-for \
  --target tcp://postgres:5432 \
  --target tcp://redis:6379 \
  --target http://config:8080/healthz

# HTTPS with self-signed certificates
initium wait-for --target https://vault:8200/v1/sys/health --insecure-tls
```

## Helm Chart

The Helm chart makes it easy to inject Initium initContainers into your deployments.

```bash
helm install my-app charts/initium \
  --set sampleDeployment.enabled=true \
  --set 'initContainers[0].name=wait-for-db' \
  --set 'initContainers[0].command[0]=wait-for' \
  --set 'initContainers[0].args[0]=--target' \
  --set 'initContainers[0].args[1]=tcp://postgres:5432'
```

See [`charts/initium/values.yaml`](charts/initium/values.yaml) for all options.

## Security

Initium is designed to run in security-restricted environments:

- **Non-root**: Runs as UID 65534 (nobody)
- **Read-only filesystem**: Compatible with `readOnlyRootFilesystem: true`
- **No capabilities**: Drops all Linux capabilities
- **No shell**: Commands executed via `execve`, not through a shell
- **Secret redaction**: Sensitive values automatically redacted in logs
- **Minimal image**: Built `FROM scratch` — zero OS packages, zero CVEs
- **PSA `restricted`**: Fully compatible with the Kubernetes restricted Pod Security Standard

See [docs/security.md](docs/security.md) for the full threat model and [SECURITY.md](SECURITY.md) for vulnerability reporting.

## FAQ

### How do I wait for Postgres?

```yaml
initContainers:
  - name: wait-for-postgres
    image: ghcr.io/kitstream/initium:latest
    args: ["wait-for", "--target", "tcp://postgres:5432", "--timeout", "120s"]
```

Initium will retry connecting to `postgres:5432` with exponential backoff until it succeeds or the timeout is reached.

### How do I wait for multiple services?

Pass multiple `--target` flags. They are checked sequentially:

```yaml
args:
  - wait-for
  - --target
  - tcp://postgres:5432
  - --target
  - tcp://redis:6379
  - --target
  - http://config-service:8080/healthz
```

### How do I seed data?

Use the `seed` subcommand to run your seeding tool:

```yaml
initContainers:
  - name: seed-data
    image: ghcr.io/kitstream/initium:latest
    args: ["seed", "--", "/app/seed", "--file", "/seeds/initial.sql"]
    env:
      - name: DATABASE_URL
        valueFrom:
          secretKeyRef:
            name: db-credentials
            key: url
```

### How do I run database migrations?

Use the `migrate` subcommand — it wraps your migration tool with structured logging:

```yaml
initContainers:
  - name: migrate
    image: ghcr.io/kitstream/initium:latest
    args: ["migrate", "--", "flyway", "migrate"]
    env:
      - name: FLYWAY_URL
        value: "jdbc:postgresql://postgres:5432/mydb"
```

### How do I render config templates?

Use the `render` subcommand with environment variable substitution:

```yaml
initContainers:
  - name: render-config
    image: ghcr.io/kitstream/initium:latest
    args: ["render", "--template", "/templates/app.conf.tmpl", "--output", "app.conf", "--workdir", "/work"]
    env:
      - name: DB_HOST
        value: postgres
```

### How do I get JSON logs?

Add the `--json` global flag:

```yaml
args: ["--json", "wait-for", "--target", "tcp://postgres:5432"]
```

Output: `{"time":"2025-01-15T10:30:00Z","level":"INFO","msg":"target is reachable","target":"tcp://postgres:5432","attempts":"1"}`

### How do I allow self-signed TLS certificates?

Use `--insecure-tls` (must be explicitly opted in):

```yaml
args: ["wait-for", "--target", "https://vault:8200/v1/sys/health", "--insecure-tls"]
```

### Can I use Initium outside Kubernetes?

Yes. Initium is a standalone binary. Use it in Docker Compose, CI pipelines, or anywhere you need to wait for services:

```bash
docker run --rm ghcr.io/kitstream/initium:latest wait-for --target tcp://db:5432
```

### Does Initium need special permissions?

No. Initium runs as a non-root user with no capabilities and a read-only filesystem. It is compatible with the Kubernetes `restricted` Pod Security Standard.

### How do I customize retry behavior?

All retry parameters are configurable:

```yaml
args:
  - wait-for
  - --target
  - tcp://postgres:5432
  - --max-attempts
  - "30"
  - --initial-delay
  - "500ms"
  - --max-delay
  - "10s"
  - --backoff-factor
  - "1.5"
  - --jitter
  - "0.2"
```

## Examples

- [**nginx-waitfor**](examples/nginx-waitfor/): Nginx deployment waiting for a backend service
- [**postgres-migrate-seed**](examples/postgres-migrate-seed/): Wait → Migrate → Seed workflow
- [**config-render**](examples/config-render/): Render config from templates before app starts

## How to Run Locally

```bash
# Build
make build

# Run wait-for against a local service
./bin/initium wait-for --target tcp://localhost:5432 --max-attempts 5

# Run with JSON logs
./bin/initium --json wait-for --target http://localhost:8080/healthz

# Run all tests
make test
```

## How to Try in a Cluster

```bash
# Option 1: Use the pre-built image
kubectl apply -f examples/nginx-waitfor/deployment.yaml

# Option 2: Build and push your own image
make docker-build VERSION=dev
make docker-push VERSION=dev

# Option 3: Use the Helm chart
helm install my-app charts/initium \
  --set sampleDeployment.enabled=true \
  --set 'initContainers[0].name=wait-db' \
  --set 'initContainers[0].command[0]=wait-for' \
  --set 'initContainers[0].args[0]=--target' \
  --set 'initContainers[0].args[1]=tcp://postgres:5432'
```

## Documentation

- [Usage Guide](docs/usage.md) — All subcommands, flags, and examples
- [Security](docs/security.md) — Threat model, safe defaults, PSA compatibility
- [Architecture & Design](docs/design.md) — How Initium works and how to extend it

## Contributing

Contributions are welcome! Please see the [design doc](docs/design.md) for how to add new subcommands.

## License

[Apache License 2.0](LICENSE)

