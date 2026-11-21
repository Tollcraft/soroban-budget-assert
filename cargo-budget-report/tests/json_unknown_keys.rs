//! Unknown top-level report keys must fail validation (issue #1046).
//!
//! `validate_report_schema` used to ignore keys it did not recognise, so a
//! report that misspelled `snapshots` as `snapshot` validated cleanly and was
//! then parsed as having zero measurements. These tests run the real binary's
//! `--markdown --from` report-reading path, which is the same
//! `load_cost_reports` / `validate_report_schema` entry point CI uses.

use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};

fn budget_report_cmd(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary should be built");
    cmd.current_dir(dir);
    cmd
}

fn write_report(dir: &Path, name: &str, report: &serde_json::Value) -> PathBuf {
    let path = dir.join(name);
    fs::write(
        &path,
        serde_json::to_string(report).expect("fixture serialises"),
    )
    .expect("failed to write the report fixture");
    path
}

fn run_markdown_from(dir: &Path, path: &Path) -> std::process::Output {
    budget_report_cmd(dir)
        .args(["budget-report", "--markdown", "--from"])
        .arg(path)
        .assert()
        .get_output()
        .clone()
}

fn assert_rejected_with_key(stdout: &std::process::Output, key: &str) {
    assert_eq!(
        stdout.status.code(),
        Some(3),
        "an unknown top-level key is a config error\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&stdout.stdout),
        String::from_utf8_lossy(&stdout.stderr),
    );
    let stderr = String::from_utf8_lossy(&stdout.stderr);
    assert!(
        stderr.contains(&format!("unknown top-level key `{key}`")),
        "the error must name the offending key `{key}`:\n{stderr}"
    );
    assert!(
        stderr.contains("allowed keys are"),
        "the error should list the allowed keys:\n{stderr}"
    );
}

#[test]
fn misspelled_snapshots_key_is_rejected() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let path = write_report(
        dir.path(),
        "typo.json",
        &serde_json::json!({
            "schema_version": 1,
            "snapshot": []
        }),
    );

    let output = run_markdown_from(dir.path(), &path);
    assert_rejected_with_key(&output, "snapshot");
}

#[test]
fn unknown_key_alongside_a_valid_snapshots_array_is_rejected() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let path = write_report(
        dir.path(),
        "extra.json",
        &serde_json::json!({
            "schema_version": 1,
            "snapshots": [
                {"package": "pkg", "function": "fn", "metric": "CPU Instructions", "value": 1}
            ],
            "generted_at": "2026-11-21T04:27:00Z"
        }),
    );

    let output = run_markdown_from(dir.path(), &path);
    assert_rejected_with_key(&output, "generted_at");
}

#[test]
fn generation_metadata_keys_are_accepted() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let path = write_report(
        dir.path(),
        "envelope.json",
        &serde_json::json!({
            "schema_version": 1,
            "generated_at": "2026-11-21T04:27:00Z",
            "generator_version": "0.1.0",
            "snapshots": [
                {"package": "pkg", "function": "fn", "metric": "CPU Instructions", "value": 1}
            ]
        }),
    );

    let output = run_markdown_from(dir.path(), &path);
    assert!(
        output.status.success(),
        "metadata keys are part of the envelope and must validate\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("| pkg | fn | CPU Instructions | 1 |"),
        "{stdout}"
    );
}

#[test]
fn fixture_annotation_block_is_accepted() {
    // Checked-in report fixtures carry a free-form `_metadata` block; rejecting
    // it would break the repository's own sample reports.
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let path = write_report(
        dir.path(),
        "annotated.json",
        &serde_json::json!({
            "_metadata": {"description": "sample", "build_profile": "release"},
            "schema_version": 1,
            "snapshots": [
                {"package": "pkg", "function": "fn", "metric": "CPU Instructions", "value": 3}
            ]
        }),
    );

    let output = run_markdown_from(dir.path(), &path);
    assert!(
        output.status.success(),
        "the `_metadata` annotation block must stay valid\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("| pkg | fn | CPU Instructions | 3 |"),
        "{stdout}"
    );
}

#[test]
fn bare_array_reports_still_validate() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let path = write_report(
        dir.path(),
        "bare.json",
        &serde_json::json!([
            {"package": "pkg", "function": "fn", "metric": "CPU Instructions", "value": 2}
        ]),
    );

    let output = run_markdown_from(dir.path(), &path);
    assert!(
        output.status.success(),
        "historical bare arrays must keep validating\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("| pkg | fn | CPU Instructions | 2 |"),
        "{stdout}"
    );
}
