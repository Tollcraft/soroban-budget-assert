//! End-to-end tests for replay mode.
//!
//! These tests verify that the replay system can reproduce the entire
//! reporting pipeline using recorded fixtures without any network access
//! or external binaries (stellar CLI, curl).
//!
//! The test uses the same mock workspace as the live integration tests but
//! runs with --replay instead of --network/--source, so deploy, invoke,
//! and RPC calls are served from a pre-recorded fixture file instead of
//! the fake_bin scripts.

use assert_cmd::Command;
use predicates::str::contains;
use std::fs;
use std::path::{Path, PathBuf};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn mock_workspace_fixture() -> PathBuf {
    manifest_dir().join("tests/fixtures/mock_workspace")
}

/// Recursively copies `src` into `dst`, creating `dst` if needed.
fn copy_dir_all(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("failed to create destination directory");
    for entry in fs::read_dir(src).expect("failed to read source directory") {
        let entry = entry.expect("failed to read directory entry");
        let file_type = entry.file_type().expect("failed to read file type");
        let dst_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &dst_path);
        } else {
            fs::copy(entry.path(), &dst_path).expect("failed to copy fixture file");
        }
    }
}

/// Copies the mock workspace fixture into a fresh tempdir and returns it.
fn setup_mock_workspace() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("failed to create tempdir");
    copy_dir_all(&mock_workspace_fixture(), tmp.path());
    tmp
}

/// The replay fixture used by every test in this module.
///
/// Contains deterministic responses that match what the fake_bin scripts
/// produce: contract IDs for both mock contracts, invoke XDR, and
/// simulateTransaction responses that decode to instructions=1000000,
/// read_bytes=2048, write_bytes=4096.
const REPLAY_FIXTURE_JSON: &str = r#"{
  "fixture_version": 1,
  "entries": {
    "deploy:mock-contract-a": "CAMOCKCONTRACTIDAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    "deploy:mock-contract-b": "CAMOCKCONTRACTIDAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    "invoke:mock-contract-a:ping": "AAAAAgAAAAA=",
    "invoke:mock-contract-b:pong": "AAAAAgAAAAA=",
    "simulate:mock-contract-a:ping": {
      "jsonrpc": "2.0",
      "id": 1,
      "result": {
        "transactionData": "AAAAAAAAAAAAAAAAAA9CQAAACAAAABAAAAAAAAAAAAA=",
        "minResourceFee": "1000"
      }
    },
    "simulate:mock-contract-b:pong": {
      "jsonrpc": "2.0",
      "id": 1,
      "result": {
        "transactionData": "AAAAAAAAAAAAAAAAAA9CQAAACAAAABAAAAAAAAAAAAA=",
        "minResourceFee": "1000"
      }
    }
  }
}"#;

/// Builds a ready-to-run `Command` for the compiled `cargo-budget-report`
/// binary, with its cwd set to `dir`. No PATH manipulation is needed
/// because replay mode does not shell out to `stellar` or `curl`.
fn budget_report_cmd(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary should be built");
    cmd.current_dir(dir);
    cmd
}

#[test]
fn replay_produces_identical_table_report() {
    let workspace = setup_mock_workspace();
    let fixture_path = workspace.path().join("replay_fixtures.json");
    fs::write(&fixture_path, REPLAY_FIXTURE_JSON).expect("failed to write fixture file");

    let assert = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--replay",
            "--fixtures",
            fixture_path.to_str().unwrap(),
        ])
        .assert();

    let output = assert.success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify the same report structure as the live integration test.
    assert!(
        stdout.contains("WORKSPACE BUDGET REPORT"),
        "expected report header, got: {stdout}"
    );
    assert!(stdout.contains("mock-contract-a"), "got: {stdout}");
    assert!(stdout.contains("mock-contract-b"), "got: {stdout}");
    assert!(stdout.contains("CPU Instructions"), "got: {stdout}");
    assert!(stdout.contains("1,000,000 inst."), "got: {stdout}");
    assert!(stdout.contains("2,048 B"), "got: {stdout}");
    assert!(stdout.contains("4,096 B"), "got: {stdout}");
}

