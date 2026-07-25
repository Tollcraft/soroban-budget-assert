//! End-to-end integration tests for `cargo budget-report`.
//!
//! These run the real compiled binary (via `assert_cmd`) against the
//! isolated, deterministic mock workspace in `tests/fixtures/mock_workspace`
//! rather than against Tollcraft's own contracts, which made prior ad-hoc
//! testing brittle and circular.
//!
//! The mock workspace's two contracts are bare `no_std` WASM exports with no
//! dependencies (not real Soroban contracts), so `cargo build` for them is
//! near-instant. `cargo-budget-report` still shells out to the real
//! `stellar` CLI and `curl` to deploy/simulate, so both are replaced with
//! deterministic scripts in `tests/fixtures/fake_bin` (prepended to `PATH`
//! for the child process). This keeps the suite offline and reproducible:
//! no live network call, no funded/configured Stellar identity required.

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

fn fake_bin_dir() -> PathBuf {
    manifest_dir().join("tests/fixtures/fake_bin")
}

/// Recursively copies `src` into `dst`, creating `dst` if needed.
///
/// The mock workspace is copied into a fresh tempdir per test because
/// `cargo build` writes `Cargo.lock` and `target/` into its working
/// directory; running in place would leave build artifacts next to the
/// checked-in fixture.
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

/// `PATH` with the fake `stellar`/`curl` scripts prepended so the CLI under
/// test resolves them ahead of (or instead of) any real installation, while
/// still finding the real `cargo`/`rustc` used to build the mock contracts.
fn mocked_path() -> String {
    let real_path = std::env::var("PATH").unwrap_or_default();
    format!("{}:{}", fake_bin_dir().display(), real_path)
}

/// Builds a ready-to-run `Command` for the compiled `cargo-budget-report`
/// binary, with its cwd set to `dir` and `PATH` mocked as above.
fn budget_report_cmd(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary should be built");
    cmd.current_dir(dir).env("PATH", mocked_path());
    cmd
}

#[test]
fn discovers_mock_workspace_and_reports_cleanly() {
    let workspace = setup_mock_workspace();

    let assert = budget_report_cmd(workspace.path())
        .args(["budget-report", "--network", "local", "--source", "alice"])
        .assert();

    let output = assert.success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);

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
fn json_output_reports_both_mock_contracts() {
    let workspace = setup_mock_workspace();

    let assert = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
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
fn check_flag_passes_when_limits_are_generous() {
    let workspace = setup_mock_workspace();
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
            "--network",
            "local",
            "--source",
            "alice",
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
fn check_flag_fails_when_a_limit_is_exceeded() {
    let workspace = setup_mock_workspace();
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
        "--network",
        "local",
        "--source",
        "alice",
        "--check",
    ]);

    cmd.assert()
        .failure()
        .stdout(contains("mock-contract-a::ping [CPU Instructions]"))
        .stdout(contains("FAIL"));
}

// ── Quiet mode integration tests ────────────────────────────────────────

#[test]
fn quiet_mode_suppresses_stderr_progress_messages() {
    let workspace = setup_mock_workspace();

    let assert = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--quiet",
        ])
        .assert();

    let output = assert.success().get_output().clone();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // In quiet mode, the tool's own progress messages should be suppressed.
    // Child-process output (e.g. cargo build) may still appear on stderr.
    assert!(
        !stderr.contains("Discovering workspace members"),
        "stderr should not contain progress messages in quiet mode, got: {stderr:?}"
    );
    assert!(
        !stderr.contains("Simulating function"),
        "stderr should not contain simulation progress in quiet mode, got: {stderr:?}"
    );
    assert!(
        !stderr.contains("Contract deployed at:"),
        "stderr should not contain deploy status in quiet mode, got: {stderr:?}"
    );
    // stdout should still contain the budget report.
    assert!(
        stdout.contains("WORKSPACE BUDGET REPORT"),
        "stdout should contain the report header, got: {stdout}"
    );
    assert!(stdout.contains("mock-contract-a"), "got: {stdout}");
}

#[test]
fn quiet_mode_with_json_output_still_works() {
    let workspace = setup_mock_workspace();

    let assert = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--quiet",
            "--json",
        ])
        .assert();

    let output = assert.success().get_output().clone();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Tool's own progress messages should be suppressed.
    assert!(
        !stderr.contains("Discovering workspace members"),
        "stderr should not contain progress messages in quiet mode, got: {stderr:?}"
    );

    // stdout should be valid JSON.
    let reports: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    let reports = reports.as_array().expect("report should be a JSON array");
    assert!(!reports.is_empty(), "JSON report should not be empty");
}

#[test]
fn quiet_mode_with_check_still_enforces_limits() {
    let workspace = setup_mock_workspace();
    fs::write(
        workspace.path().join("budget.toml"),
        "[functions.ping]\n\
         cpu_limit = 10\n",
    )
    .expect("failed to write budget.toml");

    let assert = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--quiet",
            "--check",
        ])
        .assert();

    let output = assert.failure().get_output().clone();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Tool's own progress messages should be suppressed even on failure.
    assert!(
        !stderr.contains("Discovering workspace members"),
        "stderr should not contain progress messages in quiet mode, got: {stderr:?}"
    );

    // Check enforcement still works (FAIL appears in stdout).
    assert!(
        stdout.contains("FAIL"),
        "stdout should contain check failure, got: {stdout}"
    );
}
