# Integration Tests

This directory contains integration tests for Initium.

## Running

Integration tests require external services (Postgres, HTTP servers, etc.) and are not run in standard `cargo test`.

```bash
# Run unit tests only (default)
make test

# Integration tests require docker-compose (future)
# docker-compose -f tests/docker-compose.yml up -d
# cargo test --test integration
```

