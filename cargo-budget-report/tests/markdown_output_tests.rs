//! Output-shape tests for `--markdown` (issues #849 and #850).
//!
//! * #850 — the post-measurement path used to print the GFM table and then the
//!   whole report as pretty JSON on the same stream, while the `--from` path
//!   printed the table alone. These tests pin that `--markdown` now emits
//!   exactly one document — the table — in both modes.
//! * #849 — when the report carries `--check` metadata (`limit`/`pass`) the
//!   table gains a `Limit` and a `Status` column, and a failed-simulation stub
//!   renders as `N/A (simulation failed)` rather than `N/A (testnet required)`.
//!   The `--from` path is exactly the one the CI step summary uses, so this
//!   exercises the real consumer.

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
        &manifest_dir().join("tests/fixtures").join("mock_workspace"),
        tmp.path(),
    );
    tmp
}

fn mocked_path() -> String {
    let real_path = std::env::var("PATH").unwrap_or_default();
    format!("{}:{real_path}", fake_bin_dir().display())
}

fn budget_report_cmd(dir: &Path, mocked: bool) -> Command {
    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary should be built");
    cmd.current_dir(dir);
    if mocked {
        cmd.env("PATH", mocked_path());
    }
    cmd
}

/// A document that is only the markdown table: every non-blank line is the
/// heading, a table row, the horizontal rule, or the footer, and no
/// JSON value appears anywhere.
fn assert_is_markdown_only(stdout: &str) {
    assert!(
        stdout.contains("| Package | Function | Metric | Value |"),
        "expected the markdown header row, got:\n{stdout}"
    );
    for line in stdout.lines().filter(|l| !l.trim().is_empty()) {
        let expected = line.starts_with('#')
            || line.starts_with('|')
            || line.starts_with('_')
            || line == "---";
        assert!(
            expected,
            "unexpected line in a --markdown document: {line:?}\n--- full stdout ---\n{stdout}"
        );
    }
    assert!(
        !stdout.contains("\"package\""),
        "the raw JSON report must not be appended to --markdown output:\n{stdout}"
    );
    assert!(
        !stdout.contains("[\n"),
        "no JSON array dump may follow the markdown table:\n{stdout}"
    );
}

/// #850: a live (mocked) run must emit the table and nothing else on stdout.
#[test]
fn markdown_live_run_emits_only_the_table() {
    let workspace = setup_mock_workspace();

    let output = budget_report_cmd(workspace.path(), true)
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--markdown",
        ])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();

    assert_is_markdown_only(&stdout);
}

/// #850: `--markdown --json` has a defined meaning — `--json` wins and only
/// JSON is printed, never a table followed by a JSON dump.
#[test]
fn markdown_and_json_together_emit_json_only() {
    let workspace = setup_mock_workspace();

    let output = budget_report_cmd(workspace.path(), true)
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--markdown",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();

    assert!(
        !stdout.contains("| Package | Function | Metric | Value |"),
        "with --json present the table must not be printed:\n{stdout}"
    );
    serde_json::from_str::<serde_json::Value>(&stdout)
        .expect("--markdown --json must emit a single JSON document");
}

/// #849 + #850: the `--from` path the CI step summary uses renders the check
/// columns and a distinct failed-stub cell, and emits no JSON.
#[test]
fn markdown_from_file_renders_check_columns_and_failed_stub() {
    let workspace = tempfile::tempdir().expect("failed to create tempdir");
    let report = serde_json::json!([
        {
            "package": "mock-contract-a",
            "function": "get_value",
            "metric": "CPU Instructions",
            "value": 1_234_567,
            "limit": 5_000_000,
            "pass": true
        },
        {
            "package": "mock-contract-a",
            "function": "broken_sim",
            "metric": "CPU Instructions",
            "value": null,
            "limit": 5_000_000,
            "pass": false
        }
    ]);
    fs::write(
        workspace.path().join("current_report.json"),
        serde_json::to_string(&report).expect("fixture serialises"),
    )
    .expect("failed to write the report fixture");

    let output = budget_report_cmd(workspace.path(), false)
        .args([
            "budget-report",
            "--markdown",
            "--from",
            "current_report.json",
        ])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();

    assert!(
        stdout.contains("| Package | Function | Metric | Value | Limit | Status |"),
        "check metadata must add Limit/Status columns:\n{stdout}"
    );
    assert!(
        stdout.contains(
            "| mock-contract-a | get_value | CPU Instructions | 1,234,567 | 5,000,000 | PASS |"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "| mock-contract-a | broken_sim | CPU Instructions | N/A (simulation failed) | 5,000,000 | FAIL |"
        ),
        "a failed-check stub must not render as a missing testnet measurement:\n{stdout}"
    );
    assert!(!stdout.contains("N/A (testnet required)"), "{stdout}");
    assert!(
        !stdout.contains("\"package\""),
        "the --from path must not append a JSON dump either:\n{stdout}"
    );
}

