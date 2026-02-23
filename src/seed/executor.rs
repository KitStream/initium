use crate::logging::Logger;
use crate::seed::db::Database;
use crate::seed::schema::{SeedPlan, SeedSet, TableSeed};
use std::collections::HashMap;

pub struct SeedExecutor<'a> {
    log: &'a Logger,
    db: Box<dyn Database>,
    tracking_table: String,
    reset: bool,
    refs: HashMap<String, HashMap<String, String>>,
}

impl<'a> SeedExecutor<'a> {
    pub fn new(
        log: &'a Logger,
        db: Box<dyn Database>,
        tracking_table: String,
        reset: bool,
    ) -> Self {
        Self {
            log,
            db,
            tracking_table,
            reset,
            refs: HashMap::new(),
        }
    }

    pub fn execute(&mut self, plan: &SeedPlan) -> Result<(), String> {
        self.log.info("starting seed execution", &[]);

        self.db.ensure_tracking_table(&self.tracking_table)?;

        let mut seed_sets: Vec<&SeedSet> = plan.seed_sets.iter().collect();
        seed_sets.sort_by_key(|s| s.order);

        for ss in &seed_sets {
            self.execute_seed_set(ss)?;
        }

        self.log.info("seed execution completed", &[]);
        Ok(())
    }

    fn execute_seed_set(&mut self, ss: &SeedSet) -> Result<(), String> {
        let name = &ss.name;
        self.log.info("processing seed set", &[("seed_set", name)]);

        if self.reset {
            self.log
                .info("reset mode: clearing seed set data", &[("seed_set", name)]);
            let mut tables: Vec<&TableSeed> = ss.tables.iter().collect();
            tables.sort_by_key(|t| std::cmp::Reverse(t.order));
            for ts in &tables {
                let count = self.db.delete_rows(&ts.table)?;
                self.log.info(
                    "deleted rows",
                    &[("table", &ts.table), ("count", &count.to_string())],
                );
            }
            self.db.remove_seed_mark(&self.tracking_table, name)?;
        }

        if self.db.is_seed_applied(&self.tracking_table, name)? {
            self.log
                .info("seed set already applied, skipping", &[("seed_set", name)]);
            return Ok(());
        }

        self.db.begin_transaction()?;
        let result = self.apply_seed_set_tables(ss);
        match result {
            Ok(()) => {
                self.db.mark_seed_applied(&self.tracking_table, &ss.name)?;
                self.db.commit_transaction()?;
                self.log
                    .info("seed set applied successfully", &[("seed_set", name)]);
                Ok(())
            }
            Err(e) => {
                self.db.rollback_transaction()?;
                Err(format!("seed set '{}' failed: {}", name, e))
            }
        }
    }

    fn apply_seed_set_tables(&mut self, ss: &SeedSet) -> Result<(), String> {
        let mut tables: Vec<&TableSeed> = ss.tables.iter().collect();
        tables.sort_by_key(|t| t.order);

        for ts in &tables {
            self.apply_table_seed(ts)?;
        }
        Ok(())
    }

