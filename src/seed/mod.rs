pub mod db;
pub mod executor;
pub mod hash;
pub mod schema;

use crate::logging::Logger;

fn bootstrap_database(config: &schema::DatabaseConfig) -> String {
    if !config.default_database.is_empty() {
        return config.default_database.clone();
    }
    match config.driver.as_str() {
        // PostgreSQL requires connecting to an existing database; `postgres` is
        // guaranteed to exist on every cluster.
        "postgres" | "postgresql" => "postgres".into(),
        // MySQL can connect without selecting a database, which avoids needing
        // access to the `mysql` system schema.
        _ => String::new(),
    }
}

fn render_template(content: &str) -> Result<String, String> {
    let env_map: std::collections::HashMap<String, String> = std::env::vars().collect();
    let mut jinja_env = minijinja::Environment::new();
    jinja_env.set_undefined_behavior(minijinja::UndefinedBehavior::Lenient);
    crate::template_funcs::register(&mut jinja_env);
    jinja_env
        .add_template("seed", content)
        .map_err(|e| format!("parsing seed template: {}", e))?;
    let tmpl = jinja_env
        .get_template("seed")
        .map_err(|e| format!("getting seed template: {}", e))?;
    tmpl.render(minijinja::context!(env => env_map))
        .map_err(|e| format!("rendering seed template: {}", e))
}

pub fn run(
    log: &Logger,
    spec_file: &str,
    reset: bool,
    dry_run: bool,
    reconcile_all: bool,
) -> Result<(), String> {
    let content = std::fs::read_to_string(spec_file)
        .map_err(|e| format!("reading seed spec '{}': {}", spec_file, e))?;

    let rendered = render_template(&content)?;

    let plan = if spec_file.ends_with(".json") {
        schema::SeedPlan::from_json(&rendered)?
    } else {
        schema::SeedPlan::from_yaml(&rendered)?
    };

    let tracking_table = plan.database.tracking_table.clone();
    let driver = plan.database.driver.clone();

    // When using structured config and a phase needs to create a database that
    // matches the configured name, we try the normal connection first. If it
    // fails, we fall back to connecting to a bootstrap database, create the
    // target, then reconnect. See https://github.com/KitStream/initium/issues/50
    let may_need_bootstrap = plan.database.has_structured_config()
        && plan.phases.iter().any(|p| {
            p.create_if_missing && !p.database.is_empty() && p.database == plan.database.name
        });

    log.info("connecting to database", &[("driver", driver.as_str())]);

    let db = match db::connect(&plan.database) {
        Ok(db) => db,
        Err(err) if may_need_bootstrap => {
            log.info(
                "target database not reachable, bootstrapping via default database",
                &[("driver", driver.as_str())],
            );

            let mut admin_config = plan.database.clone();
            admin_config.name = bootstrap_database(&plan.database);

            let mut admin_db = db::connect(&admin_config)?;

            for phase in &plan.phases {
                if phase.create_if_missing && !phase.database.is_empty() {
                    log.info(
                        "creating database if missing",
                        &[("database", phase.database.as_str())],
                    );
                    admin_db.create_database(&phase.database)?;
                }
                // Schemas are database-scoped, so they must be created after
                // reconnecting to the target database. The executor handles
                // schema creation in execute_phase().
            }
            drop(admin_db);

            db::connect(&plan.database).map_err(|_| err)?
        }
        Err(err) => return Err(err),
    };
    let mut exec = executor::SeedExecutor::new(log, db, tracking_table, reset)
        .with_dry_run(dry_run)
        .with_reconcile_all(reconcile_all);
    exec.execute(&plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_template_plain_yaml() {
        let input = r#"
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
        let rendered = render_template(input).unwrap();
        assert!(rendered.contains("phases:"));
    }

    #[test]
    fn test_render_template_with_env() {
        std::env::set_var("TEST_SEED_RENDER_DRIVER", "sqlite");
        let input = r#"
database:
  driver: {{ env.TEST_SEED_RENDER_DRIVER }}
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
        let rendered = render_template(input).unwrap();
        assert!(rendered.contains("driver: sqlite"));
        std::env::remove_var("TEST_SEED_RENDER_DRIVER");
    }

    #[test]
    fn test_render_template_with_conditional() {
        std::env::set_var("TEST_SEED_ENABLE_PHASE2", "yes");
        let input = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: s1
        tables:
          - table: t
            rows:
              - a: b
{% if env.TEST_SEED_ENABLE_PHASE2 %}
  - name: phase2
    seed_sets:
      - name: s2
        tables:
          - table: t
            rows:
              - c: d
{% endif %}
"#;
        let rendered = render_template(input).unwrap();
        assert!(rendered.contains("phase2"));
        std::env::remove_var("TEST_SEED_ENABLE_PHASE2");
    }

    #[test]
    fn test_render_template_with_loop() {
        let input = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: setup
    seed_sets:
      - name: generated
        tables:
          - table: config
            rows:
{% for i in range(3) %}
              - key: item_{{ i }}
                value: val_{{ i }}
{% endfor %}
"#;
        let rendered = render_template(input).unwrap();
        assert!(rendered.contains("item_0"));
        assert!(rendered.contains("item_1"));
        assert!(rendered.contains("item_2"));
    }

    #[test]
    fn test_render_template_invalid() {
        let input = "{% invalid %}";
        let result = render_template(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_render_template_missing_env_lenient() {
        let input = r#"
database:
  driver: {{ env.NONEXISTENT_SEED_VAR_XYZ }}
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
        let rendered = render_template(input).unwrap();
        assert!(rendered.contains("driver:"));
    }
}
