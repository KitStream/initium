pub mod db;
pub mod executor;
pub mod schema;

use crate::logging::Logger;

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

pub fn run(log: &Logger, spec_file: &str, reset: bool) -> Result<(), String> {
    let content = std::fs::read_to_string(spec_file)
        .map_err(|e| format!("reading seed spec '{}': {}", spec_file, e))?;

    let rendered = render_template(&content)?;

    let plan = if spec_file.ends_with(".json") {
        schema::SeedPlan::from_json(&rendered)?
    } else {
        schema::SeedPlan::from_yaml(&rendered)?
    };

    let db_url = plan.resolve_db_url()?;
    let tracking_table = plan.database.tracking_table.clone();
    let driver = plan.database.driver.clone();

    log.info("connecting to database", &[("driver", driver.as_str())]);

    let db = db::connect(&driver, &db_url)?;
    let mut exec = executor::SeedExecutor::new(log, db, tracking_table, reset);
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