#[test]
fn replay_produces_identical_json_report() {
    let workspace = setup_mock_workspace();
    let fixture_path = workspace.path().join("replay_fixtures.json");
    fs::write(&fixture_path, REPLAY_FIXTURE_JSON).expect("failed to write fixture file");

    let assert = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--replay",
            "--fixtures",
            fixture_path.to_str().unwrap(),
            "--json",
        ])
        .assert();

    let output = assert.success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let reports: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    let reports = reports.as_array().expect("report should be a JSON array");

    let packages: std::collections::HashSet<&str> = reports
        .iter()
        .map(|r| r["package"].as_str().expect("package should be a string"))
        .collect();
    assert!(packages.contains("mock-contract-a"), "got: {reports:?}");
    assert!(packages.contains("mock-contract-b"), "got: {reports:?}");

    let cpu_entry = reports
        .iter()
        .find(|r| r["package"] == "mock-contract-a" && r["metric"] == "CPU Instructions")
        .expect("CPU Instructions entry for mock-contract-a should be present");
    assert_eq!(cpu_entry["value"], 1_000_000);

    let wasm_bytes_entry = reports
        .iter()
        .find(|r| r["package"] == "mock-contract-b" && r["metric"] == "WASM Bytes")
        .expect("WASM Bytes entry for mock-contract-b should be present");
    assert!(
        wasm_bytes_entry["value"].as_u64().unwrap_or(0) > 0,
        "got: {wasm_bytes_entry:?}"
    );
}

#[test]
fn replay_with_check_passes_when_limits_match() {
    let workspace = setup_mock_workspace();
    let fixture_path = workspace.path().join("replay_fixtures.json");
    fs::write(&fixture_path, REPLAY_FIXTURE_JSON).expect("failed to write fixture file");

    fs::write(
        workspace.path().join("budget.toml"),
        "[functions.ping]\n\
         cpu_limit = 5000000\n\
         read_limit = 5000\n\
         write_limit = 5000\n\
         \n\
         [functions.pong]\n\
         cpu_limit = 5000000\n\
         read_limit = 5000\n\
         write_limit = 5000\n",
    )
    .expect("failed to write budget.toml");

    let assert = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--replay",
            "--fixtures",
            fixture_path.to_str().unwrap(),
            "--check",
        ])
        .assert();

    let output = assert.success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("=== BUDGET CHECKS ==="), "got: {stdout}");
    assert!(
        stdout.contains("Summary: 6 check(s) passed, 0 failed"),
        "got: {stdout}"
    );
}

#[test]
fn replay_with_check_fails_when_limit_exceeded() {
    let workspace = setup_mock_workspace();
    let fixture_path = workspace.path().join("replay_fixtures.json");
    fs::write(&fixture_path, REPLAY_FIXTURE_JSON).expect("failed to write fixture file");

    fs::write(
        workspace.path().join("budget.toml"),
        "[functions.ping]\n\
         cpu_limit = 10\n\
         \n\
         [functions.pong]\n\
         cpu_limit = 5000000\n",
    )
    .expect("failed to write budget.toml");

    let mut cmd = budget_report_cmd(workspace.path());
    cmd.args([
        "budget-report",
        "--replay",
        "--fixtures",
        fixture_path.to_str().unwrap(),
        "--check",
    ]);

    cmd.assert()
        .failure()
        .stdout(contains("mock-contract-a::ping [CPU Instructions]"))
        .stdout(contains("FAIL"));
}

#[test]
fn replay_refuses_to_run_without_fixture() {
    let workspace = setup_mock_workspace();

    let assert = budget_report_cmd(workspace.path())
        .args(["budget-report", "--replay"])
        .assert();

    let output = assert.failure().get_output().clone();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Failed to read fixture file"),
        "expected fixture error, got: {stderr}"
    );
}

#[test]
fn replay_mode_does_not_require_mocked_binaries() {
    let workspace = setup_mock_workspace();
    let fixture_path = workspace.path().join("replay_fixtures.json");
    fs::write(&fixture_path, REPLAY_FIXTURE_JSON).expect("failed to write fixture file");

    // Run with the real PATH (no fake_bin prepended).  Replay mode never
    // invokes `stellar` or `curl`, so the real binaries (or their absence)
    // are irrelevant — the pipeline succeeds from recorded data alone.
    let assert = Command::cargo_bin("cargo-budget-report")
        .expect("binary should be built")
        .current_dir(workspace.path())
        .args([
            "budget-report",
            "--replay",
            "--fixtures",
            fixture_path.to_str().unwrap(),
        ])
        .assert();

    let output = assert.success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("WORKSPACE BUDGET REPORT"),
        "replay should work without fake_bin or stellar/curl on PATH, got: {stdout}"
    );
}
