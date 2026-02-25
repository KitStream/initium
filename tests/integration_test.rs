//! Integration tests requiring docker-compose services.
//!
//! These tests are ignored by default and only run when the
//! `INTEGRATION` environment variable is set to `1`.
//!
//! To run:
//!   docker compose -f tests/docker-compose.yml up -d --wait
//!   INTEGRATION=1 cargo test --test integration_test -- --test-threads=1
//!   docker compose -f tests/docker-compose.yml down

use std::process::Command;

fn initium_bin() -> String {
    env!("CARGO_BIN_EXE_initium").to_string()
}

fn integration_enabled() -> bool {
    std::env::var("INTEGRATION").is_ok_and(|v| v == "1")
}

fn input_dir() -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    format!("{}/tests/input", manifest)
}

#[cfg(feature = "postgres")]
const PG_URL: &str = "postgres://initium:initium@localhost:15432/initium_test";
#[cfg(feature = "mysql")]
const MYSQL_URL_STR: &str = "mysql://initium:initium@localhost:13306/initium_test";
#[cfg(feature = "mysql")]
const MYSQL_ROOT_URL_STR: &str = "mysql://root:rootpass@localhost:13306/initium_test";

#[cfg(feature = "postgres")]
fn pg_client() -> postgres::Client {
    postgres::Client::connect(PG_URL, postgres::NoTls).expect("failed to connect to postgres")
}

#[cfg(feature = "mysql")]
fn mysql_conn() -> mysql::PooledConn {
    let pool = mysql::Pool::new(MYSQL_URL_STR).expect("failed to connect to mysql");
    pool.get_conn().expect("failed to get mysql connection")
}

#[cfg(feature = "mysql")]
fn mysql_root_conn() -> mysql::PooledConn {
    let pool = mysql::Pool::new(MYSQL_ROOT_URL_STR).expect("failed to connect to mysql as root");
    pool.get_conn()
        .expect("failed to get mysql root connection")
}

// ---------------------------------------------------------------------------
// wait-for: TCP against Postgres
// ---------------------------------------------------------------------------
#[test]
fn test_waitfor_tcp_postgres() {
    if !integration_enabled() {
        return;
    }
    let out = Command::new(initium_bin())
        .args([
            "wait-for",
            "--target",
            "tcp://localhost:15432",
            "--timeout",
            "30s",
            "--max-attempts",
            "30",
        ])
        .output()
        .expect("failed to run initium");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "wait-for tcp postgres should succeed: {}",
        stderr
    );
    assert!(
        stderr.contains("target is reachable"),
        "expected reachable log: {}",
        stderr
    );
}

// ---------------------------------------------------------------------------
// wait-for: HTTP against nginx health-check server
// ---------------------------------------------------------------------------
#[test]
fn test_waitfor_http_server() {
    if !integration_enabled() {
        return;
    }
    let out = Command::new(initium_bin())
        .args([
            "wait-for",
            "--target",
            "http://localhost:18080/",
            "--timeout",
            "30s",
            "--max-attempts",
            "30",
        ])
        .output()
        .expect("failed to run initium");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "wait-for http should succeed: {}",
        stderr
    );
    assert!(
        stderr.contains("target is reachable"),
        "expected reachable log: {}",
        stderr
    );
}

// ---------------------------------------------------------------------------
// wait-for: non-existent service times out with proper exit code
// ---------------------------------------------------------------------------
#[test]
fn test_waitfor_nonexistent_service_timeout() {
    if !integration_enabled() {
        return;
    }
    let out = Command::new(initium_bin())
        .args([
            "wait-for",
            "--target",
            "tcp://localhost:19999",
            "--timeout",
            "2s",
            "--max-attempts",
            "2",
            "--initial-delay",
            "500ms",
        ])
        .output()
        .expect("failed to run initium");
    assert!(!out.status.success(), "wait-for non-existent should fail");
    let code = out.status.code().unwrap_or(-1);
    assert_eq!(code, 1, "expected exit code 1, got {}", code);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("not reachable"),
        "expected 'not reachable' in error: {}",
        stderr
    );
}

