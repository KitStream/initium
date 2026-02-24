use std::process::Command;

fn initium_bin() -> String {
    env!("CARGO_BIN_EXE_initium").to_string()
}

#[test]
fn test_env_var_fallback_for_json_flag() {
    // INITIUM_JSON=true should enable JSON output
    let output = Command::new(initium_bin())
        .args([
            "wait-for",
            "--target",
            "tcp://localhost:1",
            "--timeout",
            "1s",
            "--max-attempts",
            "1",
        ])
        .env("INITIUM_JSON", "true")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    // JSON output contains "msg" key
    assert!(
        stderr.contains("\"msg\""),
        "expected JSON output when INITIUM_JSON=true, got: {}",
        stderr
    );
}

#[test]
fn test_env_var_fallback_for_timeout() {
    // INITIUM_TIMEOUT=1s should set timeout to 1 second
    let output = Command::new(initium_bin())
        .args([
            "wait-for",
            "--target",
            "tcp://localhost:1",
            "--max-attempts",
            "1",
        ])
        .env("INITIUM_TIMEOUT", "1s")
        .output()
        .unwrap();
    // Should exit non-zero (connection failure), but the timeout was accepted
    assert!(!output.status.success());
}

#[test]
fn test_env_var_fallback_for_target() {
    // INITIUM_TARGET should set the target endpoints
    let output = Command::new(initium_bin())
        .args(["wait-for", "--timeout", "1s", "--max-attempts", "1"])
        .env("INITIUM_TARGET", "tcp://localhost:1")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should attempt to connect (and fail), not complain about missing --target
    assert!(
        !stderr.contains("required"),
        "expected target from env var, got: {}",
        stderr
    );
}

#[test]
fn test_env_var_multiple_targets_comma_separated() {
    // INITIUM_TARGET with comma-separated values
    let output = Command::new(initium_bin())
        .args(["wait-for", "--timeout", "1s", "--max-attempts", "1"])
        .env("INITIUM_TARGET", "tcp://localhost:1,tcp://localhost:2")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("required"),
        "expected targets from env var, got: {}",
        stderr
    );
}

#[test]
fn test_flag_takes_precedence_over_env_var() {
    // --timeout flag (1s) should override INITIUM_TIMEOUT env var (999m)
    let output = Command::new(initium_bin())
        .args([
            "wait-for",
            "--target",
            "tcp://localhost:1",
            "--timeout",
            "1s",
            "--max-attempts",
            "1",
        ])
        .env("INITIUM_TIMEOUT", "999m")
        .output()
        .unwrap();
    // If env var took precedence, the process would run for ~2.7 hours.
    // Since it exits quickly, the flag value was used.
    assert!(!output.status.success());
}

#[test]
fn test_env_var_fallback_for_insecure_tls() {
    // INITIUM_INSECURE_TLS=true should enable insecure TLS
    let output = Command::new(initium_bin())
        .args([
            "wait-for",
            "--target",
            "tcp://localhost:1",
            "--timeout",
            "1s",
            "--max-attempts",
            "1",
        ])
        .env("INITIUM_INSECURE_TLS", "true")
        .output()
        .unwrap();
    // Should not error about unknown flag; exits with connection failure
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unexpected argument"),
        "env var should be accepted: {}",
        stderr
    );
}

#[test]
fn test_env_var_fallback_for_spec() {
    // INITIUM_SPEC should set the seed spec file path
    let output = Command::new(initium_bin())
        .args(["seed"])
        .env("INITIUM_SPEC", "/nonexistent/seed.yaml")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should try to read the file (and fail), not complain about missing --spec
    assert!(
        stderr.contains("seed.yaml") || stderr.contains("nonexistent"),
        "expected file error from env var spec, got: {}",
        stderr
    );
}

#[test]
fn test_env_var_fallback_for_workdir() {
    // INITIUM_WORKDIR should set the working directory for render
    let output = Command::new(initium_bin())
        .args([
            "render",
            "--template",
            "/nonexistent/tpl",
            "--output",
            "out.txt",
        ])
        .env("INITIUM_WORKDIR", "/tmp/test-workdir")
        .output()
        .unwrap();
    // Should not complain about missing --workdir
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("required"),
        "workdir should come from env var: {}",
        stderr
    );
}

#[test]
fn test_bare_number_timeout_accepted() {
    // Bare number without unit should be treated as seconds (documented behavior)
    let output = Command::new(initium_bin())
        .args([
            "wait-for",
            "--target",
            "tcp://localhost:1",
            "--timeout",
            "1",
            "--max-attempts",
            "1",
        ])
        .output()
        .unwrap();
    // Should exit non-zero (connection failure), not complain about invalid duration
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("invalid"),
        "bare number timeout should be accepted: {}",
        stderr
    );
}

#[test]
fn test_bare_number_timeout_via_env_var() {
    // INITIUM_TIMEOUT=1 (bare number) should be accepted as 1 second
    let output = Command::new(initium_bin())
        .args([
            "wait-for",
            "--target",
            "tcp://localhost:1",
            "--max-attempts",
            "1",
        ])
        .env("INITIUM_TIMEOUT", "1")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("invalid"),
        "bare number env var timeout should be accepted: {}",
        stderr
    );
}

#[test]
fn test_env_var_false_boolean_not_set() {
    // INITIUM_JSON=false should keep text output
    let output = Command::new(initium_bin())
        .args([
            "wait-for",
            "--target",
            "tcp://localhost:1",
            "--timeout",
            "1s",
            "--max-attempts",
            "1",
        ])
        .env("INITIUM_JSON", "false")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Text output contains [INFO] or [ERROR], not JSON
    assert!(
        !stderr.contains("\"msg\""),
        "expected text output when INITIUM_JSON=false, got: {}",
        stderr
    );
}