    fn apply_table_seed(&mut self, ts: &TableSeed) -> Result<(), String> {
        let table = &ts.table;
        self.log.info(
            "seeding table",
            &[
                ("table", table.as_str()),
                ("rows", &ts.rows.len().to_string()),
            ],
        );

        for (idx, row) in ts.rows.iter().enumerate() {
            let ref_name = row
                .get("_ref")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let mut columns = Vec::new();
            let mut values = Vec::new();
            let mut unique_columns = Vec::new();
            let mut unique_values = Vec::new();

            for (key, val) in row {
                if key == "_ref" {
                    continue;
                }
                let resolved = self.resolve_value(val)?;
                columns.push(key.clone());
                values.push(resolved.clone());

                if ts.unique_key.contains(key) {
                    unique_columns.push(key.clone());
                    unique_values.push(resolved);
                }
            }

            if let Some(ref auto_id) = ts.auto_id {
                if !columns.contains(&auto_id.column) {
                    // Skip auto_id column; let the database generate it
                }
            }

            if !ts.unique_key.is_empty() {
                if self.db.row_exists(table, &unique_columns, &unique_values)? {
                    self.log.info(
                        "row already exists, skipping",
                        &[("table", table.as_str()), ("row", &(idx + 1).to_string())],
                    );
                    continue;
                }
            }

            let generated_id = self.db.insert_row(table, &columns, &values)?;

            if let Some(ref_key) = ref_name {
                let mut ref_map = HashMap::new();
                for (i, col) in columns.iter().enumerate() {
                    ref_map.insert(col.clone(), values[i].clone());
                }
                if let (Some(ref auto_id), Some(id)) = (&ts.auto_id, generated_id) {
                    ref_map.insert(auto_id.column.clone(), id.to_string());
                }
                self.refs.insert(ref_key, ref_map);
            }

            self.log.info(
                "inserted row",
                &[("table", table.as_str()), ("row", &(idx + 1).to_string())],
            );
        }

        Ok(())
    }

    fn resolve_value(&self, val: &serde_yaml::Value) -> Result<String, String> {
        match val {
            serde_yaml::Value::String(s) => {
                if let Some(ref_expr) = s.strip_prefix("@ref:") {
                    self.resolve_reference(ref_expr)
                } else if let Some(env_expr) = s.strip_prefix("$env:") {
                    std::env::var(env_expr)
                        .map_err(|_| format!("environment variable '{}' not set", env_expr))
                } else {
                    Ok(s.clone())
                }
            }
            serde_yaml::Value::Number(n) => Ok(n.to_string()),
            serde_yaml::Value::Bool(b) => Ok(b.to_string()),
            serde_yaml::Value::Null => Ok(String::new()),
            _ => Ok(format!("{:?}", val)),
        }
    }

