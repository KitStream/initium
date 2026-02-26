# Frequently Asked Questions

## Functionality

### What is Initium and when should I use it?

Initium is a single binary that replaces ad-hoc bash scripts in Kubernetes `initContainers`. Use it when your pod needs to do any of these before the main container starts:

- Wait for a database or API to become reachable
- Run database migrations or seed data
- Render config files from templates
- Fetch secrets or config from an HTTP endpoint
- Run a setup script with structured logging

### How do I wait for Postgres to be ready before my app starts?

Add an Initium initContainer that targets the Postgres TCP port:

```yaml
initContainers:
  - name: wait-for-postgres
    image: ghcr.io/kitstream/initium:latest
    args: ["wait-for", "--target", "tcp://postgres:5432", "--timeout", "120s"]
    securityContext:
      runAsNonRoot: true
      runAsUser: 65534
      readOnlyRootFilesystem: true
      allowPrivilegeEscalation: false
      capabilities:
        drop: [ALL]
```

Initium retries with exponential backoff (default: up to 60 attempts, 1s initial delay, 30s max delay) until the connection succeeds or the timeout expires.

### How do I wait for an HTTP health endpoint instead of a TCP port?

Use an `http://` or `https://` target instead of `tcp://`:

```yaml
args: ["wait-for", "--target", "http://config-service:8080/healthz"]
```

By default Initium expects HTTP 200. To accept a different status code:

```yaml
args: ["wait-for", "--target", "http://api:8080/ready", "--http-status", "204"]
```

### Can I wait for multiple services at once?

Yes. Pass multiple `--target` flags. They are checked sequentially — all must succeed:

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

If any target fails to become reachable within the timeout, the initContainer exits with a non-zero code and the pod will not start.

### How do I seed data into my database?

Use the `seed` subcommand. It wraps your existing seed tool and forwards its exit code:

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

Everything after `--` is the command Initium will execute. Initium does not interpret those arguments — it passes them directly via `execve`.

### How do I run database migrations?

Use the `migrate` subcommand. It works the same way as `seed` but is a separate subcommand so you can distinguish migration steps from seed steps in logs:

```yaml
initContainers:
  - name: migrate
    image: ghcr.io/kitstream/initium:latest
    args: ["migrate", "--", "flyway", "migrate"]
    env:
      - name: FLYWAY_URL
        value: "jdbc:postgresql://postgres:5432/mydb"
```

### How do I render a config file from a template before my app starts?

Use the `render` subcommand. Mount a ConfigMap containing your template and let Initium expand environment variables into the output:

```yaml
initContainers:
  - name: render-config
    image: ghcr.io/kitstream/initium:latest
    args:
      - render
      - --template
      - /templates/app.conf.tmpl
      - --output
      - app.conf
      - --workdir
      - /work
    env:
      - name: DB_HOST
        value: postgres
      - name: DB_PORT
        value: "5432"
    volumeMounts:
      - name: workdir
        mountPath: /work
      - name: templates
        mountPath: /templates
        readOnly: true
```

The rendered file lands in `/work/app.conf`, which your main container can read from the shared `workdir` volume.

### How do I get JSON-formatted logs?

Add `--json` before the subcommand:

```yaml
args: ["--json", "wait-for", "--target", "tcp://postgres:5432"]
```

Output looks like:

```json
{"time":"2026-01-15T10:30:00Z","level":"INFO","msg":"target is reachable","target":"tcp://postgres:5432","attempts":"1"}
```

This is useful when you're shipping logs to a centralized system like Loki, Datadog, or Elasticsearch.

### How do I tune retry behavior?

All retry parameters are flags on the `wait-for` subcommand:

| Flag               | Default | What it does                                                             |
| ------------------ | ------- | ------------------------------------------------------------------------ |
| `--max-attempts`   | `60`    | Total number of attempts before giving up                                |
| `--initial-delay`  | `1s`    | Delay after the first failure                                            |
| `--max-delay`      | `30s`   | Upper bound on delay between retries                                     |
| `--backoff-factor` | `2.0`   | Multiplier applied to the delay after each attempt                       |
| `--jitter`         | `0.1`   | Random fraction (0.0–1.0) added to each delay to prevent thundering herd |
| `--timeout`        | `5m`    | Hard deadline across all targets                                         |

Example — fast retries with low jitter:

```yaml
args:
  - wait-for
  - --target
  - tcp://postgres:5432
  - --max-attempts
  - "10"
  - --initial-delay
  - "200ms"
  - --max-delay
  - "2s"
  - --backoff-factor
  - "1.5"
  - --jitter
  - "0.05"
```

### What happens when a target never becomes reachable?

Initium exits with code `1` after exhausting all retry attempts or hitting the `--timeout` deadline. The last error is printed to stderr:

```
2026-01-15T10:32:00Z [ERROR] target not reachable target=tcp://postgres:5432 error=all 60 attempts failed, last error: tcp dial postgres:5432: dial tcp: connect: connection refused
```

