# Structured Database Seeding

Initium's `seed` subcommand applies structured, repeatable data provisioning to your database from YAML or JSON seed spec files. It replaces ad-hoc shell scripts with a declarative approach that supports idempotency, referential integrity, and a tracking table to prevent duplicate application.

Seed spec files are MiniJinja templates: they are rendered with environment variables before parsing, enabling conditional phases, loops, and dynamic configuration.

## Supported Databases

| Driver     | Connection URL format                          |
| ---------- | ---------------------------------------------- |
| `postgres` | `postgres://user:pass@host:5432/dbname`        |
| `mysql`    | `mysql://user:pass@host:3306/dbname`           |
| `sqlite`   | `/path/to/database.db` or `:memory:` for tests |

## Quick Start

```bash
# Apply seeds from a YAML spec file
initium seed --spec /seeds/seed.yaml

# Apply seeds with reset (delete + re-seed)
initium seed --spec /seeds/seed.yaml --reset

# With JSON log output
initium seed --spec /seeds/seed.yaml --json
```

## Seed Spec Schema

The seed spec file defines the complete seeding plan. Both YAML and JSON formats are supported (file extension determines parser). The spec file is a MiniJinja template rendered with environment variables before parsing.

```yaml
database:
  driver: postgres                 # Required. One of: postgres, mysql, sqlite
  url: "postgres://..."            # Direct database URL
  url_env: DATABASE_URL            # Or: name of env var containing the URL
  tracking_table: initium_seed     # Default: "initium_seed"

phases:
  - name: setup                    # Required. Phase name.
    order: 1                       # Optional. Execution order (default: 0).
    database: reporting            # Optional. Database to target/create.
    schema: analytics              # Optional. Schema to target/create.
    create_if_missing: true        # Optional. Create database/schema if missing.
    timeout: 30s                    # Optional. Default wait timeout (e.g. 30s, 1m; default: 30s).
    wait_for:                      # Optional. Objects to wait for before seeding.
      - type: table                # One of: table, view, schema, database.
        name: users
        timeout: 60s               # Optional. Per-object timeout override.
    seed_sets:                     # Optional. Seed sets to apply in this phase.
      - name: initial_data
        order: 1                   # Optional. Controls execution order across seed sets.
        tables:
          - table: config
            order: 1               # Optional. Controls execution order within a seed set.
            unique_key: [email]    # Optional. Columns used for duplicate detection.
            auto_id:               # Optional. Auto-generated ID configuration.
              column: id           # Column name for the auto-generated ID.
              id_type: integer     # ID type (default: integer).
            rows:
              - _ref: row_alias    # Optional. Internal reference name for this row.
                key: app_name
                value: "{{ env.APP_NAME }}"
```

### Field reference

| Field                                           | Type     | Required | Description                                                      |
| ----------------------------------------------- | -------- | -------- | ---------------------------------------------------------------- |
| `database.driver`                               | string   | Yes      | Database driver: `postgres`, `mysql`, or `sqlite`                |
| `database.url`                                  | string   | No       | Direct database connection URL                                   |
| `database.url_env`                              | string   | No       | Environment variable containing the database URL                 |
| `database.tracking_table`                       | string   | No       | Name of the seed tracking table (default: `initium_seed`)        |
| `phases[].name`                                 | string   | Yes      | Unique phase name                                                |
| `phases[].order`                                | integer  | No       | Execution order (lower first, default: 0)                        |
| `phases[].database`                             | string   | No       | Target database name (for create/switch)                         |
| `phases[].schema`                               | string   | No       | Target schema name (for create/switch)                           |
| `phases[].create_if_missing`                    | boolean  | No       | Create the database/schema if it does not exist (default: false) |
| `phases[].timeout`                              | string   | No       | Default wait timeout (e.g. `30s`, `1m`, `1m30s`; default: `30s`) |
| `phases[].wait_for[].type`                      | string   | Yes      | Object type: `table`, `view`, `schema`, or `database`            |
| `phases[].wait_for[].name`                      | string   | Yes      | Object name to wait for                                          |
| `phases[].wait_for[].timeout`                   | string   | No       | Per-object timeout override (e.g. `60s`, `2m`, `1m30s`)          |
| `phases[].seed_sets[].name`                     | string   | Yes      | Unique name for the seed set (used in tracking)                  |
| `phases[].seed_sets[].order`                    | integer  | No       | Execution order (lower values first, default: 0)                 |
| `phases[].seed_sets[].tables[].table`           | string   | Yes      | Target database table name                                       |
| `phases[].seed_sets[].tables[].order`           | integer  | No       | Execution order within the seed set (default: 0)                 |
| `phases[].seed_sets[].tables[].unique_key`      | string[] | No       | Columns for duplicate detection                                  |
| `phases[].seed_sets[].tables[].auto_id.column`  | string   | No       | Auto-generated ID column name                                    |
| `phases[].seed_sets[].tables[].auto_id.id_type` | string   | No       | ID type (default: `integer`)                                     |
| `phases[].seed_sets[].tables[].rows[]._ref`     | string   | No       | Internal reference name for cross-table references               |