// ---------------------------------------------------------------------------
// wait-for: TCP against MySQL
// ---------------------------------------------------------------------------
#[test]
fn test_waitfor_tcp_mysql() {
    if !integration_enabled() {
        return;
    }
    let out = Command::new(initium_bin())
        .args([
            "wait-for",
            "--target",
            "tcp://localhost:13306",
            "--timeout",
            "30s",
            "--max-attempts",
            "30",
        ])
        .output()
        .expect("failed to run initium");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "wait-for tcp mysql should succeed: {}",
        stderr
    );
}

// ---------------------------------------------------------------------------
// wait-for: multiple targets at once
// ---------------------------------------------------------------------------
#[test]
fn test_waitfor_multiple_targets() {
    if !integration_enabled() {
        return;
    }
    let out = Command::new(initium_bin())
        .args([
            "wait-for",
            "--target",
            "tcp://localhost:15432",
            "--target",
            "tcp://localhost:13306",
            "--target",
            "http://localhost:18080/",
            "--timeout",
            "30s",
            "--max-attempts",
            "30",
        ])
        .output()
        .expect("failed to run initium");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "wait-for multiple should succeed: {}",
        stderr
    );
    assert!(
        stderr.contains("all targets reachable"),
        "expected all targets reachable: {}",
        stderr
    );
}

// ---------------------------------------------------------------------------
// render: template with env vars produces correct output
// ---------------------------------------------------------------------------
#[test]
fn test_render_template() {
    if !integration_enabled() {
        return;
    }
    let workdir = tempfile::TempDir::new().expect("failed to create tempdir");
    let template = format!("{}/template.conf.tmpl", input_dir());

    let out = Command::new(initium_bin())
        .args([
            "render",
            "--template",
            &template,
            "--output",
            "app.conf",
            "--workdir",
            workdir.path().to_str().unwrap(),
        ])
        .env("DB_HOST", "postgres.prod")
        .env("DB_PORT", "5432")
        .env("DB_NAME", "myapp")
        .env("MAX_CONN", "100")
        .output()
        .expect("failed to run initium");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "render should succeed: {}", stderr);

    let rendered = std::fs::read_to_string(workdir.path().join("app.conf"))
        .expect("failed to read rendered output");
    assert!(
        rendered.contains("host = postgres.prod"),
        "expected host: {}",
        rendered
    );
    assert!(
        rendered.contains("port = 5432"),
        "expected port: {}",
        rendered
    );
    assert!(
        rendered.contains("database = myapp"),
        "expected database: {}",
        rendered
    );
    assert!(
        rendered.contains("max_connections = 100"),
        "expected max_conn: {}",
        rendered
    );
}

// ---------------------------------------------------------------------------
// fetch: from HTTP server writes response to file
// ---------------------------------------------------------------------------
#[test]
fn test_fetch_from_http_server() {
    if !integration_enabled() {
        return;
    }
    let workdir = tempfile::TempDir::new().expect("failed to create tempdir");

    let out = Command::new(initium_bin())
        .args([
            "fetch",
            "--url",
            "http://localhost:18080/",
            "--output",
            "index.html",
            "--workdir",
            workdir.path().to_str().unwrap(),
            "--timeout",
            "30s",
        ])
        .output()
        .expect("failed to run initium");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "fetch should succeed: {}", stderr);

    let fetched = std::fs::read_to_string(workdir.path().join("index.html"))
        .expect("failed to read fetched file");
    assert!(!fetched.is_empty(), "fetched file should not be empty");
    assert!(
        fetched.contains("nginx") || fetched.contains("Welcome") || fetched.contains("html"),
        "fetched content should contain html: {}",
        &fetched[..fetched.len().min(200)]
    );
}

// ---------------------------------------------------------------------------
// exec: runs command, captures output and exit code
// ---------------------------------------------------------------------------
#[test]
fn test_exec_command() {
    if !integration_enabled() {
        return;
    }
    let out = Command::new(initium_bin())
        .args(["exec", "--", "echo", "hello-from-initium"])
        .output()
        .expect("failed to run initium");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "exec echo should succeed: {}", stderr);
    assert!(
        stderr.contains("hello-from-initium"),
        "expected captured output in logs: {}",
        stderr
    );
}

#[test]
fn test_exec_failing_command() {
    if !integration_enabled() {
        return;
    }
    let out = Command::new(initium_bin())
        .args(["exec", "--", "false"])
        .output()
        .expect("failed to run initium");
    assert!(!out.status.success(), "exec false should fail");
    let code = out.status.code().unwrap_or(-1);
    assert_eq!(code, 1, "expected exit code 1, got {}", code);
}

