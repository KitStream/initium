use serde::de::{self, Deserializer};
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;

fn deserialize_string_or_number<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringOrNumber;
    impl<'de> de::Visitor<'de> for StringOrNumber {
        type Value = String;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string or number")
        }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<String, E> {
            Ok(v.to_string())
        }
        fn visit_i64<E: de::Error>(self, v: i64) -> Result<String, E> {
            Ok(v.to_string())
        }
        fn visit_u64<E: de::Error>(self, v: u64) -> Result<String, E> {
            Ok(v.to_string())
        }
        fn visit_f64<E: de::Error>(self, v: f64) -> Result<String, E> {
            Ok(v.to_string())
        }
    }
    deserializer.deserialize_any(StringOrNumber)
}

fn deserialize_optional_string_or_number<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct OptStringOrNumber;
    impl<'de> de::Visitor<'de> for OptStringOrNumber {
        type Value = Option<String>;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string, number, or null")
        }
        fn visit_none<E: de::Error>(self) -> Result<Option<String>, E> {
            Ok(None)
        }
        fn visit_unit<E: de::Error>(self) -> Result<Option<String>, E> {
            Ok(None)
        }
        fn visit_some<D2: Deserializer<'de>>(self, d: D2) -> Result<Option<String>, D2::Error> {
            deserialize_string_or_number(d).map(Some)
        }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<Option<String>, E> {
            Ok(Some(v.to_string()))
        }
        fn visit_i64<E: de::Error>(self, v: i64) -> Result<Option<String>, E> {
            Ok(Some(v.to_string()))
        }
        fn visit_u64<E: de::Error>(self, v: u64) -> Result<Option<String>, E> {
            Ok(Some(v.to_string()))
        }
        fn visit_f64<E: de::Error>(self, v: f64) -> Result<Option<String>, E> {
            Ok(Some(v.to_string()))
        }
    }
    deserializer.deserialize_any(OptStringOrNumber)
}

#[derive(Debug, Deserialize, Clone)]
pub struct SeedPlan {
    #[serde(default)]
    pub database: DatabaseConfig,
    pub phases: Vec<SeedPhase>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct DatabaseConfig {
    #[serde(default = "default_driver")]
    pub driver: String,
    #[serde(default)]
    pub url_env: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub options: HashMap<String, String>,
    #[serde(default = "default_tracking_table")]
    pub tracking_table: String,
}

impl DatabaseConfig {
    pub fn has_structured_config(&self) -> bool {
        !self.host.is_empty()
    }

    fn has_url_config(&self) -> bool {
        !self.url.is_empty() || !self.url_env.is_empty()
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.has_structured_config() && self.has_url_config() {
            return Err(
                "database config must use either structured fields (host, port, user, password, name) or url/url_env, not both".into(),
            );
        }
        Ok(())
    }
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
    #[serde(default = "default_seed_mode")]
    pub mode: String,
    pub tables: Vec<TableSeed>,
}

fn default_seed_mode() -> String {
    "once".into()
}

impl SeedSet {
    pub fn is_reconcile(&self) -> bool {
        self.mode == "reconcile"
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct TableSeed {
    pub table: String,
    #[serde(default)]
    pub order: i32,
    #[serde(default)]
    pub unique_key: Vec<String>,
    #[serde(default)]
    pub ignore_columns: Vec<String>,
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

#[derive(Debug, Deserialize, Clone)]
pub struct SeedPhase {
    pub name: String,
    #[serde(default)]
    pub order: i32,
    #[serde(default)]
    pub database: String,
    #[serde(default)]
    pub schema: String,
    #[serde(default)]
    pub create_if_missing: bool,
    #[serde(default)]
    pub wait_for: Vec<WaitForObject>,
    #[serde(
        default = "default_phase_timeout",
        deserialize_with = "deserialize_string_or_number"
    )]
    pub timeout: String,
    #[serde(default)]
    pub seed_sets: Vec<SeedSet>,
}

fn default_phase_timeout() -> String {
    "30s".into()
}

#[derive(Debug, Deserialize, Clone)]
pub struct WaitForObject {
    #[serde(rename = "type")]
    pub obj_type: String,
    pub name: String,
    #[serde(default, deserialize_with = "deserialize_optional_string_or_number")]
    pub timeout: Option<String>,
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
        self.database.validate()?;
        if self.phases.is_empty() {
            return Err("seed plan must contain at least one phase".into());
        }
        for phase in &self.phases {
            if phase.name.is_empty() {
                return Err("phase name must not be empty".into());
            }
            for wf in &phase.wait_for {
                Self::validate_wait_for(wf)?;
            }
            for ss in &phase.seed_sets {
                Self::validate_seed_set(ss)?;
            }
        }
        Ok(())
    }

