use crate::duration::{format_duration, parse_duration};
use crate::logging::Logger;
use crate::seed::db::Database;
use crate::seed::hash::compute_seed_set_hash;
use crate::seed::schema::{SeedPhase, SeedPlan, SeedSet, TableSeed, WaitForObject};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::{Duration, Instant};

pub struct SeedExecutor<'a> {
    log: &'a Logger,
    db: Box<dyn Database>,
    tracking_table: String,
    reset: bool,
    dry_run: bool,
    reconcile_all: bool,
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
            dry_run: false,
            reconcile_all: false,
            refs: HashMap::new(),
        }
    }

    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    pub fn with_reconcile_all(mut self, reconcile_all: bool) -> Self {
        self.reconcile_all = reconcile_all;
        self
    }

    pub fn execute(&mut self, plan: &SeedPlan) -> Result<(), String> {
        self.log.info("starting seed execution", &[]);
        self.db.ensure_tracking_table(&self.tracking_table)?;
        self.db.migrate_tracking_table(&self.tracking_table)?;
        self.db.ensure_row_tracking_table(&self.tracking_table)?;

        self.execute_phases(plan)?;

        self.log.info("seed execution completed", &[]);
        Ok(())
    }

    fn execute_phases(&mut self, plan: &SeedPlan) -> Result<(), String> {
        let mut phases: Vec<&SeedPhase> = plan.phases.iter().collect();
        phases.sort_by_key(|p| p.order);
        for phase in &phases {
            self.execute_phase(phase)?;
        }
        Ok(())
    }

    fn execute_phase(&mut self, phase: &SeedPhase) -> Result<(), String> {
        self.log
            .info("executing phase", &[("phase", phase.name.as_str())]);

        if phase.create_if_missing {
            if !phase.database.is_empty() {
                self.log.info(
                    "creating database if missing",
                    &[("database", phase.database.as_str())],
                );
                self.db.create_database(&phase.database)?;
            }
            if !phase.schema.is_empty() {
                self.log.info(
                    "creating schema if missing",
                    &[("schema", phase.schema.as_str())],
                );
                self.db.create_schema(&phase.schema)?;
            }
        }

        let phase_timeout =
            parse_duration(&phase.timeout).map_err(|e| format!("invalid phase timeout: {}", e))?;
        for wf in &phase.wait_for {
            self.wait_for_object(wf, &phase_timeout)?;
        }

        let mut seed_sets: Vec<&SeedSet> = phase.seed_sets.iter().collect();
        seed_sets.sort_by_key(|s| s.order);

        if self.reset {
            for ss in seed_sets.iter().rev() {
                self.reset_seed_set(ss)?;
            }
        }

        for ss in &seed_sets {
            self.execute_seed_set(ss)?;
        }

        self.log
            .info("phase completed", &[("phase", phase.name.as_str())]);
        Ok(())
    }

    fn wait_for_object(
        &mut self,
        wf: &WaitForObject,
        phase_timeout: &Duration,
    ) -> Result<(), String> {
        let timeout_dur = match &wf.timeout {
            Some(t) => parse_duration(t).map_err(|e| format!("invalid wait_for timeout: {}", e))?,
            None => *phase_timeout,
        };
        let timeout_str = format_duration(timeout_dur);
        let deadline = Instant::now() + timeout_dur;
        let poll_interval = Duration::from_millis(500);

        self.log.info(
            "waiting for object",
            &[
                ("type", wf.obj_type.as_str()),
                ("name", wf.name.as_str()),
                ("timeout", &timeout_str),
            ],
        );

        loop {
            match self.db.object_exists(&wf.obj_type, &wf.name) {
                Ok(true) => {
                    self.log.info(
                        "object found",
                        &[("type", wf.obj_type.as_str()), ("name", wf.name.as_str())],
                    );
                    return Ok(());
                }
                Ok(false) => {}
                Err(e) => {
                    return Err(format!(
                        "error checking {} '{}' on {} driver: {}",
                        wf.obj_type,
                        wf.name,
                        self.db.driver_name(),
                        e
                    ));
                }
            }

            if Instant::now() >= deadline {
                return Err(format!(
                    "timeout after {} waiting for {} '{}'",
                    timeout_str, wf.obj_type, wf.name
                ));
            }

            std::thread::sleep(poll_interval);
        }
    }

    fn reset_seed_set(&mut self, ss: &SeedSet) -> Result<(), String> {
        let name = &ss.name;
        let tt = self.tracking_table.clone();
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
        self.db.delete_all_tracked_rows(&tt, name)?;
        self.db.remove_seed_mark(&self.tracking_table, name)?;
        Ok(())
    }

    fn execute_seed_set(&mut self, ss: &SeedSet) -> Result<(), String> {
        let name = &ss.name;
        let is_reconcile = ss.is_reconcile() || self.reconcile_all;
        self.log.info(
            "processing seed set",
            &[
                ("seed_set", name),
                ("mode", if is_reconcile { "reconcile" } else { "once" }),
            ],
        );

        if is_reconcile {
            // Guard: reconcile requires unique_key on every table.
            // Schema validation catches this for mode: reconcile, but --reconcile-all
            // can force reconcile on mode: once seed sets that lack unique_key.
            for ts in &ss.tables {
                if ts.unique_key.is_empty() {
                    return Err(format!(
                        "cannot reconcile seed set '{}': table '{}' has no unique_key (required for reconcile mode)",
                        name, ts.table
                    ));
                }
            }
            return self.reconcile_seed_set(ss);
        }

        if self.db.is_seed_applied(&self.tracking_table, name)? {
            self.log
                .info("seed set already applied, skipping", &[("seed_set", name)]);
            return Ok(());
        }

        if self.dry_run {
            self.log.info(
                "dry-run: seed set would be applied (new)",
                &[("seed_set", name)],
            );
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

            if !ts.unique_key.is_empty()
                && self.db.row_exists(table, &unique_columns, &unique_values)?
            {
                self.log.info(
                    "row already exists, skipping",
                    &[("table", table.as_str()), ("row", &(idx + 1).to_string())],
                );
                continue;
            }

            let auto_id_col = ts.auto_id.as_ref().map(|a| a.column.as_str());
            let generated_id = self.db.insert_row(table, &columns, &values, auto_id_col)?;

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

    // --- Reconciliation ---

    fn reconcile_seed_set(&mut self, ss: &SeedSet) -> Result<(), String> {
        let name = &ss.name;

        // Compute hash of current spec (resolve env vars, keep @ref: as literals)
        let current_hash = compute_seed_set_hash(ss, &|val| self.resolve_value(val))?;

        // Check stored hash for quick skip.
        // Only skip if the seed set has no @ref: expressions, because
        // compute_seed_set_hash treats @ref: values as literals. Resolved
        // reference targets can change without affecting the hash (e.g.,
        // upstream auto_id row deleted/reinserted), and skipping could leave
        // stale foreign keys.
        let stored_hash = self.db.get_seed_hash(&self.tracking_table, name)?;
        let has_refs = ss.tables.iter().any(|ts| {
            ts.rows.iter().any(|row| {
                row.values()
                    .any(|v| v.as_str().map(|s| s.starts_with("@ref:")).unwrap_or(false))
            })
        });
        if !has_refs && stored_hash.as_deref() == Some(current_hash.as_str()) {
            self.log.info(
                "seed set unchanged (hash match), skipping",
                &[("seed_set", name)],
            );
            // Still need to populate refs for downstream seed sets
            self.populate_refs_from_db(ss)?;
            return Ok(());
        }

        if self.dry_run {
            self.log.info(
                "dry-run: seed set has changes, would reconcile",
                &[("seed_set", name)],
            );
            self.dry_run_reconcile_tables(ss)?;
            return Ok(());
        }

        self.log.info("reconciling seed set", &[("seed_set", name)]);

        self.db.begin_transaction()?;
        let result = self.reconcile_tables(ss, &current_hash);
        match result {
            Ok(()) => {
                let tt = self.tracking_table.clone();
                self.db.update_seed_entry(&tt, name, &current_hash)?;
                self.db.commit_transaction()?;
                self.log
                    .info("seed set reconciled successfully", &[("seed_set", name)]);
                Ok(())
            }
            Err(e) => {
                self.db.rollback_transaction()?;
                Err(format!("reconciling seed set '{}' failed: {}", name, e))
            }
        }
    }

    fn reconcile_tables(&mut self, ss: &SeedSet, _hash: &str) -> Result<(), String> {
        let mut tables: Vec<&TableSeed> = ss.tables.iter().collect();
        tables.sort_by_key(|t| t.order);

        for ts in &tables {
            self.reconcile_table(ss, ts)?;
        }
        Ok(())
    }

    fn reconcile_table(&mut self, ss: &SeedSet, ts: &TableSeed) -> Result<(), String> {
        let table = &ts.table;
        let tt = self.tracking_table.clone();
        let ss_name = ss.name.clone();

        self.log.info(
            "reconciling table",
            &[
                ("table", table.as_str()),
                ("rows", &ts.rows.len().to_string()),
            ],
        );

        // Get currently tracked rows for this seed_set + table
        let tracked = self.db.get_tracked_rows(&tt, &ss_name, table)?;
        let tracked_keys: HashSet<String> = tracked.iter().map(|(k, _)| k.clone()).collect();
        let tracked_values: HashMap<String, String> = tracked.into_iter().collect();

        let mut seen_keys = HashSet::new();

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

            // Build canonical row_key JSON (sorted by unique key column name)
            let row_key = build_row_key(&ts.unique_key, &unique_columns, &unique_values);
            // Build row_values JSON (excluding ignored columns for comparison)
            let row_values_json = build_row_values_excluding(&columns, &values, &ts.ignore_columns);

            seen_keys.insert(row_key.clone());

            let existing_values = tracked_values.get(&row_key);

            if let Some(stored_vals) = existing_values {
                // Row exists in tracking — check if values changed
                if stored_vals == &row_values_json {
                    // No change — populate refs if needed
                    self.populate_row_refs(ts, &ref_name, &columns, &values)?;
                    self.log.info(
                        "row unchanged, skipping",
                        &[("table", table.as_str()), ("row", &(idx + 1).to_string())],
                    );
                    continue;
                }

                // Values differ — UPDATE (exclude key columns and ignored columns)
                let non_key_columns: Vec<String> = columns
                    .iter()
                    .filter(|c| !ts.unique_key.contains(c) && !ts.ignore_columns.contains(c))
                    .cloned()
                    .collect();
                let non_key_values: Vec<String> = columns
                    .iter()
                    .zip(values.iter())
                    .filter(|(c, _)| !ts.unique_key.contains(c) && !ts.ignore_columns.contains(c))
                    .map(|(_, v)| v.clone())
                    .collect();

                if !non_key_columns.is_empty() {
                    self.db.update_row(
                        table,
                        &non_key_columns,
                        &non_key_values,
                        &unique_columns,
                        &unique_values,
                    )?;
                }

                self.db
                    .store_tracked_row(&tt, &ss_name, table, &row_key, &row_values_json)?;
                self.populate_row_refs(ts, &ref_name, &columns, &values)?;
                self.log.info(
                    "updated row",
                    &[("table", table.as_str()), ("row", &(idx + 1).to_string())],
                );
            } else {
                // New row — INSERT
                let auto_id_col = ts.auto_id.as_ref().map(|a| a.column.as_str());
                let generated_id = self.db.insert_row(table, &columns, &values, auto_id_col)?;

                if let Some(ref_key) = &ref_name {
                    let mut ref_map = HashMap::new();
                    for (i, col) in columns.iter().enumerate() {
                        ref_map.insert(col.clone(), values[i].clone());
                    }
                    if let (Some(ref auto_id), Some(id)) = (&ts.auto_id, generated_id) {
                        ref_map.insert(auto_id.column.clone(), id.to_string());
                    }
                    self.refs.insert(ref_key.clone(), ref_map);
                }

                self.db
                    .store_tracked_row(&tt, &ss_name, table, &row_key, &row_values_json)?;
                self.log.info(
                    "inserted row",
                    &[("table", table.as_str()), ("row", &(idx + 1).to_string())],
                );
            }
        }

        // Delete orphaned rows (in tracking but not in current spec)
        let orphaned_keys: Vec<String> = tracked_keys.difference(&seen_keys).cloned().collect();

        for orphan_key in &orphaned_keys {
            // Parse the row_key JSON to get column names + values
            let key_map: BTreeMap<String, String> = serde_json::from_str(orphan_key)
                .map_err(|e| format!("parsing orphan row key: {}", e))?;
            let key_cols: Vec<String> = key_map.keys().cloned().collect();
            let key_vals: Vec<String> = key_map.values().cloned().collect();

            self.db.delete_row_by_key(table, &key_cols, &key_vals)?;
            self.db
                .delete_tracked_row(&tt, &ss_name, table, orphan_key)?;
            self.log.info(
                "deleted orphaned row",
                &[("table", table.as_str()), ("row_key", orphan_key)],
            );
        }

        Ok(())
    }

    /// Populate refs from an existing (unchanged) row, fetching auto_id from DB if needed.
    fn populate_row_refs(
        &mut self,
        ts: &TableSeed,
        ref_name: &Option<String>,
        columns: &[String],
        values: &[String],
    ) -> Result<(), String> {
        if let Some(ref_key) = ref_name {
            let mut ref_map = HashMap::new();
            for (i, col) in columns.iter().enumerate() {
                ref_map.insert(col.clone(), values[i].clone());
            }

            // If there's an auto_id, fetch the actual ID from the DB
            if let Some(ref auto_id) = ts.auto_id {
                let unique_cols: Vec<String> = ts.unique_key.clone();
                let unique_vals: Vec<String> = ts
                    .unique_key
                    .iter()
                    .filter_map(|uk| {
                        columns
                            .iter()
                            .zip(values.iter())
                            .find(|(c, _)| *c == uk)
                            .map(|(_, v)| v.clone())
                    })
                    .collect();
                if let Some(row_vals) = self.db.get_row_columns(
                    &ts.table,
                    &unique_cols,
                    &unique_vals,
                    std::slice::from_ref(&auto_id.column),
                )? {
                    if let Some(id_val) = row_vals.first() {
                        ref_map.insert(auto_id.column.clone(), id_val.clone());
                    }
                }
            }
            self.refs.insert(ref_key.clone(), ref_map);
        }
        Ok(())
    }

    /// Populate refs for a skipped (hash-matched) seed set by reading from DB.
    fn populate_refs_from_db(&mut self, ss: &SeedSet) -> Result<(), String> {
        let mut tables: Vec<&TableSeed> = ss.tables.iter().collect();
        tables.sort_by_key(|t| t.order);

        for ts in &tables {
            for row in &ts.rows {
                let ref_name = row
                    .get("_ref")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                if ref_name.is_none() {
                    continue;
                }

                let mut columns = Vec::new();
                let mut values = Vec::new();

                for (key, val) in row {
                    if key == "_ref" {
                        continue;
                    }
                    let resolved = self.resolve_value(val)?;
                    columns.push(key.clone());
                    values.push(resolved);
                }

                self.populate_row_refs(ts, &ref_name, &columns, &values)?;
            }
        }
        Ok(())
    }

    /// Resolve a value for dry-run: treats `@ref:` as literals to avoid failures
    /// when refs haven't been populated (common with auto_id + refs in same seed set).
    fn resolve_value_dry_run(&self, val: &serde_yaml::Value) -> Result<String, String> {
        match val {
            serde_yaml::Value::String(s) if s.starts_with("@ref:") => Ok(s.clone()),
            _ => self.resolve_value(val),
        }
    }

    /// Dry-run: compute what reconciliation would do without modifying the DB.
    fn dry_run_reconcile_tables(&mut self, ss: &SeedSet) -> Result<(), String> {
        let mut tables: Vec<&TableSeed> = ss.tables.iter().collect();
        tables.sort_by_key(|t| t.order);
        let tt = self.tracking_table.clone();
        let ss_name = ss.name.clone();

        for ts in &tables {
            let tracked = self.db.get_tracked_rows(&tt, &ss_name, &ts.table)?;
            let tracked_keys: HashSet<String> = tracked.iter().map(|(k, _)| k.clone()).collect();
            let tracked_values: HashMap<String, String> = tracked.into_iter().collect();

            let mut seen_keys = HashSet::new();
            let mut inserts = 0u64;
            let mut updates = 0u64;

            for row in &ts.rows {
                let mut unique_columns = Vec::new();
                let mut unique_values = Vec::new();
                let mut columns = Vec::new();
                let mut values = Vec::new();

                for (key, val) in row {
                    if key == "_ref" {
                        continue;
                    }
                    let resolved = self.resolve_value_dry_run(val)?;
                    columns.push(key.clone());
                    values.push(resolved.clone());
                    if ts.unique_key.contains(key) {
                        unique_columns.push(key.clone());
                        unique_values.push(resolved);
                    }
                }

                let row_key = build_row_key(&ts.unique_key, &unique_columns, &unique_values);
                let row_values_json =
                    build_row_values_excluding(&columns, &values, &ts.ignore_columns);
                seen_keys.insert(row_key.clone());

                match tracked_values.get(&row_key) {
                    Some(stored) if stored == &row_values_json => {}
                    Some(_) => updates += 1,
                    None => inserts += 1,
                }
            }

            let deletes = tracked_keys.difference(&seen_keys).count() as u64;

            self.log.info(
                "dry-run: table reconciliation summary",
                &[
                    ("table", ts.table.as_str()),
                    ("inserts", &inserts.to_string()),
                    ("updates", &updates.to_string()),
                    ("deletes", &deletes.to_string()),
                ],
            );
        }

        // Populate refs from DB for downstream dry-run accuracy
        self.populate_refs_from_db(ss)?;
        Ok(())
    }
}

