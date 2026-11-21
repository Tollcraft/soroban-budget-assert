//! Envelope metadata for `--json` (issue #1047).
//!
//! Every emitted envelope now carries `generated_at` (ISO-8601 UTC) and
//! `generator_version` alongside `schema_version` and `snapshots`, so an
//! archived report can be ordered and attributed to the release that produced
//! it. These tests exercise the real binary: a mocked measurement run for the
//! writer side, then the report-reading path for the round trip.

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

fn budget_report_cmd(dir: &Path, mocked: bool) -> Command {
    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary should be built");
    cmd.current_dir(dir);
    if mocked {
        let real_path = std::env::var("PATH").unwrap_or_default();
        cmd.env("PATH", format!("{}:{real_path}", fake_bin_dir().display()));
    }
    cmd
}

/// Assert `ts` is exactly `YYYY-MM-DDTHH:MM:SSZ`.
fn assert_iso8601_utc(ts: &str) {
    assert_eq!(
        ts.len(),
        20,
        "expected a second-precision UTC stamp, got {ts:?}"
    );
    for (index, byte) in ts.bytes().enumerate() {
        let expected = match index {
            4 | 7 => Some(b'-'),
            10 => Some(b'T'),
            13 | 16 => Some(b':'),
            19 => Some(b'Z'),
            _ => None,
        };
        match expected {
            Some(sep) => assert_eq!(byte, sep, "unexpected separator in {ts:?}"),
            None => assert!(byte.is_ascii_digit(), "non-digit in {ts:?}"),
        }
    }
}

#[test]
fn emitted_envelope_carries_generated_at_and_generator_version() {
    let workspace = setup_mock_workspace();

    let output = budget_report_cmd(workspace.path(), true)
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("--json must emit a single JSON document");
    assert_eq!(json["schema_version"], 1);
    assert!(
        json["snapshots"].is_array(),
        "snapshots must stay an array: {json}"
    );
    assert_iso8601_utc(
        json["generated_at"]
            .as_str()
            .expect("generated_at must be present as a string"),
    );
    assert_eq!(
        json["generator_version"],
        env!("CARGO_PKG_VERSION"),
        "generator_version must be the crate version"
    );
}

#[test]
fn emitted_envelope_round_trips_through_report_reading() {
    let workspace = setup_mock_workspace();

    let output = budget_report_cmd(workspace.path(), true)
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .clone();
    let envelope = String::from_utf8_lossy(&output.stdout).into_owned();

    // Feed the writer's own output back into the reader: the metadata keys are
    // additive, so validation must accept them and the snapshots must survive.
    let path = workspace.path().join("envelope.json");
    fs::write(&path, &envelope).expect("failed to write the envelope fixture");

    let round_trip = budget_report_cmd(workspace.path(), false)
        .args(["budget-report", "--markdown", "--from"])
        .arg(&path)
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&round_trip.stdout);
    assert!(
        stdout.contains("| Package | Function | Metric | Value |"),
        "the round-tripped envelope must render as a report table:\n{stdout}"
    );

    let json: serde_json::Value = serde_json::from_str(&envelope).expect("valid JSON");
    let snapshots = json["snapshots"].as_array().expect("snapshots array");
    let rendered_rows = stdout.lines().filter(|line| line.starts_with("| ")).count();
    // Header row + one row per snapshot (minus the empty "| " case, if any).
    assert!(
        rendered_rows >= snapshots.len(),
        "expected at least one table row per snapshot ({} snapshots, {} rows):\n{stdout}",
        snapshots.len(),
        rendered_rows
    );
}