### Wait-for object support by driver

| Object type | SQLite | PostgreSQL | MySQL |
| ----------- | ------ | ---------- | ----- |
| `table`     | ✅      | ✅          | ✅     |
| `view`      | ✅      | ✅          | ✅     |
| `schema`    | ❌      | ✅          | ✅*    |
| `database`  | ❌      | ✅          | ✅*    |

\* In MySQL, `schema` and `database` are synonymous.

### Create-if-missing support by driver

| Operation         | SQLite | PostgreSQL | MySQL |
| ----------------- | ------ | ---------- | ----- |
| `CREATE DATABASE` | ❌      | ✅          | ✅     |
| `CREATE SCHEMA`   | ❌      | ✅          | ✅*    |

\* In MySQL, `CREATE SCHEMA` maps to `CREATE DATABASE`.

SQLite does not support separate databases or schemas — each file is a database.

### Database URL resolution

The database URL is resolved in this order:

1. `database.url_env` — environment variable name containing the URL
2. `database.url` — direct URL in the spec file
3. `DATABASE_URL` — fallback environment variable

## Features

### MiniJinja Templating

All seed spec files are rendered as MiniJinja templates before parsing. Environment variables are available as `{{ env.VAR_NAME }}`. This enables:

- **Dynamic values**: `{{ env.APP_VERSION }}`
- **Conditional phases**: `{% if env.ENABLE_ANALYTICS %}...{% endif %}`
- **Generated rows**: `{% for i in range(10) %}...{% endfor %}`
- **Lenient mode**: missing env vars render as empty strings (no errors)

```yaml
database:
  driver: {{ env.DB_DRIVER }}
  url_env: DATABASE_URL
phases:
  - name: config
    seed_sets:
      - name: app_config
        tables:
          - table: config
            rows:
{% for key in ["app_name", "version", "env"] %}
              - key: {{ key }}
                value: "{{ env['APP_' ~ key | upper] }}"
{% endfor %}
```

Note: The `@ref:` syntax for cross-table references is processed at execution time (after template rendering), so it works seamlessly with MiniJinja templates.

### Idempotency via Tracking Table

Initium creates a tracking table (default: `initium_seed`) that records which seed sets have been applied. On subsequent runs, already-applied seed sets are skipped automatically.

```
┌──────────────────────────────────────┐
│           initium_seed               │
├──────────┬───────────────────────────┤
│ seed_set │ applied_at                │
├──────────┼───────────────────────────┤
│ users    │ 2025-01-15T10:30:00Z      │
│ config   │ 2025-01-15T10:30:01Z      │
└──────────┴───────────────────────────┘
```

### Duplicate Detection via Unique Keys

When `unique_key` is specified on a table, each row is checked against existing data before insertion. Rows matching the unique key are skipped, preventing duplicate inserts even within the same seed set.

```yaml
tables:
  - table: users
    unique_key: [email]
    rows:
      - name: Alice
        email: alice@example.com    # Skipped if email already exists
```

### Auto-Generated IDs and Cross-Table References

Use `auto_id` to let the database generate IDs, and `_ref` + `@ref:` to reference generated values in other tables:

