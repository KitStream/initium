use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone)]
pub struct SeedPlan {
    pub version: String,
    #[serde(default)]
    pub database: DatabaseConfig,
    pub seed_sets: Vec<SeedSet>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct DatabaseConfig {
    #[serde(default = "default_driver")]
    pub driver: String,
    #[serde(default)]
    pub url_env: String,
    #[serde(default)]
    pub url: String,
    #[serde(default = "default_tracking_table")]
    pub tracking_table: String,
}

fn default_driver() -> String {
    "postgres".into()
}

fn default_tracking_table() -> String {
    "initium_seed".into()
}

#[derive(Debug, Deserialize, Clone)]
pub struct SeedSet {
    pub name: String,
    #[serde(default)]
    pub order: i32,
    pub tables: Vec<TableSeed>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TableSeed {
    pub table: String,
    #[serde(default)]
    pub order: i32,
    #[serde(default)]
    pub unique_key: Vec<String>,
    #[serde(default)]
    pub auto_id: Option<AutoIdConfig>,
    pub rows: Vec<HashMap<String, serde_yaml::Value>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AutoIdConfig {
    pub column: String,
    /// Reserved for future use (e.g. UUID generation); parsed from spec for forward compatibility.
    #[serde(default = "default_auto_id_type")]
    #[allow(dead_code)]
    pub id_type: String,
}

fn default_auto_id_type() -> String {
    "integer".into()
}

impl SeedPlan {
    pub fn from_yaml(content: &str) -> Result<Self, String> {
        let plan: SeedPlan =
            serde_yaml::from_str(content).map_err(|e| format!("parsing seed YAML: {}", e))?;
        plan.validate()?;
        Ok(plan)
    }

    pub fn from_json(content: &str) -> Result<Self, String> {
        let plan: SeedPlan =
            serde_json::from_str(content).map_err(|e| format!("parsing seed JSON: {}", e))?;
        plan.validate()?;
        Ok(plan)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != "1" {
            return Err(format!(
                "unsupported seed schema version: {} (expected \"1\")",
                self.version
            ));
        }
        if self.seed_sets.is_empty() {
            return Err("seed plan must contain at least one seed_set".into());
        }
        for ss in &self.seed_sets {
            if ss.name.is_empty() {
                return Err("seed_set name must not be empty".into());
            }
            if ss.tables.is_empty() {
                return Err(format!(
                    "seed_set '{}' must contain at least one table",
                    ss.name
                ));
            }
            for ts in &ss.tables {
                if ts.table.is_empty() {
                    return Err(format!(
                        "table name must not be empty in seed_set '{}'",
                        ss.name
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn resolve_db_url(&self) -> Result<String, String> {
        if !self.database.url_env.is_empty() {
            std::env::var(&self.database.url_env).map_err(|_| {
                format!(
                    "environment variable '{}' not set for database URL",
                    self.database.url_env
                )
            })
        } else if !self.database.url.is_empty() {
            Ok(self.database.url.clone())
        } else {
            std::env::var("DATABASE_URL")
                .map_err(|_| "no database URL configured: set database.url, database.url_env, or DATABASE_URL env var".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_yaml() {
        let yaml = r#"
version: "1"
database:
  driver: sqlite
  url: ":memory:"
seed_sets:
  - name: basic
    tables:
      - table: users
        rows:
          - name: alice
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        assert_eq!(plan.version, "1");
        assert_eq!(plan.seed_sets.len(), 1);
        assert_eq!(plan.seed_sets[0].tables[0].table, "users");
    }

    #[test]
    fn test_parse_with_auto_id_and_unique_key() {
        let yaml = r#"
version: "1"
database:
  driver: sqlite
  url: ":memory:"
  tracking_table: my_seeds
seed_sets:
  - name: accounts
    order: 1
    tables:
      - table: accounts
        order: 1
        unique_key: [email]
        auto_id:
          column: id
          id_type: integer
        rows:
          - email: alice@example.com
            role: admin
          - email: bob@example.com
            role: user
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let ts = &plan.seed_sets[0].tables[0];
        assert_eq!(ts.unique_key, vec!["email"]);
        assert!(ts.auto_id.is_some());
        assert_eq!(ts.auto_id.as_ref().unwrap().column, "id");
        assert_eq!(ts.rows.len(), 2);
        assert_eq!(plan.database.tracking_table, "my_seeds");
    }

    #[test]
    fn test_parse_json() {
        let json = r#"{
  "version": "1",
  "database": {"driver": "sqlite", "url": ":memory:"},
  "seed_sets": [
    {
      "name": "test",
      "tables": [
        {"table": "items", "rows": [{"name": "thing"}]}
      ]
    }
  ]
}"#;
        let plan = SeedPlan::from_json(json).unwrap();
        assert_eq!(plan.seed_sets[0].name, "test");
    }

    #[test]
    fn test_invalid_version() {
        let yaml = r#"
version: "2"
seed_sets:
  - name: x
    tables:
      - table: t
        rows: []
"#;
        assert!(SeedPlan::from_yaml(yaml).is_err());
    }

    #[test]
    fn test_empty_seed_sets() {
        let yaml = r#"
version: "1"
seed_sets: []
"#;
        assert!(SeedPlan::from_yaml(yaml).is_err());
    }

    #[test]
    fn test_empty_table_name() {
        let yaml = r#"
version: "1"
seed_sets:
  - name: x
    tables:
      - table: ""
        rows: []
"#;
        assert!(SeedPlan::from_yaml(yaml).is_err());
    }

    #[test]
    fn test_resolve_url_from_config() {
        let yaml = r#"
version: "1"
database:
  driver: sqlite
  url: "test.db"
seed_sets:
  - name: x
    tables:
      - table: t
        rows: []
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        assert_eq!(plan.resolve_db_url().unwrap(), "test.db");
    }

    #[test]
    fn test_resolve_url_from_env() {
        std::env::set_var("TEST_SEED_DB_URL", "postgres://localhost/test");
        let yaml = r#"
version: "1"
database:
  driver: postgres
  url_env: TEST_SEED_DB_URL
seed_sets:
  - name: x
    tables:
      - table: t
        rows: []
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        assert_eq!(plan.resolve_db_url().unwrap(), "postgres://localhost/test");
        std::env::remove_var("TEST_SEED_DB_URL");
    }

    #[test]
    fn test_default_tracking_table() {
        let yaml = r#"
version: "1"
database:
  driver: sqlite
  url: ":memory:"
seed_sets:
  - name: x
    tables:
      - table: t
        rows: []
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        assert_eq!(plan.database.tracking_table, "initium_seed");
    }

    #[test]
    fn test_references_in_values() {
        let yaml = r#"
version: "1"
database:
  driver: sqlite
  url: ":memory:"
seed_sets:
  - name: refs
    tables:
      - table: departments
        auto_id:
          column: id
        rows:
          - _ref: dept_eng
            name: Engineering
      - table: employees
        rows:
          - name: Alice
            department_id: "@ref:dept_eng.id"
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let emp_rows = &plan.seed_sets[0].tables[1].rows;
        let dept_id = emp_rows[0].get("department_id").unwrap();
        assert_eq!(dept_id.as_str().unwrap(), "@ref:dept_eng.id");
    }
}
