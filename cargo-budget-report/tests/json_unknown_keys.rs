//! Integration tests for unknown top-level key rejection (issue #1046).
//!
//! `validate_report_schema` must reject JSON reports that contain top-level
//! keys beyond the allowed set (`schema_version`, `snapshots`, `generated_at`,
//! `generator_version`), naming the bad key in the error message. This catches
//! typos like `snapshot` being silently accepted as an empty report.

use assert_cmd::Command;
use std::fs;
use std::path::Path;

fn derive_cmd(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary should be built");
    cmd.current_dir(dir);
    cmd
}

#[test]
fn misspelled_snapshots_key_is_rejected() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = dir.path().join("tier-a-limits.env");

    // "snapshot" is a typo for "snapshots"
    let bad_json = r#"{"schema_version": 1, "snapshot": []}"#;
    let file_path = dir.path().join("bad_key.json");
    fs::write(&file_path, bad_json).expect("write");

    let assert = derive_cmd(dir.path())
        .args(["budget-report", "--derive-limits"])
        .arg(&out)
        .args(["--from"])
        .arg(&file_path)
        .args([
            "--margin-cpu",
            "1.25",
            "--margin-memory",
            "1.25",
            "--margin-read",
            "1.25",
            "--margin-write",
            "1.25",
        ])
        .assert();

    // The command should fail and name the bad key
    assert.failure().stderr(predicates::str::contains(
        "unknown top-level key `snapshot`",
    ));
}

#[test]
fn extra_key_alongside_valid_keys_is_rejected() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = dir.path().join("tier-a-limits.env");

    let bad_json = r#"{
        "schema_version": 1,
        "snapshots": [],
        "extra_field": "unexpected"
    }"#;
    let file_path = dir.path().join("extra_key.json");
    fs::write(&file_path, bad_json).expect("write");

    let assert = derive_cmd(dir.path())
        .args(["budget-report", "--derive-limits"])
        .arg(&out)
        .args(["--from"])
        .arg(&file_path)
        .args([
            "--margin-cpu",
            "1.25",
            "--margin-memory",
            "1.25",
            "--margin-read",
            "1.25",
            "--margin-write",
            "1.25",
        ])
        .assert();

    assert.failure().stderr(predicates::str::contains(
        "unknown top-level key `extra_field`",
    ));
}

#[test]
fn valid_keys_still_accepted() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = dir.path().join("tier-a-limits.env");

    let good_json = r#"{
        "schema_version": 1,
        "snapshots": [
            {"package": "pkg", "function": "fn", "metric": "CPU Instructions", "value": 100}
        ]
    }"#;
    let file_path = dir.path().join("valid.json");
    fs::write(&file_path, good_json).expect("write");

    let assert = derive_cmd(dir.path())
        .args(["budget-report", "--derive-limits"])
        .arg(&out)
        .args(["--from"])
        .arg(&file_path)
        .args([
            "--margin-cpu",
            "1.25",
            "--margin-memory",
            "1.25",
            "--margin-read",
            "1.25",
            "--margin-write",
            "1.25",
        ])
        .assert();

    // Should succeed — no unknown keys
    assert.success();
}