    fn validate_seed_set(ss: &SeedSet) -> Result<(), String> {
        if ss.name.is_empty() {
            return Err("seed_set name must not be empty".into());
        }
        let valid_modes = ["once", "reconcile"];
        if !valid_modes.contains(&ss.mode.as_str()) {
            return Err(format!(
                "seed_set '{}' has invalid mode '{}' (supported: {})",
                ss.name,
                ss.mode,
                valid_modes.join(", ")
            ));
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
            if ss.is_reconcile() && ts.unique_key.is_empty() {
                return Err(format!(
                    "table '{}' in seed_set '{}' must have unique_key when mode is 'reconcile'",
                    ts.table, ss.name
                ));
            }
            if ss.is_reconcile() {
                if ts.unique_key.iter().any(|k| k.trim().is_empty()) {
                    return Err(format!(
                        "table '{}' in seed_set '{}' has empty or whitespace-only entries in unique_key when mode is 'reconcile'",
                        ts.table, ss.name
                    ));
                }
                let reserved_keys = ["_ref"];
                if let Some(reserved) = ts
                    .unique_key
                    .iter()
                    .find(|k| reserved_keys.contains(&k.as_str()))
                {
                    return Err(format!(
                        "table '{}' in seed_set '{}' uses reserved column '{}' in unique_key when mode is 'reconcile'",
                        ts.table, ss.name, reserved
                    ));
                }
                if ts.ignore_columns.iter().any(|c| c.trim().is_empty()) {
                    return Err(format!(
                        "table '{}' in seed_set '{}' has empty or whitespace-only entries in ignore_columns",
                        ts.table, ss.name
                    ));
                }
                for ic in &ts.ignore_columns {
                    if ts.unique_key.contains(ic) {
                        return Err(format!(
                            "table '{}' in seed_set '{}': column '{}' cannot be in both unique_key and ignore_columns",
                            ts.table, ss.name, ic
                        ));
                    }
                }
                for (row_idx, row) in ts.rows.iter().enumerate() {
                    for uk in &ts.unique_key {
                        if !row.contains_key(uk) {
                            return Err(format!(
                                "table '{}' in seed_set '{}': row {} is missing unique_key column '{}'",
                                ts.table, ss.name, row_idx + 1, uk
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_wait_for(wf: &WaitForObject) -> Result<(), String> {
        let valid_types = ["table", "view", "schema", "database"];
        if !valid_types.contains(&wf.obj_type.as_str()) {
            return Err(format!(
                "unsupported wait_for type '{}' (supported: {})",
                wf.obj_type,
                valid_types.join(", ")
            ));
        }
        if wf.name.is_empty() {
            return Err(format!(
                "wait_for name must not be empty for type '{}'",
                wf.obj_type
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_yaml() {
        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: basic
    seed_sets:
      - name: basic
        tables:
          - table: users
            rows:
              - name: alice
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        assert_eq!(plan.phases.len(), 1);
        assert_eq!(plan.phases[0].seed_sets[0].tables[0].table, "users");
    }

    #[test]
    fn test_parse_with_auto_id_and_unique_key() {
        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
  tracking_table: my_seeds
phases:
  - name: accounts_phase
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
        let ts = &plan.phases[0].seed_sets[0].tables[0];
        assert_eq!(ts.unique_key, vec!["email"]);
        assert!(ts.auto_id.is_some());
        assert_eq!(ts.auto_id.as_ref().unwrap().column, "id");
        assert_eq!(ts.rows.len(), 2);
        assert_eq!(plan.database.tracking_table, "my_seeds");
    }

    #[test]
    fn test_parse_json() {
        let json = r#"{
  "database": {"driver": "sqlite", "url": ":memory:"},
  "phases": [
    {
      "name": "phase1",
      "seed_sets": [
        {
          "name": "test",
          "tables": [
            {"table": "items", "rows": [{"name": "thing"}]}
          ]
        }
      ]
    }
  ]
}"#;
        let plan = SeedPlan::from_json(json).unwrap();
        assert_eq!(plan.phases[0].seed_sets[0].name, "test");
    }

    #[test]
    fn test_empty_phases() {
        let yaml = r#"
phases: []
"#;
        assert!(SeedPlan::from_yaml(yaml).is_err());
    }

    #[test]
    fn test_empty_table_name() {
        let yaml = r#"
phases:
  - name: phase1
    seed_sets:
      - name: x
        tables:
          - table: ""
            rows: []
"#;
        assert!(SeedPlan::from_yaml(yaml).is_err());
    }

    #[test]
    fn test_url_config() {
        let yaml = r#"
database:
  driver: sqlite
  url: "test.db"
phases:
  - name: phase1
    seed_sets:
      - name: x
        tables:
          - table: t
            rows: []
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        assert_eq!(plan.database.url, "test.db");
        assert!(!plan.database.has_structured_config());
    }

    #[test]
    fn test_url_env_config() {
        let yaml = r#"
database:
  driver: postgres
  url_env: TEST_SEED_DB_URL
phases:
  - name: phase1
    seed_sets:
      - name: x
        tables:
          - table: t
            rows: []
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        assert_eq!(plan.database.url_env, "TEST_SEED_DB_URL");
        assert!(!plan.database.has_structured_config());
    }

    #[test]
    fn test_structured_config() {
        let yaml = r#"
database:
  driver: postgres
  host: pg.example.com
  port: 5432
  user: netbird
  password: "s3cret!"
  name: mydb
  options:
    sslmode: disable
phases:
  - name: phase1
    seed_sets:
      - name: x
        tables:
          - table: t
            rows: []
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        assert!(plan.database.has_structured_config());
        assert_eq!(plan.database.host, "pg.example.com");
        assert_eq!(plan.database.port, Some(5432));
        assert_eq!(plan.database.user, "netbird");
        assert_eq!(plan.database.password, "s3cret!");
        assert_eq!(plan.database.name, "mydb");
        assert_eq!(plan.database.options.get("sslmode").unwrap(), "disable");
    }

    #[test]
    fn test_structured_config_default_port() {
        let yaml = r#"
database:
  driver: postgres
  host: localhost
  user: app
  name: mydb
phases:
  - name: phase1
    seed_sets:
      - name: x
        tables:
          - table: t
            rows: []
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        assert!(plan.database.has_structured_config());
        assert_eq!(plan.database.port, None);
    }

    #[test]
    fn test_rejects_url_and_structured_config() {
        let yaml = r#"
database:
  driver: postgres
  url: "postgres://localhost/db"
  host: localhost
phases:
  - name: phase1
    seed_sets:
      - name: x
        tables:
          - table: t
            rows: []
"#;
        let err = SeedPlan::from_yaml(yaml).unwrap_err();
        assert!(err.contains("not both"));
    }

    #[test]
    fn test_rejects_url_env_and_structured_config() {
        let yaml = r#"
database:
  driver: postgres
  url_env: DATABASE_URL
  host: localhost
phases:
  - name: phase1
    seed_sets:
      - name: x
        tables:
          - table: t
            rows: []
"#;
        let err = SeedPlan::from_yaml(yaml).unwrap_err();
        assert!(err.contains("not both"));
    }

    #[test]
    fn test_default_tracking_table() {
        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
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
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
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
        let emp_rows = &plan.phases[0].seed_sets[0].tables[1].rows;
        let dept_id = emp_rows[0].get("department_id").unwrap();
        assert_eq!(dept_id.as_str().unwrap(), "@ref:dept_eng.id");
    }

    #[test]
    fn test_parse_phases() {
        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: setup
    create_if_missing: true
    wait_for:
      - type: table
        name: config
    timeout: 10
    seed_sets:
      - name: initial
        tables:
          - table: config
            rows:
              - key: app_name
                value: test
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        assert_eq!(plan.phases.len(), 1);
        assert_eq!(plan.phases[0].name, "setup");
        assert!(plan.phases[0].create_if_missing);
        assert_eq!(plan.phases[0].wait_for.len(), 1);
        assert_eq!(plan.phases[0].wait_for[0].obj_type, "table");
        assert_eq!(plan.phases[0].wait_for[0].name, "config");
        assert_eq!(plan.phases[0].timeout, "10");
        assert_eq!(plan.phases[0].seed_sets.len(), 1);
    }

    #[test]
    fn test_empty_phases_error() {
        let yaml = r#"
phases: []
"#;
        let err = SeedPlan::from_yaml(yaml).unwrap_err();
        assert!(err.contains("at least one phase"));
    }

    #[test]
    fn test_empty_phase_name() {
        let yaml = r#"
phases:
  - name: ""
    seed_sets:
      - name: x
        tables:
          - table: t
            rows: []
"#;
        let err = SeedPlan::from_yaml(yaml).unwrap_err();
        assert!(err.contains("phase name must not be empty"));
    }

    #[test]
    fn test_invalid_wait_for_type() {
        let yaml = r#"
phases:
  - name: setup
    wait_for:
      - type: index
        name: my_index
"#;
        let err = SeedPlan::from_yaml(yaml).unwrap_err();
        assert!(err.contains("unsupported wait_for type"));
    }

    #[test]
    fn test_empty_wait_for_name() {
        let yaml = r#"
phases:
  - name: setup
    wait_for:
      - type: table
        name: ""
"#;
        let err = SeedPlan::from_yaml(yaml).unwrap_err();
        assert!(err.contains("wait_for name must not be empty"));
    }

    #[test]
    fn test_multiple_phases() {
        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    order: 1
    seed_sets:
      - name: s1
        tables:
          - table: t1
            rows:
              - a: b
  - name: phase2
    order: 2
    database: reporting
    schema: analytics
    create_if_missing: true
    wait_for:
      - type: schema
        name: analytics
    seed_sets:
      - name: s2
        tables:
          - table: t2
            rows:
              - c: d
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        assert_eq!(plan.phases.len(), 2);
        assert_eq!(plan.phases[0].name, "phase1");
        assert_eq!(plan.phases[1].name, "phase2");
        assert_eq!(plan.phases[1].database, "reporting");
        assert_eq!(plan.phases[1].schema, "analytics");
        assert!(plan.phases[1].create_if_missing);
    }

    #[test]
    fn test_default_timeout() {
        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: setup
    seed_sets:
      - name: s1
        tables:
          - table: t
            rows:
              - a: b
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        assert_eq!(plan.phases[0].timeout, "30s");
    }

    #[test]
    fn test_wait_for_with_per_object_timeout() {
        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: setup
    timeout: 60
    wait_for:
      - type: table
        name: users
        timeout: 120
      - type: view
        name: user_summary
    seed_sets:
      - name: s1
        tables:
          - table: t
            rows:
              - a: b
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let wf = &plan.phases[0].wait_for;
        assert_eq!(wf[0].timeout, Some("120".to_string()));
        assert_eq!(wf[1].timeout, None);
    }

    #[test]
    fn test_phase_without_seed_sets() {
        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: wait_only
    wait_for:
      - type: table
        name: users
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        assert!(plan.phases[0].seed_sets.is_empty());
    }

    #[test]
    fn test_reconcile_rejects_empty_unique_key_entry() {
        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: p
    seed_sets:
      - name: s
        mode: reconcile
        tables:
          - table: t
            unique_key: ["", "k"]
            rows:
              - k: a
"#;
        let err = SeedPlan::from_yaml(yaml).unwrap_err();
        assert!(err.contains("empty or whitespace-only"));
    }

    #[test]
    fn test_reconcile_rejects_reserved_unique_key() {
        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: p
    seed_sets:
      - name: s
        mode: reconcile
        tables:
          - table: t
            unique_key: [_ref]
            rows:
              - _ref: r1
"#;
        let err = SeedPlan::from_yaml(yaml).unwrap_err();
        assert!(err.contains("reserved column '_ref'"));
    }

    #[test]
    fn test_reconcile_rejects_row_missing_unique_key_column() {
        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: p
    seed_sets:
      - name: s
        mode: reconcile
        tables:
          - table: t
            unique_key: [email]
            rows:
              - name: Alice
"#;
        let err = SeedPlan::from_yaml(yaml).unwrap_err();
        assert!(err.contains("missing unique_key column 'email'"));
    }

    #[test]
    fn test_reconcile_rejects_ignore_columns_overlapping_unique_key() {
        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: p
    seed_sets:
      - name: s
        mode: reconcile
        tables:
          - table: t
            unique_key: [email]
            ignore_columns: [email]
            rows:
              - email: alice@co.com
"#;
        let err = SeedPlan::from_yaml(yaml).unwrap_err();
        assert!(err.contains("cannot be in both unique_key and ignore_columns"));
    }

    #[test]
    fn test_reconcile_rejects_empty_ignore_columns_entry() {
        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: p
    seed_sets:
      - name: s
        mode: reconcile
        tables:
          - table: t
            unique_key: [email]
            ignore_columns: [""]
            rows:
              - email: alice@co.com
"#;
        let err = SeedPlan::from_yaml(yaml).unwrap_err();
        assert!(err.contains("empty or whitespace-only entries in ignore_columns"));
    }

    #[test]
    fn test_reconcile_accepts_valid_ignore_columns() {
        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: p
    seed_sets:
      - name: s
        mode: reconcile
        tables:
          - table: t
            unique_key: [email]
            ignore_columns: [updated_at]
            rows:
              - email: alice@co.com
                updated_at: "2026-01-01"
"#;
        assert!(SeedPlan::from_yaml(yaml).is_ok());
    }
}