// ---------------------------------------------------------------------------
// seed: PostgreSQL — create tables, seed, verify
// ---------------------------------------------------------------------------
#[cfg(feature = "postgres")]
#[test]
fn test_seed_postgres() {
    if !integration_enabled() {
        return;
    }

    let mut client = pg_client();
    client
        .batch_execute(
            "DROP TABLE IF EXISTS employees;
             DROP TABLE IF EXISTS departments;
             DROP TABLE IF EXISTS initium_seed;
             CREATE TABLE departments (id SERIAL PRIMARY KEY, name TEXT UNIQUE);
             CREATE TABLE employees (id SERIAL PRIMARY KEY, name TEXT, email TEXT UNIQUE, department_id INTEGER REFERENCES departments(id));",
        )
        .expect("failed to create postgres tables");

    let spec = format!("{}/seed-postgres.yaml", input_dir());
    let out = Command::new(initium_bin())
        .args(["seed", "--spec", &spec])
        .env("POSTGRES_URL", PG_URL)
        .output()
        .expect("failed to run seed");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "seed postgres should succeed: {}",
        stderr
    );
    assert!(
        stderr.contains("seed execution completed"),
        "expected completion log: {}",
        stderr
    );

    // Verify data
    let dept_count: i64 = client
        .query_one("SELECT COUNT(*) FROM departments", &[])
        .unwrap()
        .get(0);
    assert_eq!(dept_count, 2, "expected 2 departments");

    let emp_count: i64 = client
        .query_one("SELECT COUNT(*) FROM employees", &[])
        .unwrap()
        .get(0);
    assert_eq!(emp_count, 2, "expected 2 employees");

    // Verify cross-table references
    let rows = client
        .query(
            "SELECT e.name, d.name FROM employees e JOIN departments d ON e.department_id = d.id ORDER BY e.name",
            &[],
        )
        .unwrap();
    assert_eq!(rows.len(), 2);
    let alice_dept: &str = rows[0].get(1);
    let bob_dept: &str = rows[1].get(1);
    assert_eq!(alice_dept, "Engineering", "Alice should be in Engineering");
    assert_eq!(bob_dept, "Sales", "Bob should be in Sales");

    // Test idempotency
    let out = Command::new(initium_bin())
        .args(["seed", "--spec", &spec])
        .env("POSTGRES_URL", PG_URL)
        .output()
        .expect("failed to re-run seed");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "idempotent seed should succeed: {}",
        stderr
    );
    assert!(
        stderr.contains("already applied"),
        "expected skip log on re-run: {}",
        stderr
    );

    let dept_count: i64 = client
        .query_one("SELECT COUNT(*) FROM departments", &[])
        .unwrap()
        .get(0);
    assert_eq!(dept_count, 2, "idempotent re-run should not duplicate");

    // Test reset mode
    let out = Command::new(initium_bin())
        .args(["seed", "--spec", &spec, "--reset"])
        .env("POSTGRES_URL", PG_URL)
        .output()
        .expect("failed to run seed --reset");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "seed --reset should succeed: {}",
        stderr
    );
    assert!(
        stderr.contains("reset mode"),
        "expected reset log: {}",
        stderr
    );

    let dept_count: i64 = client
        .query_one("SELECT COUNT(*) FROM departments", &[])
        .unwrap()
        .get(0);
    assert_eq!(dept_count, 2, "reset should re-seed 2 departments");
}

