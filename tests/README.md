# Tests

## Unit Tests

Run unit tests with:

```bash
cargo test
```

## Integration Tests

Integration tests require Docker Compose services (Postgres 16, MySQL 8.0, nginx).

```bash
# Start services
docker compose -f tests/docker-compose.yml up -d --wait

# Run integration tests
INTEGRATION=1 cargo test --test integration_test -- --test-threads=1

# Stop services
docker compose -f tests/docker-compose.yml down
```

### Test scenarios

| Test                                  | Description                                              |
| ------------------------------------- | -------------------------------------------------------- |
| `test_waitfor_tcp_postgres`           | wait-for TCP against Postgres succeeds                   |
| `test_waitfor_tcp_mysql`              | wait-for TCP against MySQL succeeds                      |
| `test_waitfor_http_server`            | wait-for HTTP against nginx returns 200                  |
| `test_waitfor_nonexistent_service_timeout` | wait-for against closed port fails with exit code 1 |
| `test_waitfor_multiple_targets`       | wait-for with Postgres + MySQL + HTTP all reachable      |
| `test_render_template`                | render envsubst template produces correct output         |
| `test_fetch_from_http_server`         | fetch from nginx writes HTML to file                     |
| `test_exec_command`                   | exec echo captures output in logs                        |
| `test_exec_failing_command`           | exec false returns exit code 1                           |
| `test_seed_postgres`                  | seed PostgreSQL with refs, idempotency, and reset        |
| `test_seed_mysql`                     | seed MySQL with refs and idempotency                     |
| `test_seed_postgres_create_database`  | seed creates a PostgreSQL database via create_if_missing |
| `test_seed_postgres_create_schema`    | seed creates a PostgreSQL schema via create_if_missing   |
| `test_seed_mysql_create_database`     | seed creates a MySQL database via create_if_missing      |
| `test_seed_postgres_create_nonexistent_db_alpha` | create-if-missing with known non-existing PG database    |
| `test_seed_postgres_create_nonexistent_db_beta`  | create-if-missing with second non-existing PG database + idempotency |
| `test_seed_mysql_create_nonexistent_db_alpha`    | create-if-missing with known non-existing MySQL database |
| `test_seed_mysql_create_nonexistent_db_beta`     | create-if-missing with second non-existing MySQL database + idempotency |

### CI

Integration tests run automatically via `.github/workflows/integration.yml` using GitHub Actions service containers.