/// #1105: assert that `NOT_MEASURED` ("N/A (testnet required)") is emitted
/// only when a metric is unmeasured locally (normal mode without check metadata),
/// and `NOT_MEASURED_CHECK` ("N/A (simulation failed)") is emitted only when
/// check failure metadata is present.
#[test]
fn markdown_renders_distinct_sentinels_for_unmeasured_and_failed_checks() {
    let workspace = tempfile::tempdir().expect("failed to create tempdir");

    // 1. Normal mode (no check metadata): unmeasured metric renders as "N/A (testnet required)"
    let normal_report = serde_json::json!([
        {
            "package": "mock-contract-a",
            "function": "get_value",
            "metric": "Read Bytes",
            "value": null,
            "limit": null,
            "pass": null
        }
    ]);
    let normal_file = workspace.path().join("normal_report.json");
    fs::write(
        &normal_file,
        serde_json::to_string(&normal_report).expect("fixture serialises"),
    )
    .expect("failed to write normal report fixture");

    let output_normal = budget_report_cmd(workspace.path(), false)
        .args([
            "budget-report",
            "--markdown",
            "--from",
            "normal_report.json",
        ])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout_normal = String::from_utf8_lossy(&output_normal.stdout).into_owned();

    // Verify 4-column header in normal mode (no check columns)
    assert!(
        stdout_normal.contains("| Package | Function | Metric | Value |"),
        "normal report must use 4-column header:\n{stdout_normal}"
    );
    assert!(
        !stdout_normal.contains("| Limit | Status |"),
        "normal report must not have Limit/Status columns:\n{stdout_normal}"
    );
    // Sentinel must be "N/A (testnet required)", NEVER "N/A (simulation failed)"
    assert!(
        stdout_normal
            .contains("| mock-contract-a | get_value | Read Bytes | N/A (testnet required) |"),
        "unmeasured local metric must render as N/A (testnet required):\n{stdout_normal}"
    );
    assert!(
        !stdout_normal.contains("N/A (simulation failed)"),
        "normal report must not contain simulation failed sentinel:\n{stdout_normal}"
    );

    // 2. Check mode (with limit/pass metadata): failed simulation stub renders as "N/A (simulation failed)"
    let check_report = serde_json::json!([
        {
            "package": "mock-contract-a",
            "function": "broken_sim",
            "metric": "CPU Instructions",
            "value": null,
            "limit": 5_000_000,
            "pass": false
        }
    ]);
    let check_file = workspace.path().join("check_report.json");
    fs::write(
        &check_file,
        serde_json::to_string(&check_report).expect("fixture serialises"),
    )
    .expect("failed to write check report fixture");

    let output_check = budget_report_cmd(workspace.path(), false)
        .args(["budget-report", "--markdown", "--from", "check_report.json"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout_check = String::from_utf8_lossy(&output_check.stdout).into_owned();

    // Verify 6-column header in check mode
    assert!(
        stdout_check.contains("| Package | Function | Metric | Value | Limit | Status |"),
        "check report must use 6-column header with Limit/Status:\n{stdout_check}"
    );
    // Sentinel must be "N/A (simulation failed)", NEVER "N/A (testnet required)"
    assert!(
        stdout_check.contains(
            "| mock-contract-a | broken_sim | CPU Instructions | N/A (simulation failed) | 5,000,000 | FAIL |"
        ),
        "failed check stub must render as N/A (simulation failed):\n{stdout_check}"
    );
    assert!(
        !stdout_check.contains("N/A (testnet required)"),
        "check report must not contain testnet required sentinel for failed simulation:\n{stdout_check}"
    );
}