// ---------------------------------------------------------------------------
// seed: MySQL — create tables, seed, verify
// ---------------------------------------------------------------------------
#[cfg(feature = "mysql")]
#[test]
fn test_seed_mysql() {
    if !integration_enabled() {
        return;
    }
    use mysql::prelude::Queryable;

    let mut conn = mysql_conn();
    conn.query_drop("DROP TABLE IF EXISTS orders").unwrap();
    conn.query_drop("DROP TABLE IF EXISTS products").unwrap();
    conn.query_drop("DROP TABLE IF EXISTS initium_seed")
        .unwrap();
    conn.query_drop(
        "CREATE TABLE products (id INT AUTO_INCREMENT PRIMARY KEY, sku VARCHAR(255) UNIQUE, name VARCHAR(255), price VARCHAR(50))",
    )
    .unwrap();
    conn.query_drop(
        "CREATE TABLE orders (id INT AUTO_INCREMENT PRIMARY KEY, product_id INT, quantity VARCHAR(50), FOREIGN KEY (product_id) REFERENCES products(id))",
    )
    .unwrap();

    let spec = format!("{}/seed-mysql.yaml", input_dir());
    let out = Command::new(initium_bin())
        .args(["seed", "--spec", &spec])
        .env("MYSQL_URL", MYSQL_URL_STR)
        .output()
        .expect("failed to run seed");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "seed mysql should succeed: {}",
        stderr
    );
    assert!(
        stderr.contains("seed execution completed"),
        "expected completion log: {}",
        stderr
    );

    // Verify data
    let prod_count: Option<i64> = conn
        .exec_first("SELECT COUNT(*) FROM products", ())
        .unwrap();
    assert_eq!(prod_count, Some(2), "expected 2 products");

    let order_count: Option<i64> = conn.exec_first("SELECT COUNT(*) FROM orders", ()).unwrap();
    assert_eq!(order_count, Some(2), "expected 2 orders");

    // Verify cross-table references
    let rows: Vec<(String, String)> = conn
        .exec(
            "SELECT p.name, o.quantity FROM orders o JOIN products p ON o.product_id = p.id ORDER BY p.name",
            (),
        )
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].0, "Gadget");
    assert_eq!(rows[0].1, "1");
    assert_eq!(rows[1].0, "Widget");
    assert_eq!(rows[1].1, "2");

    // Test idempotency
    let out = Command::new(initium_bin())
        .args(["seed", "--spec", &spec])
        .env("MYSQL_URL", MYSQL_URL_STR)
        .output()
        .expect("failed to re-run seed");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "idempotent seed should succeed: {}",
        stderr
    );
    assert!(
        stderr.contains("already applied"),
        "expected skip log on re-run: {}",
        stderr
    );

    let prod_count: Option<i64> = conn
        .exec_first("SELECT COUNT(*) FROM products", ())
        .unwrap();
    assert_eq!(
        prod_count,
        Some(2),
        "idempotent re-run should not duplicate"
    );
}

// ---------------------------------------------------------------------------
// seed: PostgreSQL — create database via seed phase
// ---------------------------------------------------------------------------
#[cfg(feature = "postgres")]
#[test]
fn test_seed_postgres_create_database() {
    if !integration_enabled() {
        return;
    }

    let mut client = pg_client();
    let _ = client.batch_execute("DROP DATABASE IF EXISTS initium_created_db");

    let spec = format!("{}/create-db-postgres.yaml", input_dir());
    let out = Command::new(initium_bin())
        .args(["seed", "--spec", &spec])
        .env("POSTGRES_URL", PG_URL)
        .output()
        .expect("failed to run seed");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "seed create database should succeed: {}",
        stderr
    );
    assert!(
        stderr.contains("creating database if missing"),
        "expected create database log: {}",
        stderr
    );

    let count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM pg_database WHERE datname = 'initium_created_db'",
            &[],
        )
        .unwrap()
        .get(0);
    assert_eq!(count, 1, "expected database to exist");

    // Idempotent re-run
    let out = Command::new(initium_bin())
        .args(["seed", "--spec", &spec])
        .env("POSTGRES_URL", PG_URL)
        .output()
        .expect("failed to re-run seed");
    assert!(
        out.status.success(),
        "idempotent create database should succeed"
    );

    let _ = client.batch_execute("DROP DATABASE IF EXISTS initium_created_db");
}

// ---------------------------------------------------------------------------
// seed: PostgreSQL — create schema via seed phase
// ---------------------------------------------------------------------------
#[cfg(feature = "postgres")]
#[test]
fn test_seed_postgres_create_schema() {
    if !integration_enabled() {
        return;
    }

    let mut client = pg_client();
    let _ = client.batch_execute("DROP SCHEMA IF EXISTS test_analytics CASCADE");

    let spec = format!("{}/create-schema-postgres.yaml", input_dir());
    let out = Command::new(initium_bin())
        .args(["seed", "--spec", &spec])
        .env("POSTGRES_URL", PG_URL)
        .output()
        .expect("failed to run seed");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "seed create schema should succeed: {}",
        stderr
    );
    assert!(
        stderr.contains("creating schema if missing"),
        "expected create schema log: {}",
        stderr
    );

    let count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM information_schema.schemata WHERE schema_name = 'test_analytics'",
            &[],
        )
        .unwrap()
        .get(0);
    assert_eq!(count, 1, "expected schema to exist");

    let _ = client.batch_execute("DROP SCHEMA IF EXISTS test_analytics CASCADE");
}

