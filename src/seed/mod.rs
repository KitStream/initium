pub mod db;
pub mod executor;
pub mod schema;

use crate::logging::Logger;

pub fn run(log: &Logger, spec_file: &str, reset: bool) -> Result<(), String> {
    let content = std::fs::read_to_string(spec_file)
        .map_err(|e| format!("reading seed spec '{}': {}", spec_file, e))?;

    let plan = if spec_file.ends_with(".json") {
        schema::SeedPlan::from_json(&content)?
    } else {
        schema::SeedPlan::from_yaml(&content)?
    };

    let db_url = plan.resolve_db_url()?;
    let tracking_table = plan.database.tracking_table.clone();
    let driver = plan.database.driver.clone();

    log.info("connecting to database", &[("driver", driver.as_str())]);
    let db = db::connect(&driver, &db_url)?;

    let mut exec = executor::SeedExecutor::new(log, db, tracking_table, reset);
    exec.execute(&plan)
}