```yaml
phases:
  - name: data
    seed_sets:
      - name: with_refs
        tables:
          - table: departments
            order: 1
            auto_id:
              column: id
            rows:
              - _ref: dept_eng            # Name this row for later reference
                name: Engineering

          - table: employees
            order: 2
            rows:
              - name: Alice
                email: alice@example.com
                department_id: "@ref:dept_eng.id"   # Resolves to the generated ID
```

### Environment Variable Substitution

Use `$env:VAR_NAME` or MiniJinja `{{ env.VAR_NAME }}` to inject values from environment variables at runtime. This is ideal for credentials loaded from Kubernetes secrets:

```yaml
rows:
  - username: "$env:ADMIN_USERNAME"
    password_hash: "{{ env.ADMIN_PASSWORD_HASH }}"
```

### Reset Mode

Use `--reset` to delete all data from seeded tables and remove tracking entries before re-applying. Tables are deleted in reverse order to respect foreign key constraints:

```bash
initium seed --spec /seeds/seed.yaml --reset
```

### Ordering

Both seed sets and tables within seed sets support explicit ordering via the `order` field. Lower values execute first (default: 0). This ensures parent tables are seeded before dependent tables.

### Transaction Safety

Each seed set is applied within a database transaction. If any row fails to insert, the entire seed set is rolled back, preventing partial data application.

## Kubernetes Usage

### Credentials via Environment Variables (from Secrets)

```yaml
apiVersion: v1
kind: Pod
spec:
  initContainers:
    - name: seed-data
      image: ghcr.io/kitstream/initium:latest
      args: ["seed", "--spec", "/seeds/seed.yaml"]
      env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: db-credentials
              key: url
        - name: ADMIN_USERNAME
          valueFrom:
            secretKeyRef:
              name: admin-credentials
              key: username
        - name: ADMIN_PASSWORD_HASH
          valueFrom:
            secretKeyRef:
              name: admin-credentials
              key: password-hash
      volumeMounts:
        - name: seed-specs
          mountPath: /seeds
          readOnly: true
      securityContext:
        runAsNonRoot: true
        runAsUser: 65534
        readOnlyRootFilesystem: true
        allowPrivilegeEscalation: false
        capabilities:
          drop: [ALL]
  volumes:
    - name: seed-specs
      configMap:
        name: seed-specs
```

## CLI Reference

| Flag      | Default    | Description                             |
| --------- | ---------- | --------------------------------------- |
| `--spec`  | (required) | Path to seed spec file (YAML or JSON)   |
| `--reset` | `false`    | Delete existing data and re-apply seeds |
| `--json`  | `false`    | Enable JSON log output                  |

## Failure Modes

| Scenario                           | Behavior                                               |
| ---------------------------------- | ------------------------------------------------------ |
| Invalid spec file                  | Fails with parse error before connecting to database   |
| Invalid MiniJinja template         | Fails with template syntax error before parsing YAML   |
| Database unreachable               | Fails with connection error                            |
| Unsupported driver                 | Fails with descriptive error listing supported drivers |
| Missing env var for URL            | Fails with error naming the missing variable           |
| Missing env var in `$env:`         | Fails with error naming the missing variable           |
| Unresolved `@ref:`                 | Fails with error naming the missing reference          |
| Row insertion failure              | Entire seed set rolled back via transaction            |
| Duplicate row (with unique_key)    | Row silently skipped                                   |
| Already-applied seed set           | Seed set silently skipped                              |
| Wait-for object timeout            | Fails with structured timeout error naming the object  |
| Unsupported object type for driver | Fails immediately with driver-specific error           |
| CREATE DATABASE on SQLite          | Fails with "not supported" error                       |
| CREATE SCHEMA on SQLite            | Fails with "not supported" error                       |

## Examples

See the [`examples/seed/`](../examples/seed/) directory:

- [`basic-seed.yaml`](../examples/seed/basic-seed.yaml) — PostgreSQL with departments and employees, cross-table references
- [`sqlite-seed.yaml`](../examples/seed/sqlite-seed.yaml) — SQLite configuration table seeding
- [`env-credentials-seed.yaml`](../examples/seed/env-credentials-seed.yaml) — MySQL with credentials from Kubernetes secrets
- [`phased-seed.yaml`](../examples/seed/phased-seed.yaml) — Multi-phase PostgreSQL seeding with wait-for, create-if-missing, and MiniJinja templating