// ---------------------------------------------------------------------------
// seed: MySQL — create database via seed phase
// ---------------------------------------------------------------------------
#[cfg(feature = "mysql")]
#[test]
fn test_seed_mysql_create_database() {
    if !integration_enabled() {
        return;
    }
    use mysql::prelude::Queryable;

    let mut conn = mysql_root_conn();
    let _ = conn.query_drop("DROP DATABASE IF EXISTS initium_created_db");

    let spec = format!("{}/create-db-mysql.yaml", input_dir());
    let out = Command::new(initium_bin())
        .args(["seed", "--spec", &spec])
        .env("MYSQL_URL", MYSQL_ROOT_URL_STR)
        .output()
        .expect("failed to run seed");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "seed create database should succeed: {}",
        stderr
    );

    let count: Option<i64> = conn
        .exec_first(
            "SELECT COUNT(*) FROM information_schema.schemata WHERE SCHEMA_NAME = 'initium_created_db'",
            (),
        )
        .unwrap();
    assert_eq!(count, Some(1), "expected database to exist");

    // Idempotent re-run
    let out = Command::new(initium_bin())
        .args(["seed", "--spec", &spec])
        .env("MYSQL_URL", MYSQL_ROOT_URL_STR)
        .output()
        .expect("failed to re-run seed");
    assert!(
        out.status.success(),
        "idempotent create database should succeed"
    );

    let _ = conn.query_drop("DROP DATABASE IF EXISTS initium_created_db");
}

// ---------------------------------------------------------------------------
// seed: PostgreSQL — create non-existing database and seed data into it
// ---------------------------------------------------------------------------
#[cfg(feature = "postgres")]
#[test]
fn test_seed_postgres_create_nonexistent_db_alpha() {
    if !integration_enabled() {
        return;
    }

    let mut client = pg_client();
    let _ = client.batch_execute("DROP DATABASE IF EXISTS initium_noexist_alpha");

    // Verify the database does NOT exist before seeding
    let count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM pg_database WHERE datname = 'initium_noexist_alpha'",
            &[],
        )
        .unwrap()
        .get(0);
    assert_eq!(count, 0, "database should not exist before test");

    let spec = format!("{}/create-nonexistent-db-alpha-postgres.yaml", input_dir());
    let out = Command::new(initium_bin())
        .args(["seed", "--spec", &spec])
        .env("POSTGRES_URL", PG_URL)
        .output()
        .expect("failed to run seed");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "seed create nonexistent db alpha should succeed: {}",
        stderr
    );
    assert!(
        stderr.contains("creating database if missing"),
        "expected create database log: {}",
        stderr
    );

    // Verify the database was created
    let count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM pg_database WHERE datname = 'initium_noexist_alpha'",
            &[],
        )
        .unwrap()
        .get(0);
    assert_eq!(count, 1, "database should now exist");

    let _ = client.batch_execute("DROP DATABASE IF EXISTS initium_noexist_alpha");
}

// ---------------------------------------------------------------------------
// seed: PostgreSQL — create a second non-existing database with different name
// ---------------------------------------------------------------------------
#[cfg(feature = "postgres")]
#[test]
fn test_seed_postgres_create_nonexistent_db_beta() {
    if !integration_enabled() {
        return;
    }

    let mut client = pg_client();
    let _ = client.batch_execute("DROP DATABASE IF EXISTS initium_noexist_beta");

    // Verify the database does NOT exist before seeding
    let count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM pg_database WHERE datname = 'initium_noexist_beta'",
            &[],
        )
        .unwrap()
        .get(0);
    assert_eq!(count, 0, "database should not exist before test");

    let spec = format!("{}/create-nonexistent-db-beta-postgres.yaml", input_dir());
    let out = Command::new(initium_bin())
        .args(["seed", "--spec", &spec])
        .env("POSTGRES_URL", PG_URL)
        .output()
        .expect("failed to run seed");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "seed create nonexistent db beta should succeed: {}",
        stderr
    );
    assert!(
        stderr.contains("creating database if missing"),
        "expected create database log: {}",
        stderr
    );

    // Verify the database was created
    let count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM pg_database WHERE datname = 'initium_noexist_beta'",
            &[],
        )
        .unwrap()
        .get(0);
    assert_eq!(count, 1, "database should now exist");

    // Re-run to verify idempotency — should not fail
    let out = Command::new(initium_bin())
        .args(["seed", "--spec", &spec])
        .env("POSTGRES_URL", PG_URL)
        .output()
        .expect("failed to re-run seed");
    assert!(
        out.status.success(),
        "idempotent create nonexistent db beta should succeed"
    );

    let _ = client.batch_execute("DROP DATABASE IF EXISTS initium_noexist_beta");
}

