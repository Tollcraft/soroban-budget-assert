//! Process-level integration tests for `cargo-budget-report` exit codes.
//!
//! Issue #406: cargo-budget-report exits with distinct codes so CI can
//! branch on the outcome. These tests run the real compiled binary and
//! assert on the exact process exit code, not just success/failure.
//!
//! Exit code contract (from `cargo-budget-report/src/error.rs`):
//!   0 = success
//!   1 = generic failure
//!   3 = config error
//!   4 = budget exceeded
//!   5 = regression
//!   6 = network failure

use assert_cmd::Command;
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

fn setup_mock_workspace() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("failed to create tempdir");
    copy_dir_all(&mock_workspace_fixture(), tmp.path());
    tmp
}

fn mocked_path() -> String {
    let real_path = std::env::var("PATH").unwrap_or_default();
    format!("{}:{}", fake_bin_dir().display(), real_path)
}

fn budget_report_cmd(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary should be built");
    cmd.current_dir(dir).env("PATH", mocked_path());
    cmd
}

// ── Exit code constants (must match error.rs) ─────────────────────────
const EXIT_SUCCESS: i32 = 0;
const EXIT_CONFIG_ERROR: i32 = 3;
const EXIT_BUDGET_EXCEEDED: i32 = 4;

#[test]
fn exit_code_zero_on_success() {
    let workspace = setup_mock_workspace();

    // No budget.toml — the tool runs a plain report with no limits.
    let output = budget_report_cmd(workspace.path())
        .args(["budget-report", "--network", "local", "--source", "alice"])
        .assert()
        .success()
        .get_output()
        .clone();

    assert_eq!(
        output.status.code(),
        Some(EXIT_SUCCESS),
        "clean run should exit 0"
    );
}

#[test]
fn exit_code_config_error_on_malformed_budget_toml() {
    let workspace = setup_mock_workspace();

    // Write a TOML file that is syntactically invalid.
    fs::write(
        workspace.path().join("budget.toml"),
        "this is not valid toml {{{{",
    )
    .expect("failed to write budget.toml");

    let output = budget_report_cmd(workspace.path())
        .args(["budget-report", "--network", "local", "--source", "alice"])
        .assert()
        .failure()
        .get_output()
        .clone();

    assert_eq!(
        output.status.code(),
        Some(EXIT_CONFIG_ERROR),
        "malformed budget.toml should exit 3 (config error)"
    );
}

#[test]
fn exit_code_budget_exceeded_when_check_fails() {
    let workspace = setup_mock_workspace();

    // Set a CPU limit that the mock contract will exceed.
    fs::write(
        workspace.path().join("budget.toml"),
        "[functions.ping]\ncpu_limit = 10\n\n[functions.pong]\ncpu_limit = 5000000\n",
    )
    .expect("failed to write budget.toml");

    let output = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--check",
        ])
        .assert()
        .failure()
        .get_output()
        .clone();

    assert_eq!(
        output.status.code(),
        Some(EXIT_BUDGET_EXCEEDED),
        "budget limit breach should exit 4 (budget exceeded)"
    );
}

#[test]
fn exit_code_zero_when_check_passes() {
    let workspace = setup_mock_workspace();

    // Set generous limits that the mock contract will not breach.
    fs::write(
        workspace.path().join("budget.toml"),
        "[functions.ping]\ncpu_limit = 5000000\n\n[functions.pong]\ncpu_limit = 5000000\n",
    )
    .expect("failed to write budget.toml");

    let output = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--check",
        ])
        .assert()
        .success()
        .get_output()
        .clone();

    assert_eq!(
        output.status.code(),
        Some(EXIT_SUCCESS),
        "check with generous limits should exit 0"
    );
}