Because the initContainer exits non-zero, Kubernetes will restart it according to the pod's `restartPolicy` (default: `Always` for Deployments).

### Can I use Initium outside of Kubernetes?

Yes. It is a standalone static binary. Common non-Kubernetes uses:

```bash
# Docker Compose — wait for a dependency before starting
docker run --rm --network mynet ghcr.io/kitstream/initium:latest \
  wait-for --target tcp://db:5432

# CI pipeline — gate a step on service readiness
./bin/initium wait-for --target http://localhost:8080/healthz --timeout 30s
```

### What is the `--` separator and why is it needed?

The `--` tells Initium where its own flags end and the wrapped command begins. Without it, Initium might try to interpret your tool's flags as its own:

```yaml
# Correct — Initium sees "migrate" subcommand, then passes "flyway migrate" to execve
args: ["migrate", "--", "flyway", "migrate"]

# Wrong — Initium tries to parse "flyway" as a flag to the migrate subcommand
args: ["migrate", "flyway", "migrate"]
```

This is the same convention used by `kubectl`, `docker`, and many other CLI tools.

### Does Initium run commands through a shell?

No. Commands passed after `--` are executed directly via the operating system's `execve` syscall. There is no `/bin/sh -c` wrapper. This means:

- Shell features like pipes (`|`), redirects (`>`), globbing (`*`), and variable expansion (`$VAR`) will **not** work
- This is intentional — it prevents shell injection attacks
- If you genuinely need shell features, wrap your script in a file and execute it: `args: ["exec", "--", "/bin/sh", "/scripts/setup.sh"]`

---

## Security

### Does Initium need root or any special privileges?

No. Initium is designed for the most restrictive Kubernetes security posture:

```yaml
securityContext:
  runAsNonRoot: true
  runAsUser: 65534        # nobody
  runAsGroup: 65534
  readOnlyRootFilesystem: true
  allowPrivilegeEscalation: false
  seccompProfile:
    type: RuntimeDefault
  capabilities:
    drop: [ALL]
```

This satisfies the Kubernetes `restricted` Pod Security Standard. No special PSPs, ClusterRoles, or RBAC bindings are required.

### Will my secrets show up in Initium's logs?

No. Initium automatically redacts values for log keys matching common secret patterns: `token`, `password`, `secret`, `auth`, `authorization`, `api_key`, `apikey`. These appear as `REDACTED` in both text and JSON log output.

However, Initium cannot redact secrets that appear as part of a URL or arbitrary string. Avoid embedding credentials directly in target URLs — use environment variables and your application's own config parsing instead.

### Can Initium write files outside the working directory?

No. All file-writing operations (`render`, `fetch`) are constrained to the path specified by `--workdir` (default: `/work`). Initium rejects:

- **Absolute target paths** like `/etc/passwd`
- **Path traversal** like `../../etc/passwd` or `sub/../../etc/shadow`

If a path escapes the workdir, Initium exits with an error and writes nothing.

### Can Initium make network requests I didn't ask for?

No. Initium has no default outbound connections. Every network target must be explicitly provided via `--target` or `--url` flags. There is no telemetry, no update checker, no default phone-home behavior.

### Why is TLS verification on by default?

To prevent man-in-the-middle attacks. If you connect to `https://vault:8200`, Initium verifies the server's TLS certificate against the system CA bundle.

If you need to connect to a service with a self-signed certificate (common in dev/staging), explicitly opt in:

```yaml
args: ["wait-for", "--target", "https://vault:8200/v1/sys/health", "--insecure-tls"]
```

The `--insecure-tls` flag is intentionally verbose — it should stand out in code review.

### Why does the image use `FROM scratch` instead of Alpine or Debian?

A `scratch` base image contains zero OS packages, zero libraries, zero shells. This means:

- **Zero CVEs** from base image packages — nothing to scan, nothing to patch
- **No shell** for attackers to use if the container is compromised
- **Tiny image** — the final image is ~2MB (just the Rust binary + CA certificates)

The trade-off is that you cannot `kubectl exec` into the container for debugging. This is acceptable for initContainers, which run once and exit.

### How do I verify that the Initium image hasn't been tampered with?

Release images include SBOM and provenance attestations. Verify with cosign:

```bash
cosign verify-attestation \
  --type https://slsa.dev/provenance/v0.2 \
  ghcr.io/kitstream/initium:0.1.0
```

### Is Initium safe to use in multi-tenant clusters?

Yes. Initium runs with least privilege, makes no cluster API calls, and cannot access other namespaces, nodes, or the Kubernetes API. It only makes outbound TCP/HTTP connections to targets you explicitly configure.

---

## Deployment

### How do I install Initium?

There are three ways:

**1. Reference the image directly in your pod spec** (simplest):

```yaml
initContainers:
  - name: wait-for-db
    image: ghcr.io/kitstream/initium:latest
    args: ["wait-for", "--target", "tcp://postgres:5432"]
```

