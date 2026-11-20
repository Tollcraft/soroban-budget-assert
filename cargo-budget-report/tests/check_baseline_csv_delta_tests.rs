//! Integration tests for issue #1022:
//! Update the CSV reporting loop in `main.rs` to include new and stale functions with appropriate
//! status indicators.

use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
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
    copy_dir_all(
        &manifest_dir().join("tests/fixtures/mock_workspace"),
        tmp.path(),
    );
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

#[test]
fn test_check_baseline_csv_includes_new_and_stale_functions() {
    let workspace = setup_mock_workspace();

    // Write a baseline that includes:
    // 1. `mock-contract-a::ping` (matches current contract function -> compared)
    // 2. `mock-contract-legacy::removed_fn` (not present in current workspace -> stale)
    // Note: `mock-contract-b::pong` is in current workspace but not in baseline -> new
    fs::write(
        workspace.path().join("budget-baseline.toml"),
        r#"
["mock-contract-a::ping"]
cpu_instructions = 1000000
read_bytes = 2048
write_bytes = 4096

["mock-contract-legacy::removed_fn"]
cpu_instructions = 500000
read_bytes = 1024
write_bytes = 2048
"#,
    )
    .expect("failed to write baseline");

    let assert = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--check-baseline",
            "budget-baseline.toml",
            "--csv",
        ])
        .assert();

    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut lines = stdout.lines();
    assert_eq!(
        lines.next().unwrap_or(""),
        "package,function,metric,baseline,current,tolerance,verdict",
        "header row must be standard: {stdout}"
    );

    // Verify compared function row
    assert!(
        stdout.contains("mock-contract-a,ping,cpu_instructions,1000000,1000000,"),
        "expected compared function row for mock-contract-a::ping: {stdout}"
    );

    // Verify new function row
    assert!(
        stdout.contains("mock-contract-b,pong,,,,,new"),
        "expected new function row for mock-contract-b::pong: {stdout}"
    );

    // Verify stale function row
    assert!(
        stdout.contains("mock-contract-legacy,removed_fn,,,,,stale"),
        "expected stale function row for mock-contract-legacy::removed_fn: {stdout}"
    );
}