    fn resolve_reference(&self, expr: &str) -> Result<String, String> {
        let parts: Vec<&str> = expr.splitn(2, '.').collect();
        if parts.len() != 2 {
            return Err(format!(
                "invalid reference '{}': expected format 'ref_name.column'",
                expr
            ));
        }
        let ref_name = parts[0];
        let column = parts[1];
        let ref_map = self.refs.get(ref_name).ok_or_else(|| {
            format!(
                "reference '{}' not found (ensure it appears before use)",
                ref_name
            )
        })?;
        ref_map
            .get(column)
            .cloned()
            .ok_or_else(|| format!("column '{}' not found in reference '{}'", column, ref_name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::{Level, Logger};
    use crate::seed::db::SqliteDb;
    use crate::seed::schema::SeedPlan;
    use std::io::Write;

    fn test_logger() -> Logger {
        struct NullWriter;
        impl Write for NullWriter {
            fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
                Ok(data.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        Logger::new(Box::new(NullWriter), false, Level::Info)
    }

    fn setup_db_with_tables(db: &SqliteDb) {
        db.conn
            .execute_batch(
                "CREATE TABLE departments (id INTEGER PRIMARY KEY, name TEXT UNIQUE);
                 CREATE TABLE employees (id INTEGER PRIMARY KEY, name TEXT, email TEXT UNIQUE, department_id INTEGER);",
            )
            .unwrap();
    }

    #[test]
    fn test_basic_seed_execution() {
        let yaml = r#"
version: "1"
database:
  driver: sqlite
  url: ":memory:"
seed_sets:
  - name: basic
    tables:
      - table: departments
        unique_key: [name]
        auto_id:
          column: id
        rows:
          - name: Engineering
          - name: Sales
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        setup_db_with_tables(&sqlite);

        let log = test_logger();
        let mut executor = SeedExecutor::new(&log, Box::new(sqlite), "initium_seed".into(), false);
        executor.execute(&plan).unwrap();

        let db = SqliteDb::connect(db_path_str).unwrap();
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM departments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2, "expected 2 departments");

        let names: Vec<String> = db
            .conn
            .prepare("SELECT name FROM departments ORDER BY name")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(names, vec!["Engineering", "Sales"]);
    }

    #[test]
    fn test_idempotent_seed() {
        let yaml = r#"
version: "1"
database:
  driver: sqlite
  url: ":memory:"
seed_sets:
  - name: idempotent
    tables:
      - table: departments
        unique_key: [name]
        rows:
          - name: Engineering
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        setup_db_with_tables(&sqlite);

        let log = test_logger();
        let mut executor = SeedExecutor::new(&log, Box::new(sqlite), "initium_seed".into(), false);
        executor.execute(&plan).unwrap();
        // Second execution should skip (already applied)
        executor.execute(&plan).unwrap();

        let db = SqliteDb::connect(db_path_str).unwrap();
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM departments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            count, 1,
            "expected exactly 1 department after idempotent re-run"
        );

        let name: String = db
            .conn
            .query_row("SELECT name FROM departments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "Engineering");
    }

    #[test]
    fn test_unique_key_skip_duplicates() {
        let yaml = r#"
version: "1"
database:
  driver: sqlite
  url: ":memory:"
seed_sets:
  - name: dupes
    tables:
      - table: departments
        unique_key: [name]
        rows:
          - name: Engineering
          - name: Engineering
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        setup_db_with_tables(&sqlite);

        let log = test_logger();
        let mut executor = SeedExecutor::new(&log, Box::new(sqlite), "initium_seed".into(), false);
        executor.execute(&plan).unwrap();

        let db = SqliteDb::connect(db_path_str).unwrap();
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM departments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1, "duplicate row should have been skipped");

        let name: String = db
            .conn
            .query_row("SELECT name FROM departments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "Engineering");
    }

    #[test]
    fn test_reference_resolution() {
        let yaml = r#"
version: "1"
database:
  driver: sqlite
  url: ":memory:"
seed_sets:
  - name: with_refs
    tables:
      - table: departments
        order: 1
        auto_id:
          column: id
        rows:
          - _ref: dept_eng
            name: Engineering
      - table: employees
        order: 2
        rows:
          - name: Alice
            email: alice@example.com
            department_id: "@ref:dept_eng.id"
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        setup_db_with_tables(&sqlite);

        let log = test_logger();
        let mut executor = SeedExecutor::new(&log, Box::new(sqlite), "initium_seed".into(), false);
        executor.execute(&plan).unwrap();

        let db = SqliteDb::connect(db_path_str).unwrap();
        let dept_id: Option<i64> = db
            .conn
            .query_row(
                "SELECT id FROM departments WHERE name = 'Engineering'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(dept_id.is_some(), "department id should not be NULL");
        let dept_id = dept_id.unwrap();

        let (emp_name, emp_email, emp_dept_id): (String, String, Option<i64>) = db
            .conn
            .query_row(
                "SELECT name, email, department_id FROM employees",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert!(
            emp_dept_id.is_some(),
            "employee department_id should not be NULL"
        );
        let emp_dept_id = emp_dept_id.unwrap();
        assert_eq!(emp_name, "Alice");
        assert_eq!(emp_email, "alice@example.com");
        assert_eq!(
            emp_dept_id, dept_id,
            "employee department_id should match referenced department"
        );
    }

    #[test]
    fn test_multiple_references_same_table() {
        let yaml = r#"
version: "1"
database:
  driver: sqlite
  url: ":memory:"
seed_sets:
  - name: multi_refs
    tables:
      - table: departments
        order: 1
        auto_id:
          column: id
        rows:
          - _ref: dept_eng
            name: Engineering
          - _ref: dept_sales
            name: Sales
      - table: employees
        order: 2
        rows:
          - name: Alice
            email: alice@example.com
            department_id: "@ref:dept_eng.id"
          - name: Bob
            email: bob@example.com
            department_id: "@ref:dept_eng.id"
          - name: Carol
            email: carol@example.com
            department_id: "@ref:dept_sales.id"
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        setup_db_with_tables(&sqlite);

        let log = test_logger();
        let mut executor = SeedExecutor::new(&log, Box::new(sqlite), "initium_seed".into(), false);
        executor.execute(&plan).unwrap();

        let db = SqliteDb::connect(db_path_str).unwrap();

        // Verify 2 departments with different IDs
        let dept_count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM departments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(dept_count, 2, "expected 2 departments");

        let eng_id: Option<i64> = db
            .conn
            .query_row(
                "SELECT id FROM departments WHERE name = 'Engineering'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            eng_id.is_some(),
            "Engineering department id should not be NULL"
        );
        let eng_id = eng_id.unwrap();

        let sales_id: Option<i64> = db
            .conn
            .query_row("SELECT id FROM departments WHERE name = 'Sales'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(sales_id.is_some(), "Sales department id should not be NULL");
        let sales_id = sales_id.unwrap();

        assert_ne!(
            eng_id, sales_id,
            "department IDs should be different between rows"
        );

        // Verify 3 employees
        let emp_count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM employees", [], |r| r.get(0))
            .unwrap();
        assert_eq!(emp_count, 3, "expected 3 employees");

        // Verify Alice -> Engineering
        let alice_dept: Option<i64> = db
            .conn
            .query_row(
                "SELECT department_id FROM employees WHERE name = 'Alice'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            alice_dept.is_some(),
            "Alice department_id should not be NULL"
        );
        assert_eq!(
            alice_dept.unwrap(),
            eng_id,
            "Alice should reference Engineering department"
        );

        // Verify Bob -> Engineering
        let bob_dept: Option<i64> = db
            .conn
            .query_row(
                "SELECT department_id FROM employees WHERE name = 'Bob'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(bob_dept.is_some(), "Bob department_id should not be NULL");
        assert_eq!(
            bob_dept.unwrap(),
            eng_id,
            "Bob should reference Engineering department"
        );

        // Verify Carol -> Sales
        let carol_dept: Option<i64> = db
            .conn
            .query_row(
                "SELECT department_id FROM employees WHERE name = 'Carol'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            carol_dept.is_some(),
            "Carol department_id should not be NULL"
        );
        assert_eq!(
            carol_dept.unwrap(),
            sales_id,
            "Carol should reference Sales department"
        );
    }

    #[test]
    fn test_reset_mode() {
        let yaml = r#"
version: "1"
database:
  driver: sqlite
  url: ":memory:"
seed_sets:
  - name: resetable
    tables:
      - table: departments
        unique_key: [name]
        rows:
          - name: Engineering
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let sqlite = SqliteDb::connect(":memory:").unwrap();
        setup_db_with_tables(&sqlite);

        let log = test_logger();

        // First apply normally
        {
            let _executor = SeedExecutor::new(
                &log,
                Box::new(SqliteDb::connect(":memory:").unwrap()),
                "initium_seed".into(),
                false,
            );
            // We need the same db instance, so let's use a file-based approach
        }

        // Use a temp file for persistence across executor instances
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let db1 = SqliteDb::connect(db_path_str).unwrap();
        db1.conn
            .execute_batch("CREATE TABLE departments (id INTEGER PRIMARY KEY, name TEXT UNIQUE);")
            .unwrap();

        let mut exec1 = SeedExecutor::new(&log, Box::new(db1), "initium_seed".into(), false);
        exec1.execute(&plan).unwrap();

        // Verify row was inserted
        let db_check = SqliteDb::connect(db_path_str).unwrap();
        let count: i64 = db_check
            .conn
            .query_row("SELECT COUNT(*) FROM departments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);

        // Now reset
        let db2 = SqliteDb::connect(db_path_str).unwrap();
        let mut exec2 = SeedExecutor::new(&log, Box::new(db2), "initium_seed".into(), true);
        exec2.execute(&plan).unwrap();

        // After reset + re-seed, should still have 1 row
        let db_final = SqliteDb::connect(db_path_str).unwrap();
        let count: i64 = db_final
            .conn
            .query_row("SELECT COUNT(*) FROM departments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_env_substitution() {
        std::env::set_var("TEST_SEED_DEPT_NAME", "FromEnv");
        let yaml = r#"
version: "1"
database:
  driver: sqlite
  url: ":memory:"
seed_sets:
  - name: env_test
    tables:
      - table: departments
        rows:
          - name: "$env:TEST_SEED_DEPT_NAME"
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        setup_db_with_tables(&sqlite);

        let log = test_logger();
        let mut executor = SeedExecutor::new(&log, Box::new(sqlite), "initium_seed".into(), false);
        executor.execute(&plan).unwrap();
        std::env::remove_var("TEST_SEED_DEPT_NAME");

        let db = SqliteDb::connect(db_path_str).unwrap();
        let name: String = db
            .conn
            .query_row("SELECT name FROM departments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "FromEnv", "env variable should have been substituted");
    }

    #[test]
    fn test_ordering() {
        let yaml = r#"
version: "1"
database:
  driver: sqlite
  url: ":memory:"
seed_sets:
  - name: second
    order: 2
    tables:
      - table: departments
        rows:
          - name: Dept2
  - name: first
    order: 1
    tables:
      - table: departments
        rows:
          - name: Dept1
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        setup_db_with_tables(&sqlite);

        let log = test_logger();
        let mut executor = SeedExecutor::new(&log, Box::new(sqlite), "initium_seed".into(), false);
        executor.execute(&plan).unwrap();

        let db = SqliteDb::connect(db_path_str).unwrap();
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM departments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2, "both seed sets should have been applied");

        // "first" (order=1) runs before "second" (order=2), so Dept1 gets id 1
        let names: Vec<String> = db
            .conn
            .prepare("SELECT name FROM departments ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(
            names,
            vec!["Dept1", "Dept2"],
            "seed sets should be applied in order"
        );
    }

    #[test]
    fn test_empty_rows() {
        let yaml = r#"
version: "1"
database:
  driver: sqlite
  url: ":memory:"
seed_sets:
  - name: empty
    tables:
      - table: departments
        rows: []
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        setup_db_with_tables(&sqlite);

        let log = test_logger();
        let mut executor = SeedExecutor::new(&log, Box::new(sqlite), "initium_seed".into(), false);
        executor.execute(&plan).unwrap();

        let db = SqliteDb::connect(db_path_str).unwrap();
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM departments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            count, 0,
            "no rows should have been inserted for empty rows list"
        );
    }

    #[test]
    fn test_invalid_reference() {
        let yaml = r#"
version: "1"
database:
  driver: sqlite
  url: ":memory:"
seed_sets:
  - name: bad_ref
    tables:
      - table: departments
        rows:
          - name: "@ref:nonexistent.id"
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let sqlite = SqliteDb::connect(":memory:").unwrap();
        setup_db_with_tables(&sqlite);

        let log = test_logger();
        let mut executor = SeedExecutor::new(&log, Box::new(sqlite), "initium_seed".into(), false);
        let result = executor.execute(&plan);
        assert!(result.is_err());
    }

    #[test]
    fn test_numeric_and_boolean_values() {
        let yaml = r#"
version: "1"
database:
  driver: sqlite
  url: ":memory:"
seed_sets:
  - name: types
    tables:
      - table: config
        rows:
          - key: max_retries
            value: 5
          - key: enabled
            value: true
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        sqlite
            .conn
            .execute("CREATE TABLE config (key TEXT, value TEXT)", [])
            .unwrap();

        let log = test_logger();
        let mut executor = SeedExecutor::new(&log, Box::new(sqlite), "initium_seed".into(), false);
        executor.execute(&plan).unwrap();

        let db = SqliteDb::connect(db_path_str).unwrap();
        let rows: Vec<(String, String)> = db
            .conn
            .prepare("SELECT key, value FROM config ORDER BY key")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], ("enabled".to_string(), "true".to_string()));
        assert_eq!(rows[1], ("max_retries".to_string(), "5".to_string()));
    }
}
