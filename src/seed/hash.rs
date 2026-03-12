use crate::seed::schema::SeedSet;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Compute a deterministic SHA-256 hash of a seed set's content.
///
/// Values are resolved using the provided resolver function (expanding env vars
/// and templates), except `@ref:` expressions which are kept as literals to
/// avoid cascading false positives when auto-generated IDs shift.
///
/// The `_ref` key is excluded from the hash (it is a structural label, not data).
pub fn compute_seed_set_hash(
    ss: &SeedSet,
    resolver: &dyn Fn(&serde_yaml::Value) -> Result<String, String>,
) -> Result<String, String> {
    let mut hasher = Sha256::new();

    let mut tables: Vec<_> = ss.tables.iter().collect();
    tables.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.table.cmp(&b.table)));

    for ts in &tables {
        hasher.update(ts.table.as_bytes());
        hasher.update(b"\n");

        // Include unique_key in hash so changing it triggers reconciliation
        let uk_json = serde_json::to_string(&ts.unique_key)
            .map_err(|e| format!("serializing unique_key: {}", e))?;
        hasher.update(uk_json.as_bytes());
        hasher.update(b"\n");

        // Include auto_id config
        let auto_id_str = match &ts.auto_id {
            Some(a) => format!("{}:{}", a.column, a.id_type),
            None => String::new(),
        };
        hasher.update(auto_id_str.as_bytes());
        hasher.update(b"\n");

        for row in &ts.rows {
            // Sort keys for determinism (HashMap iteration order is random)
            let sorted: BTreeMap<_, _> = row.iter().collect();
            for (key, val) in &sorted {
                if key.as_str() == "_ref" {
                    continue;
                }
                // Ignored columns don't affect the hash — changes to them
                // won't trigger reconciliation.
                if ts.ignore_columns.contains(key) {
                    continue;
                }
                hasher.update(key.as_bytes());
                hasher.update(b"=");

                // Keep @ref: literals as-is; resolve everything else
                let val_str = match val.as_str() {
                    Some(s) if s.starts_with("@ref:") => s.to_string(),
                    _ => resolver(val)?,
                };
                hasher.update(val_str.as_bytes());
                hasher.update(b"\x00");
            }
            hasher.update(b"\n");
        }
    }

    let hash = hasher.finalize();
    Ok(hex_encode(&hash))
}

fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{:02x}", b);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seed::schema::SeedPlan;

    fn identity_resolver(val: &serde_yaml::Value) -> Result<String, String> {
        match val {
            serde_yaml::Value::String(s) => Ok(s.clone()),
            serde_yaml::Value::Number(n) => Ok(n.to_string()),
            serde_yaml::Value::Bool(b) => Ok(b.to_string()),
            serde_yaml::Value::Null => Ok(String::new()),
            _ => Ok(format!("{:?}", val)),
        }
    }

    #[test]
    fn test_deterministic_hash() {
        let yaml = r#"
database:
  driver: sqlite
  url: ":memory:"
phases:
  - name: phase1
    seed_sets:
      - name: test
        mode: reconcile
        tables:
          - table: users
            unique_key: [email]
            rows:
              - email: alice@example.com
                name: Alice
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let ss = &plan.phases[0].seed_sets[0];
        let h1 = compute_seed_set_hash(ss, &identity_resolver).unwrap();
        let h2 = compute_seed_set_hash(ss, &identity_resolver).unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA-256 hex
    }

    #[test]
    fn test_hash_changes_on_value_change() {
        let yaml1 = r#"
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
            unique_key: [k]
            rows:
              - k: a
                v: "1"
"#;
        let yaml2 = r#"
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
            unique_key: [k]
            rows:
              - k: a
                v: "2"
"#;
        let plan1 = SeedPlan::from_yaml(yaml1).unwrap();
        let plan2 = SeedPlan::from_yaml(yaml2).unwrap();
        let h1 = compute_seed_set_hash(&plan1.phases[0].seed_sets[0], &identity_resolver).unwrap();
        let h2 = compute_seed_set_hash(&plan2.phases[0].seed_sets[0], &identity_resolver).unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_hash_stable_with_ref_expressions() {
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
            unique_key: [name]
            rows:
              - name: Alice
                dept_id: "@ref:dept_eng.id"
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let ss = &plan.phases[0].seed_sets[0];
        let h1 = compute_seed_set_hash(ss, &identity_resolver).unwrap();
        let h2 = compute_seed_set_hash(ss, &identity_resolver).unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_changes_on_env_resolution() {
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
            unique_key: [k]
            rows:
              - k: a
                v: some_value
"#;
        let plan = SeedPlan::from_yaml(yaml).unwrap();
        let ss = &plan.phases[0].seed_sets[0];

        let h1 = compute_seed_set_hash(ss, &identity_resolver).unwrap();

        // Simulate different env resolution
        let different_resolver = |val: &serde_yaml::Value| -> Result<String, String> {
            match val {
                serde_yaml::Value::String(s) if s == "some_value" => Ok("different_value".into()),
                _ => identity_resolver(val),
            }
        };

        let h2 = compute_seed_set_hash(ss, &different_resolver).unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_hash_changes_on_row_added() {
        let yaml1 = r#"
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
            unique_key: [k]
            rows:
              - k: a
"#;
        let yaml2 = r#"
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
            unique_key: [k]
            rows:
              - k: a
              - k: b
"#;
        let plan1 = SeedPlan::from_yaml(yaml1).unwrap();
        let plan2 = SeedPlan::from_yaml(yaml2).unwrap();
        let h1 = compute_seed_set_hash(&plan1.phases[0].seed_sets[0], &identity_resolver).unwrap();
        let h2 = compute_seed_set_hash(&plan2.phases[0].seed_sets[0], &identity_resolver).unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_hash_ignores_ignored_columns() {
        let yaml1 = r#"
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
            unique_key: [k]
            ignore_columns: [note]
            rows:
              - k: a
                note: "version 1"
"#;
        let yaml2 = r#"
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
            unique_key: [k]
            ignore_columns: [note]
            rows:
              - k: a
                note: "version 2"
"#;
        let plan1 = SeedPlan::from_yaml(yaml1).unwrap();
        let plan2 = SeedPlan::from_yaml(yaml2).unwrap();
        let h1 = compute_seed_set_hash(&plan1.phases[0].seed_sets[0], &identity_resolver).unwrap();
        let h2 = compute_seed_set_hash(&plan2.phases[0].seed_sets[0], &identity_resolver).unwrap();
        assert_eq!(
            h1, h2,
            "hash should be identical when only ignored columns change"
        );
    }
}
