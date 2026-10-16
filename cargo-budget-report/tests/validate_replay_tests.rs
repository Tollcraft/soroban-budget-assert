//! Regression tests for issue #861: `--replay` silently drops `--validate`.
//!
//! The skip is intentional (replay mode cannot assume `stellar xdr decode`
//! exists), but it was invisible: no test pinned the interaction. These
//! tests drive the run path through the real binary against the deterministic
//! mock workspace:
//!
//! * a live (mocked-PATH) run with `--validate` performs validation — the
//!   stock fake `xdr decode` agrees with the fake RPC payload, so the run
//!   reports `validation passed` (a forced mismatch variant of this setup is
//!   already pinned by `exit_code_generic_on_validate_mismatch` in
//!   `exit_code_tests.rs`);
//! * a `--replay` run of the same fixture with `--validate` skips validation
//!   entirely — no `VALIDATION FAILED`, no `validation passed` line — and
//!   exits successfully with a byte-identical report.

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
    format!("{}:{}", fake_bin_dir().display(), real_path)
}

fn budget_report_cmd(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary should be built");
    cmd.current_dir(dir).env("PATH", mocked_path());
    cmd
}

#[test]
fn validate_runs_on_a_live_run() {
    let workspace = setup_mock_workspace();

    let assert = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--validate",
        ])
        .assert();

    // The stock fake `xdr decode` agrees with the fake RPC payload, so a
    // live validation run must execute validation and report the pass.
    let output = assert.success().get_output().clone();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("validation passed"),
        "a live --validate run must execute validation, got: {stderr}"
    );
}

#[test]
fn replay_silently_drops_validate() {
    let workspace = setup_mock_workspace();
    let fixture_path = workspace.path().join("fixture.json");

    // Record the fixture first (a live run without --validate succeeds).
    let record = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--record",
            fixture_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .get_output()
        .clone();
    let recorded_stdout = String::from_utf8_lossy(&record.stdout).into_owned();

    // Replay with --validate and a PATH that excludes the fake stellar/curl
    // scripts entirely: the whole pipeline must run with no Stellar CLI.
    let mut replay_cmd = Command::cargo_bin("cargo-budget-report").expect("binary should be built");
    replay_cmd
        .current_dir(workspace.path())
        .env("PATH", std::env::var("PATH").unwrap_or_default());
    let replay = replay_cmd
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--replay",
            fixture_path.to_str().unwrap(),
            "--validate",
        ])
        .assert()
        .success()
        .get_output()
        .clone();

    let replayed_stdout = String::from_utf8_lossy(&replay.stdout);
    assert_eq!(
        recorded_stdout,
        replayed_stdout.as_ref(),
        "replay with --validate must reproduce the recorded report"
    );
    let replayed_stderr = String::from_utf8_lossy(&replay.stderr);
    assert!(
        !replayed_stderr.contains("VALIDATION FAILED"),
        "replay must not validate even with the flag present, got: {replayed_stderr}"
    );
    assert!(
        !replayed_stderr.contains("validation passed"),
        "replay must not attempt validation at all, got: {replayed_stderr}"
    );
}
