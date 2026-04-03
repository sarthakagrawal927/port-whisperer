use std::process::Command;

fn ports_cmd(args: &[&str]) -> (String, String, bool) {
    let output = Command::new("cargo")
        .args(["run", "--release", "--"])
        .args(args)
        .output()
        .expect("Failed to run ports");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (stdout, stderr, output.status.success())
}

// ── Basic commands don't crash ──

#[test]
fn test_default_runs() {
    let (stdout, _, success) = ports_cmd(&[]);
    assert!(success);
    // Should show either a table or "No listening ports"
    assert!(stdout.contains("PORT") || stdout.contains("No listening ports"));
}

#[test]
fn test_all_flag() {
    let (stdout, _, success) = ports_cmd(&["--all"]);
    assert!(success);
    assert!(stdout.contains("PORT") || stdout.contains("No listening ports"));
}

#[test]
fn test_all_short_flag() {
    let (stdout, _, success) = ports_cmd(&["-a"]);
    assert!(success);
    assert!(stdout.contains("PORT") || stdout.contains("No listening ports"));
}

#[test]
fn test_help() {
    let (stdout, _, success) = ports_cmd(&["help"]);
    assert!(success);
    assert!(stdout.contains("USAGE"));
    assert!(stdout.contains("ports"));
}

#[test]
fn test_help_long_flag() {
    let (stdout, _, success) = ports_cmd(&["--help"]);
    assert!(success);
    assert!(stdout.contains("USAGE"));
}

#[test]
fn test_help_short_flag() {
    let (stdout, _, success) = ports_cmd(&["-h"]);
    assert!(success);
    assert!(stdout.contains("USAGE"));
}

// ── JSON output ──

#[test]
fn test_json_valid() {
    let (stdout, _, success) = ports_cmd(&["json"]);
    assert!(success);
    let parsed: Result<Vec<serde_json::Value>, _> = serde_json::from_str(&stdout);
    assert!(parsed.is_ok(), "JSON output is not valid: {}", stdout);
}

#[test]
fn test_json_structure() {
    let (stdout, _, success) = ports_cmd(&["json"]);
    assert!(success);
    let entries: Vec<serde_json::Value> = serde_json::from_str(&stdout).unwrap();
    for entry in &entries {
        assert!(entry.get("port").is_some(), "Missing 'port' field");
        assert!(entry.get("pid").is_some(), "Missing 'pid' field");
        assert!(entry.get("name").is_some(), "Missing 'name' field");
        assert!(entry.get("framework").is_some(), "Missing 'framework' field");
        assert!(entry.get("health").is_some(), "Missing 'health' field");
        // Health must be one of the valid values
        let health = entry["health"].as_str().unwrap();
        assert!(
            ["healthy", "orphaned", "zombie"].contains(&health),
            "Invalid health value: {}",
            health
        );
        // Port must be a valid u16
        let port = entry["port"].as_u64().unwrap();
        assert!(port > 0 && port <= 65535, "Invalid port: {}", port);
    }
}

#[test]
fn test_json_all_has_more_or_equal() {
    let (stdout_default, _, _) = ports_cmd(&["json"]);
    let (stdout_all, _, _) = ports_cmd(&["json", "--all"]);
    let default: Vec<serde_json::Value> = serde_json::from_str(&stdout_default).unwrap();
    let all: Vec<serde_json::Value> = serde_json::from_str(&stdout_all).unwrap();
    assert!(
        all.len() >= default.len(),
        "--all ({}) should have >= default ({}) entries",
        all.len(),
        default.len()
    );
}

// ── Port inspect ──

#[test]
fn test_inspect_nonexistent_port() {
    let (_, stderr, _) = ports_cmd(&["1"]);
    assert!(
        stderr.contains("No process found on port :1"),
        "Expected 'No process found' for port 1, got: {}",
        stderr
    );
}

#[test]
fn test_inspect_invalid_port_too_large() {
    let (_, stderr, _) = ports_cmd(&["99999"]);
    assert!(
        stderr.contains("Invalid port") || stderr.contains("must be 1-65535"),
        "Expected invalid port error for 99999, got: {}",
        stderr
    );
}

#[test]
fn test_inspect_zero_port() {
    // Port 0 is technically parseable as u16 but nothing listens on it
    let (_, stderr, _) = ports_cmd(&["0"]);
    assert!(
        stderr.contains("No process found"),
        "Expected no process on port 0, got: {}",
        stderr
    );
}

// ── Edge cases ──

#[test]
fn test_unknown_command() {
    let (_, stderr, _) = ports_cmd(&["foobar"]);
    assert!(
        stderr.contains("Unknown command"),
        "Expected unknown command error, got: {}",
        stderr
    );
}

#[test]
fn test_open_no_arg() {
    let (_, stderr, _) = ports_cmd(&["open"]);
    assert!(
        stderr.contains("Usage"),
        "Expected usage message, got: {}",
        stderr
    );
}

#[test]
fn test_free_no_arg() {
    let (_, stderr, _) = ports_cmd(&["free"]);
    assert!(
        stderr.contains("Usage"),
        "Expected usage message, got: {}",
        stderr
    );
}

#[test]
fn test_free_nonexistent_port() {
    let (_, stderr, _) = ports_cmd(&["free", "1"]);
    assert!(
        stderr.contains("Nothing on :1"),
        "Expected nothing-on-port error, got: {}",
        stderr
    );
}

// ── Process list ──

#[test]
fn test_ps_runs() {
    let (stdout, _, success) = ports_cmd(&["ps"]);
    assert!(success);
    assert!(stdout.contains("PID") || stdout.contains("No dev processes"));
}

#[test]
fn test_ps_all_runs() {
    let (stdout, _, success) = ports_cmd(&["ps", "--all"]);
    assert!(success);
    assert!(stdout.contains("PID") || stdout.contains("No dev processes"));
}

// ── Clean ──

#[test]
fn test_clean_runs() {
    // Just verify it doesn't crash — stdin is closed so it won't prompt
    let (stdout, _, success) = ports_cmd(&["clean"]);
    assert!(success);
    assert!(
        stdout.contains("Scanning") || stdout.contains("healthy"),
        "Clean should show scanning output, got: {}",
        stdout
    );
}

// ── Log ──

#[test]
fn test_log_runs() {
    let (stdout, _, success) = ports_cmd(&["log"]);
    assert!(success);
    // Either shows table or "No port history"
    assert!(
        stdout.contains("TIMESTAMP") || stdout.contains("No port history") || stdout.contains("PORT"),
        "Log should show history or empty message, got: {}",
        stdout
    );
}

#[test]
fn test_log_filter_nonexistent_port() {
    let (stdout, _, success) = ports_cmd(&["log", "11111"]);
    assert!(success);
    assert!(
        stdout.contains("No history for port :11111"),
        "Expected no history message, got: {}",
        stdout
    );
}