/// Build a canonical JSON key from unique key columns (sorted by column name).
fn build_row_key(unique_key_spec: &[String], columns: &[String], values: &[String]) -> String {
    let mut map = BTreeMap::new();
    for uk in unique_key_spec {
        if let Some(idx) = columns.iter().position(|c| c == uk) {
            map.insert(uk.clone(), values[idx].clone());
        }
    }
    serde_json::to_string(&map).unwrap_or_default()
}

/// Build a canonical JSON representation of row values, excluding specified columns.
/// Ignored columns are excluded from tracking so changes to them don't trigger reconciliation.
fn build_row_values_excluding(columns: &[String], values: &[String], exclude: &[String]) -> String {
    let mut map = BTreeMap::new();
    for (i, col) in columns.iter().enumerate() {
        if !exclude.contains(col) {
            map.insert(col.clone(), values[i].clone());
        }
    }
    serde_json::to_string(&map).unwrap_or_default()
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
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
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
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
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
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
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
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
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
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
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

        let emp_count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM employees", [], |r| r.get(0))
            .unwrap();
        assert_eq!(emp_count, 3, "expected 3 employees");

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
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: resetable
        tables:
          - table: departments
            unique_key: [name]
            rows:
              - name: Engineering
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let log = test_logger();

        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let db1 = SqliteDb::connect(db_path_str).unwrap();
        db1.conn
            .execute_batch("CREATE TABLE departments (id INTEGER PRIMARY KEY, name TEXT UNIQUE);")
            .unwrap();

        let mut exec1 = SeedExecutor::new(&log, Box::new(db1), "initium_seed".into(), false);
        exec1.execute(&plan).unwrap();

        let db_check = SqliteDb::connect(db_path_str).unwrap();
        let count: i64 = db_check
            .conn
            .query_row("SELECT COUNT(*) FROM departments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let db2 = SqliteDb::connect(db_path_str).unwrap();
        let mut exec2 = SeedExecutor::new(&log, Box::new(db2), "initium_seed".into(), true);
        exec2.execute(&plan).unwrap();

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
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
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
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: ordered
        order: 1
        tables:
          - table: departments
            rows:
              - name: Dept2
            order: 2
          - table: departments
            rows:
              - name: Dept1
            order: 1
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
            .prepare("SELECT name FROM departments ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(
            names,
            vec!["Dept1", "Dept2"],
            "Dept1 should be inserted before Dept2"
        );
    }

    #[test]
    fn test_empty_rows() {
        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
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
        assert_eq!(count, 0, "no rows should be inserted for empty rows list");
    }

    #[test]
    fn test_invalid_reference() {
        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: bad_ref
        tables:
          - table: departments
            rows:
              - name: "@ref:nonexistent.id"
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        setup_db_with_tables(&sqlite);

        let log = test_logger();
        let mut executor = SeedExecutor::new(&log, Box::new(sqlite), "initium_seed".into(), false);
        let result = executor.execute(&plan);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_numeric_and_boolean_values() {
        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: types
        tables:
          - table: config
            rows:
              - key: max_retries
                value: 5
              - key: debug
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
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM config", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);

        let rows: Vec<(String, String)> = db
            .conn
            .prepare("SELECT key, value FROM config ORDER BY key")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(rows[0], ("debug".to_string(), "true".to_string()));
        assert_eq!(rows[1], ("max_retries".to_string(), "5".to_string()));
    }

    #[test]
    fn test_basic_phase_execution() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        setup_db_with_tables(&sqlite);

        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: initial
        tables:
          - table: departments
            rows:
              - name: Engineering
              - name: Sales
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let log = test_logger();
        let mut executor = SeedExecutor::new(&log, Box::new(sqlite), "initium_seed".into(), false);
        executor.execute(&plan).unwrap();

        let db = SqliteDb::connect(db_path_str).unwrap();
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM departments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_multiple_phases() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        setup_db_with_tables(&sqlite);

        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    order: 1
    seed_sets:
      - name: depts
        tables:
          - table: departments
            auto_id:
              column: id
            rows:
              - _ref: dept_eng
                name: Engineering
  - name: phase2
    order: 2
    seed_sets:
      - name: employees
        tables:
          - table: employees
            rows:
              - name: Alice
                email: alice@example.com
                department_id: "@ref:dept_eng.id"
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let log = test_logger();
        let mut executor = SeedExecutor::new(&log, Box::new(sqlite), "initium_seed".into(), false);
        executor.execute(&plan).unwrap();

        let db = SqliteDb::connect(db_path_str).unwrap();
        let dept_count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM departments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(dept_count, 1);

        let emp_count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM employees", [], |r| r.get(0))
            .unwrap();
        assert_eq!(emp_count, 1);

        let dept_id: i64 = db
            .conn
            .query_row("SELECT id FROM departments", [], |r| r.get(0))
            .unwrap();
        let emp_dept_id: i64 = db
            .conn
            .query_row("SELECT department_id FROM employees", [], |r| r.get(0))
            .unwrap();
        assert_eq!(dept_id, emp_dept_id, "cross-phase references should work");
    }

    #[test]
    fn test_wait_for_existing_table() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        setup_db_with_tables(&sqlite);

        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: wait_and_seed
    timeout: 2
    wait_for:
      - type: table
        name: departments
    seed_sets:
      - name: data
        tables:
          - table: departments
            rows:
              - name: Engineering
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let log = test_logger();
        let mut executor = SeedExecutor::new(&log, Box::new(sqlite), "initium_seed".into(), false);
        executor.execute(&plan).unwrap();

        let db = SqliteDb::connect(db_path_str).unwrap();
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM departments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_wait_for_timeout() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();

        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: will_timeout
    timeout: 1
    wait_for:
      - type: table
        name: nonexistent_table
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let log = test_logger();
        let mut executor = SeedExecutor::new(&log, Box::new(sqlite), "initium_seed".into(), false);
        let result = executor.execute(&plan);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("timeout"),
            "error should mention timeout: {}",
            err
        );
        assert!(err.contains("nonexistent_table"));
    }

    #[test]
    fn test_wait_for_per_object_timeout() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();

        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: per_obj_timeout
    timeout: 60
    wait_for:
      - type: table
        name: missing_table
        timeout: 1
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let log = test_logger();
        let mut executor = SeedExecutor::new(&log, Box::new(sqlite), "initium_seed".into(), false);
        let result = executor.execute(&plan);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("timeout after 1s"));
    }

    #[test]
    fn test_create_if_missing_unsupported_on_sqlite() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();

        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: create_phase
    database: mydb
    create_if_missing: true
    seed_sets:
      - name: s
        tables:
          - table: t
            rows:
              - a: b
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let log = test_logger();
        let mut executor = SeedExecutor::new(&log, Box::new(sqlite), "initium_seed".into(), false);
        let result = executor.execute(&plan);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("does not support"),
            "should report unsupported: {}",
            err
        );
    }

    #[test]
    fn test_phase_without_seed_sets() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        setup_db_with_tables(&sqlite);

        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: wait_only
    timeout: 2
    wait_for:
      - type: table
        name: departments
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let log = test_logger();
        let mut executor = SeedExecutor::new(&log, Box::new(sqlite), "initium_seed".into(), false);
        executor.execute(&plan).unwrap();
    }

    #[test]
    fn test_wait_for_view() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        sqlite
            .conn
            .execute_batch(
                "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT);
                 CREATE VIEW items_view AS SELECT * FROM items;",
            )
            .unwrap();

        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: view_wait
    timeout: 2
    wait_for:
      - type: view
        name: items_view
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let log = test_logger();
        let mut executor = SeedExecutor::new(&log, Box::new(sqlite), "initium_seed".into(), false);
        executor.execute(&plan).unwrap();
    }

    #[test]
    fn test_wait_for_unsupported_type_on_sqlite() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();

        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: schema_wait
    timeout: 2
    wait_for:
      - type: schema
        name: public
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let log = test_logger();
        let mut executor = SeedExecutor::new(&log, Box::new(sqlite), "initium_seed".into(), false);
        let result = executor.execute(&plan);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("does not support"),
            "should report unsupported: {}",
            err
        );
    }

    // --- Reconciliation tests ---

    #[test]
    fn test_reconcile_initial_apply() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        setup_db_with_tables(&sqlite);

        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: reconcile_test
        mode: reconcile
        tables:
          - table: departments
            unique_key: [name]
            rows:
              - name: Engineering
              - name: Sales
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let log = test_logger();
        let mut executor = SeedExecutor::new(&log, Box::new(sqlite), "initium_seed".into(), false);
        executor.execute(&plan).unwrap();

        let db = SqliteDb::connect(db_path_str).unwrap();
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM departments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_reconcile_skip_unchanged() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        setup_db_with_tables(&sqlite);

        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: reconcile_idem
        mode: reconcile
        tables:
          - table: departments
            unique_key: [name]
            rows:
              - name: Engineering
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let log = test_logger();

        // First run
        let db1 = SqliteDb::connect(db_path_str).unwrap();
        let mut exec1 = SeedExecutor::new(&log, Box::new(db1), "initium_seed".into(), false);
        exec1.execute(&plan).unwrap();

        // Second run — should skip (hash match)
        let db2 = SqliteDb::connect(db_path_str).unwrap();
        let mut exec2 = SeedExecutor::new(&log, Box::new(db2), "initium_seed".into(), false);
        exec2.execute(&plan).unwrap();

        let db = SqliteDb::connect(db_path_str).unwrap();
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM departments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_reconcile_update_changed_row() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        sqlite
            .conn
            .execute_batch("CREATE TABLE config (key TEXT PRIMARY KEY, value TEXT);")
            .unwrap();

        let yaml1 = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: config
        mode: reconcile
        tables:
          - table: config
            unique_key: [key]
            rows:
              - key: app_name
                value: OldName