**2. Use the Helm chart** (for templated deployments):

```bash
helm install my-app charts/initium \
  --set sampleDeployment.enabled=true \
  --set 'initContainers[0].name=wait-db' \
  --set 'initContainers[0].command[0]=wait-for' \
  --set 'initContainers[0].args[0]=--target' \
  --set 'initContainers[0].args[1]=tcp://postgres:5432'
```

**3. Build from source**:

```bash
git clone https://github.com/KitStream/initium.git
cd initium
make build
./bin/initium --help
```

### What image tag should I use?

- **`latest`** — tracks the most recent release. Convenient but not reproducible.
- **`0.1.0`** (specific version) — pinned and reproducible. **Recommended for production.**

Always pin a specific version in production workloads to avoid unexpected behavior from image updates.

### Does the Helm chart install any cluster resources?

No. The Helm chart installs nothing by default (`sampleDeployment.enabled: false`). It provides templates and values for injecting Initium initContainers into your own deployments. It does not create CRDs, webhooks, ClusterRoles, or any cluster-scoped resources.

### How do I share data between the initContainer and my main container?

Use an `emptyDir` volume mounted at `--workdir` (default `/work`):

```yaml
spec:
  initContainers:
    - name: render-config
      image: ghcr.io/kitstream/initium:latest
      args: ["render", "--template", "/templates/app.conf.tmpl", "--output", "app.conf", "--workdir", "/work"]
      volumeMounts:
        - name: workdir
          mountPath: /work
  containers:
    - name: app
      image: myapp:latest
      volumeMounts:
        - name: workdir
          mountPath: /work
          readOnly: true
  volumes:
    - name: workdir
      emptyDir: {}
```

The initContainer writes to `/work`, and the main container reads from it.

### How do I chain multiple init steps (wait → migrate → seed)?

Define multiple initContainers in order. Kubernetes runs them sequentially:

```yaml
initContainers:
  - name: wait-for-db
    image: ghcr.io/kitstream/initium:latest
    args: ["wait-for", "--target", "tcp://postgres:5432"]
    securityContext: &initium-security
      runAsNonRoot: true
      runAsUser: 65534
      readOnlyRootFilesystem: true
      allowPrivilegeEscalation: false
      capabilities:
        drop: [ALL]

  - name: migrate
    image: ghcr.io/kitstream/initium:latest
    args: ["migrate", "--", "/app/migrate", "up"]
    securityContext: *initium-security

  - name: seed
    image: ghcr.io/kitstream/initium:latest
    args: ["seed", "--", "/app/seed", "--file", "/seeds/data.sql"]
    securityContext: *initium-security
```

If any step fails, the subsequent steps do not run and the pod stays in `Init:Error`.

### What Kubernetes versions does Initium support?

Initium has no dependency on the Kubernetes API — it is just a binary that runs inside a container. It works on any Kubernetes version that supports `initContainers` (1.6+). The Helm chart uses standard `apps/v1` APIs and works on Kubernetes 1.16+.

### Can I use Initium with a private container registry?

Yes. Pull the public image and push it to your registry:

```bash
docker pull ghcr.io/kitstream/initium:0.1.0
docker tag ghcr.io/kitstream/initium:0.1.0 registry.internal/initium:0.1.0
docker push registry.internal/initium:0.1.0
```

Then reference your internal registry in the pod spec. If your registry requires authentication, configure an `imagePullSecret` as usual.

### How do I build Initium locally?

```bash
git clone https://github.com/KitStream/initium.git
cd initium
make build        # produces target/release/initium
make test         # runs all unit tests
make lint         # runs cargo clippy + cargo fmt --check
```

### How do I build a custom Docker image?

```bash
# Build for your local architecture
docker build -t initium:dev .

# Build multi-arch (requires Docker Buildx)
make docker-build VERSION=dev
```

### My initContainer is stuck in `Init:CrashLoopBackOff`. How do I debug?

Check the initContainer logs:

```bash
kubectl logs <pod-name> -c <initcontainer-name>
```

Common causes:

| Symptom                                   | Likely cause                                 | Fix                                                                          |
| ----------------------------------------- | -------------------------------------------- | ---------------------------------------------------------------------------- |
| `target not reachable` after all attempts | Target service isn't running or DNS is wrong | Check the service/endpoint exists and is in the same namespace (or use FQDN) |
| `unsupported target scheme`               | Missing `tcp://` or `http://` prefix         | Add the scheme: `tcp://postgres:5432` not just `postgres:5432`               |
| `path traversal detected`                 | Output path tries to escape workdir          | Use a relative path for `--output`                                           |
| `context cancelled`                       | Overall `--timeout` was too short            | Increase `--timeout` or check why the target takes so long                   |

### Does Initium support ARM-based nodes (e.g., AWS Graviton)?

Yes. The container image is built for both `linux/amd64` and `linux/arm64`. Kubernetes pulls the correct architecture automatically via the multi-arch manifest.