// ---------------------------------------------------------------------------
// seed: MySQL — create non-existing database and verify
// ---------------------------------------------------------------------------
#[cfg(feature = "mysql")]
#[test]
fn test_seed_mysql_create_nonexistent_db_alpha() {
    if !integration_enabled() {
        return;
    }
    use mysql::prelude::Queryable;

    let mut conn = mysql_root_conn();
    let _ = conn.query_drop("DROP DATABASE IF EXISTS initium_noexist_alpha");

    // Verify the database does NOT exist before seeding
    let count: Option<i64> = conn
        .exec_first(
            "SELECT COUNT(*) FROM information_schema.schemata WHERE SCHEMA_NAME = 'initium_noexist_alpha'",
            (),
        )
        .unwrap();
    assert_eq!(count, Some(0), "database should not exist before test");

    let spec = format!("{}/create-nonexistent-db-alpha-mysql.yaml", input_dir());
    let out = Command::new(initium_bin())
        .args(["seed", "--spec", &spec])
        .env("MYSQL_URL", MYSQL_ROOT_URL_STR)
        .output()
        .expect("failed to run seed");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "seed create nonexistent db alpha should succeed: {}",
        stderr
    );

    // Verify the database was created
    let count: Option<i64> = conn
        .exec_first(
            "SELECT COUNT(*) FROM information_schema.schemata WHERE SCHEMA_NAME = 'initium_noexist_alpha'",
            (),
        )
        .unwrap();
    assert_eq!(count, Some(1), "database should now exist");

    let _ = conn.query_drop("DROP DATABASE IF EXISTS initium_noexist_alpha");
}

// ---------------------------------------------------------------------------
// seed: MySQL — create a second non-existing database with different name
// ---------------------------------------------------------------------------
#[cfg(feature = "mysql")]
#[test]
fn test_seed_mysql_create_nonexistent_db_beta() {
    if !integration_enabled() {
        return;
    }
    use mysql::prelude::Queryable;

    let mut conn = mysql_root_conn();
    let _ = conn.query_drop("DROP DATABASE IF EXISTS initium_noexist_beta");

    // Verify the database does NOT exist before seeding
    let count: Option<i64> = conn
        .exec_first(
            "SELECT COUNT(*) FROM information_schema.schemata WHERE SCHEMA_NAME = 'initium_noexist_beta'",
            (),
        )
        .unwrap();
    assert_eq!(count, Some(0), "database should not exist before test");

    let spec = format!("{}/create-nonexistent-db-beta-mysql.yaml", input_dir());
    let out = Command::new(initium_bin())
        .args(["seed", "--spec", &spec])
        .env("MYSQL_URL", MYSQL_ROOT_URL_STR)
        .output()
        .expect("failed to run seed");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "seed create nonexistent db beta should succeed: {}",
        stderr
    );

    // Verify the database was created
    let count: Option<i64> = conn
        .exec_first(
            "SELECT COUNT(*) FROM information_schema.schemata WHERE SCHEMA_NAME = 'initium_noexist_beta'",
            (),
        )
        .unwrap();
    assert_eq!(count, Some(1), "database should now exist");

    // Re-run to verify idempotency — should not fail
    let out = Command::new(initium_bin())
        .args(["seed", "--spec", &spec])
        .env("MYSQL_URL", MYSQL_ROOT_URL_STR)
        .output()
        .expect("failed to re-run seed");
    assert!(
        out.status.success(),
        "idempotent create nonexistent db beta should succeed"
    );

    let _ = conn.query_drop("DROP DATABASE IF EXISTS initium_noexist_beta");
}