"#;
        let plan1 = SeedPlan::from_yaml(yaml1).unwrap();
        let log = test_logger();

        let db1 = SqliteDb::connect(db_path_str).unwrap();
        let mut exec1 = SeedExecutor::new(&log, Box::new(db1), "initium_seed".into(), false);
        exec1.execute(&plan1).unwrap();

        // Verify initial value
        let db_check = SqliteDb::connect(db_path_str).unwrap();
        let val: String = db_check
            .conn
            .query_row("SELECT value FROM config WHERE key = 'app_name'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(val, "OldName");

        // Run with changed value
        let yaml2 = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: config
        mode: reconcile
        tables:
          - table: config
            unique_key: [key]
            rows:
              - key: app_name
                value: NewName
"#;
        let plan2 = SeedPlan::from_yaml(yaml2).unwrap();
        let db2 = SqliteDb::connect(db_path_str).unwrap();
        let mut exec2 = SeedExecutor::new(&log, Box::new(db2), "initium_seed".into(), false);
        exec2.execute(&plan2).unwrap();

        let db_final = SqliteDb::connect(db_path_str).unwrap();
        let val: String = db_final
            .conn
            .query_row("SELECT value FROM config WHERE key = 'app_name'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(val, "NewName");
    }

    #[test]
    fn test_reconcile_add_new_row() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        setup_db_with_tables(&sqlite);

        let yaml1 = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: depts
        mode: reconcile
        tables:
          - table: departments
            unique_key: [name]
            rows:
              - name: Engineering
"#;
        let plan1 = SeedPlan::from_yaml(yaml1).unwrap();
        let log = test_logger();

        let db1 = SqliteDb::connect(db_path_str).unwrap();
        let mut exec1 = SeedExecutor::new(&log, Box::new(db1), "initium_seed".into(), false);
        exec1.execute(&plan1).unwrap();

        // Add a row
        let yaml2 = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: depts
        mode: reconcile
        tables:
          - table: departments
            unique_key: [name]
            rows:
              - name: Engineering
              - name: Sales
"#;
        let plan2 = SeedPlan::from_yaml(yaml2).unwrap();
        let db2 = SqliteDb::connect(db_path_str).unwrap();
        let mut exec2 = SeedExecutor::new(&log, Box::new(db2), "initium_seed".into(), false);
        exec2.execute(&plan2).unwrap();

        let db = SqliteDb::connect(db_path_str).unwrap();
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM departments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_reconcile_delete_removed_row() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        setup_db_with_tables(&sqlite);

        let yaml1 = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: depts
        mode: reconcile
        tables:
          - table: departments
            unique_key: [name]
            rows:
              - name: Engineering
              - name: Sales
"#;
        let plan1 = SeedPlan::from_yaml(yaml1).unwrap();
        let log = test_logger();

        let db1 = SqliteDb::connect(db_path_str).unwrap();
        let mut exec1 = SeedExecutor::new(&log, Box::new(db1), "initium_seed".into(), false);
        exec1.execute(&plan1).unwrap();

        let db_check = SqliteDb::connect(db_path_str).unwrap();
        let count: i64 = db_check
            .conn
            .query_row("SELECT COUNT(*) FROM departments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);

        // Remove Sales
        let yaml2 = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: depts
        mode: reconcile
        tables:
          - table: departments
            unique_key: [name]
            rows:
              - name: Engineering
"#;
        let plan2 = SeedPlan::from_yaml(yaml2).unwrap();
        let db2 = SqliteDb::connect(db_path_str).unwrap();
        let mut exec2 = SeedExecutor::new(&log, Box::new(db2), "initium_seed".into(), false);
        exec2.execute(&plan2).unwrap();

        let db_final = SqliteDb::connect(db_path_str).unwrap();
        let count: i64 = db_final
            .conn
            .query_row("SELECT COUNT(*) FROM departments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let name: String = db_final
            .conn
            .query_row("SELECT name FROM departments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "Engineering");
    }

    #[test]
    fn test_reconcile_with_auto_id_and_refs() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        setup_db_with_tables(&sqlite);

        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: org
        mode: reconcile
        tables:
          - table: departments
            order: 1
            unique_key: [name]
            auto_id:
              column: id
            rows:
              - _ref: dept_eng
                name: Engineering
          - table: employees
            order: 2
            unique_key: [email]
            rows:
              - name: Alice
                email: alice@example.com
                department_id: "@ref:dept_eng.id"
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let log = test_logger();

        // First apply
        let db1 = SqliteDb::connect(db_path_str).unwrap();
        let mut exec1 = SeedExecutor::new(&log, Box::new(db1), "initium_seed".into(), false);
        exec1.execute(&plan).unwrap();

        // Verify
        let db = SqliteDb::connect(db_path_str).unwrap();
        let dept_id: i64 = db
            .conn
            .query_row(
                "SELECT id FROM departments WHERE name = 'Engineering'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let emp_dept_id: i64 = db
            .conn
            .query_row(
                "SELECT department_id FROM employees WHERE email = 'alice@example.com'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(dept_id, emp_dept_id);

        // Run again — should be a no-op (hash match)
        let db2 = SqliteDb::connect(db_path_str).unwrap();
        let mut exec2 = SeedExecutor::new(&log, Box::new(db2), "initium_seed".into(), false);
        exec2.execute(&plan).unwrap();

        let db_final = SqliteDb::connect(db_path_str).unwrap();
        let count: i64 = db_final
            .conn
            .query_row("SELECT COUNT(*) FROM employees", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_reconcile_mode_requires_unique_key() {
        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: bad
        mode: reconcile
        tables:
          - table: departments
            rows:
              - name: Engineering
"#;
        let result = SeedPlan::from_yaml(yaml);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("unique_key"),
            "error should mention unique_key: {}",
            err
        );
    }

    #[test]
    fn test_reconcile_all_flag_overrides_mode() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        sqlite
            .conn
            .execute_batch("CREATE TABLE config (key TEXT PRIMARY KEY, value TEXT);")
            .unwrap();

        // mode: once, but we use reconcile_all
        let yaml1 = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: cfg
        tables:
          - table: config
            unique_key: [key]
            rows:
              - key: app
                value: v1
"#;
        let plan1 = SeedPlan::from_yaml(yaml1).unwrap();
        let log = test_logger();

        let db1 = SqliteDb::connect(db_path_str).unwrap();
        let mut exec1 = SeedExecutor::new(&log, Box::new(db1), "initium_seed".into(), false)
            .with_reconcile_all(true);
        exec1.execute(&plan1).unwrap();

        // Change value and run again with reconcile_all
        let yaml2 = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: cfg
        tables:
          - table: config
            unique_key: [key]
            rows:
              - key: app
                value: v2
"#;
        let plan2 = SeedPlan::from_yaml(yaml2).unwrap();
        let db2 = SqliteDb::connect(db_path_str).unwrap();
        let mut exec2 = SeedExecutor::new(&log, Box::new(db2), "initium_seed".into(), false)
            .with_reconcile_all(true);
        exec2.execute(&plan2).unwrap();

        let db = SqliteDb::connect(db_path_str).unwrap();
        let val: String = db
            .conn
            .query_row("SELECT value FROM config WHERE key = 'app'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(val, "v2");
    }

    #[test]
    fn test_dry_run_no_changes() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        setup_db_with_tables(&sqlite);

        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: dry
        mode: reconcile
        tables:
          - table: departments
            unique_key: [name]
            rows:
              - name: Engineering
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let log = test_logger();

        let db1 = SqliteDb::connect(db_path_str).unwrap();
        let mut exec =
            SeedExecutor::new(&log, Box::new(db1), "initium_seed".into(), false).with_dry_run(true);
        exec.execute(&plan).unwrap();

        // Should not have inserted anything
        let db = SqliteDb::connect(db_path_str).unwrap();
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM departments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0, "dry-run should not modify the database");
    }

    #[test]
    fn test_reconcile_cross_seed_set_refs() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        setup_db_with_tables(&sqlite);

        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    order: 1
    seed_sets:
      - name: depts
        mode: reconcile
        order: 1
        tables:
          - table: departments
            unique_key: [name]
            auto_id:
              column: id
            rows:
              - _ref: dept_eng
                name: Engineering
  - name: phase2
    order: 2
    seed_sets:
      - name: emps
        mode: reconcile
        order: 1
        tables:
          - table: employees
            unique_key: [email]
            rows:
              - name: Alice
                email: alice@example.com
                department_id: "@ref:dept_eng.id"
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let log = test_logger();

        // First run
        let db1 = SqliteDb::connect(db_path_str).unwrap();
        let mut exec1 = SeedExecutor::new(&log, Box::new(db1), "initium_seed".into(), false);
        exec1.execute(&plan).unwrap();

        let db = SqliteDb::connect(db_path_str).unwrap();
        let dept_id: i64 = db
            .conn
            .query_row("SELECT id FROM departments", [], |r| r.get(0))
            .unwrap();
        let emp_dept_id: i64 = db
            .conn
            .query_row("SELECT department_id FROM employees", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            dept_id, emp_dept_id,
            "cross-phase reconcile refs should work"
        );

        // Second run — both should skip (hash match), refs should still resolve
        let db2 = SqliteDb::connect(db_path_str).unwrap();
        let mut exec2 = SeedExecutor::new(&log, Box::new(db2), "initium_seed".into(), false);
        exec2.execute(&plan).unwrap();

        let db_final = SqliteDb::connect(db_path_str).unwrap();
        let emp_count: i64 = db_final
            .conn
            .query_row("SELECT COUNT(*) FROM employees", [], |r| r.get(0))
            .unwrap();
        assert_eq!(emp_count, 1, "second run should not duplicate employees");
    }

    #[test]
    fn test_reconcile_tracking_table_migration() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        // Create old-style tracking table (no content_hash column)
        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        sqlite
            .conn
            .execute_batch(
                "CREATE TABLE initium_seed (
                    seed_set TEXT PRIMARY KEY,
                    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
                CREATE TABLE departments (id INTEGER PRIMARY KEY, name TEXT UNIQUE);",
            )
            .unwrap();

        // Insert a legacy tracking entry
        sqlite
            .conn
            .execute(
                "INSERT INTO initium_seed (seed_set) VALUES ('legacy_set')",
                [],
            )
            .unwrap();

        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: new_set
        mode: reconcile
        tables:
          - table: departments
            unique_key: [name]
            rows:
              - name: Engineering
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let log = test_logger();

        let db2 = SqliteDb::connect(db_path_str).unwrap();
        let mut exec = SeedExecutor::new(&log, Box::new(db2), "initium_seed".into(), false);
        exec.execute(&plan).unwrap();

        // Verify migration worked: content_hash column exists
        let db = SqliteDb::connect(db_path_str).unwrap();
        let has_hash: bool = db
            .conn
            .prepare("PRAGMA table_info(initium_seed)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .any(|r| r.map(|n| n == "content_hash").unwrap_or(false));
        assert!(
            has_hash,
            "tracking table should have content_hash column after migration"
        );

        // Legacy entry should still be there
        let legacy: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM initium_seed WHERE seed_set = 'legacy_set'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(legacy, 1, "legacy entry should be preserved");
    }

    #[test]
    fn test_invalid_seed_mode() {
        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: bad_mode
        mode: invalid
        tables:
          - table: t
            rows:
              - a: b
"#;
        let result = SeedPlan::from_yaml(yaml);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid mode"));
    }

    #[test]
    fn test_reconcile_all_rejects_missing_unique_key() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        setup_db_with_tables(&sqlite);

        // mode: once with no unique_key + reconcile_all should error
        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: no_uk
        tables:
          - table: departments
            rows:
              - name: Engineering
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let log = test_logger();
        let mut exec = SeedExecutor::new(&log, Box::new(sqlite), "initium_seed".into(), false)
            .with_reconcile_all(true);
        let result = exec.execute(&plan);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no unique_key"));
    }

    #[test]
    fn test_reconcile_ignore_columns_not_compared() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        sqlite
            .conn
            .execute_batch(
                "CREATE TABLE config (key TEXT PRIMARY KEY, value TEXT, updated_at TEXT);",
            )
            .unwrap();

        // Initial apply with updated_at as ignored column
        let yaml1 = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: config
        mode: reconcile
        tables:
          - table: config
            unique_key: [key]
            ignore_columns: [updated_at]
            rows:
              - key: app_name
                value: MyApp
                updated_at: "2026-01-01"
"#;
        let plan1 = SeedPlan::from_yaml(yaml1).unwrap();
        let log = test_logger();

        let db1 = SqliteDb::connect(db_path_str).unwrap();
        let mut exec1 = SeedExecutor::new(&log, Box::new(db1), "initium_seed".into(), false);
        exec1.execute(&plan1).unwrap();

        // Verify initial values
        let db_check = SqliteDb::connect(db_path_str).unwrap();
        let val: String = db_check
            .conn
            .query_row(
                "SELECT updated_at FROM config WHERE key = 'app_name'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(val, "2026-01-01");

        // Change the ignored column value — should NOT trigger an update
        let yaml2 = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: config
        mode: reconcile
        tables:
          - table: config
            unique_key: [key]
            ignore_columns: [updated_at]
            rows:
              - key: app_name
                value: MyApp
                updated_at: "2026-12-31"
"#;
        let plan2 = SeedPlan::from_yaml(yaml2).unwrap();
        let db2 = SqliteDb::connect(db_path_str).unwrap();
        let mut exec2 = SeedExecutor::new(&log, Box::new(db2), "initium_seed".into(), false);
        exec2.execute(&plan2).unwrap();

        // updated_at should remain unchanged (ignored column not updated)
        let db_final = SqliteDb::connect(db_path_str).unwrap();
        let val: String = db_final
            .conn
            .query_row(
                "SELECT updated_at FROM config WHERE key = 'app_name'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(val, "2026-01-01");
    }

    #[test]
    fn test_reconcile_ignore_columns_still_inserted() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        sqlite
            .conn
            .execute_batch("CREATE TABLE items (name TEXT PRIMARY KEY, note TEXT);")
            .unwrap();

        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: items
        mode: reconcile
        tables:
          - table: items
            unique_key: [name]
            ignore_columns: [note]
            rows:
              - name: item1
                note: "initial note"
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let log = test_logger();

        let db1 = SqliteDb::connect(db_path_str).unwrap();
        let mut exec = SeedExecutor::new(&log, Box::new(db1), "initium_seed".into(), false);
        exec.execute(&plan).unwrap();

        // Ignored column should still be present on initial insert
        let db_check = SqliteDb::connect(db_path_str).unwrap();
        let note: String = db_check
            .conn
            .query_row("SELECT note FROM items WHERE name = 'item1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(note, "initial note");
    }

    #[test]
    fn test_reconcile_ignore_columns_non_ignored_still_updated() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let sqlite = SqliteDb::connect(db_path_str).unwrap();
        sqlite
            .conn
            .execute_batch(
                "CREATE TABLE config (key TEXT PRIMARY KEY, value TEXT, updated_at TEXT);",
            )
            .unwrap();

        // Initial
        let yaml1 = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: config
        mode: reconcile
        tables:
          - table: config
            unique_key: [key]
            ignore_columns: [updated_at]
            rows:
              - key: setting1
                value: old_value
                updated_at: "2026-01-01"
"#;
        let plan1 = SeedPlan::from_yaml(yaml1).unwrap();
        let log = test_logger();

        let db1 = SqliteDb::connect(db_path_str).unwrap();
        let mut exec1 = SeedExecutor::new(&log, Box::new(db1), "initium_seed".into(), false);
        exec1.execute(&plan1).unwrap();

        // Change value (non-ignored) — should trigger update, but NOT touch updated_at
        let yaml2 = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: config
        mode: reconcile
        tables:
          - table: config
            unique_key: [key]
            ignore_columns: [updated_at]
            rows:
              - key: setting1
                value: new_value
                updated_at: "2026-12-31"
"#;
        let plan2 = SeedPlan::from_yaml(yaml2).unwrap();
        let db2 = SqliteDb::connect(db_path_str).unwrap();
        let mut exec2 = SeedExecutor::new(&log, Box::new(db2), "initium_seed".into(), false);
        exec2.execute(&plan2).unwrap();

        let db_final = SqliteDb::connect(db_path_str).unwrap();
        let (value, updated_at): (String, String) = db_final
            .conn
            .query_row(
                "SELECT value, updated_at FROM config WHERE key = 'setting1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(value, "new_value"); // Non-ignored column updated
        assert_eq!(updated_at, "2026-01-01"); // Ignored column preserved
    }
}
