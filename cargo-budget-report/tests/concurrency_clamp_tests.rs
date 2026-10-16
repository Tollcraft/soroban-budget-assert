//! Regression tests for issue #863: `--concurrency 0` clamps to one worker
//! at runtime.
//!
//! Parsing already accepts `0` (see
//! `test_concurrency_zero_is_accepted_at_parse_time`); the clamp to a usable
//! worker count happens later in the run. The exact arithmetic
//! (`worker_count(0, n) == 1`, `worker_count(N, n) <= n`) is pinned by unit
//! tests in `src/main.rs`; these tests prove the behaviour end to end: runs
//! with `--concurrency 0`, `1`, and a value far above the task count all
//! complete and produce the same report.

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

fn run_with_concurrency(dir: &Path, concurrency: &str) -> std::process::Output {
    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary should be built");
    cmd.current_dir(dir).env("PATH", mocked_path());
    cmd.args([
        "budget-report",
        "--network",
        "local",
        "--source",
        "alice",
        "--concurrency",
        concurrency,
    ])
    .assert()
    .success()
    .get_output()
    .clone()
}

#[test]
fn concurrency_zero_completes_like_one() {
    let workspace = setup_mock_workspace();

    let zero = run_with_concurrency(workspace.path(), "0");
    let one = run_with_concurrency(workspace.path(), "1");

    let zero_stdout = String::from_utf8_lossy(&zero.stdout);
    let one_stdout = String::from_utf8_lossy(&one.stdout);
    assert!(
        zero_stdout.contains("WORKSPACE BUDGET REPORT"),
        "concurrency 0 must still complete every task, got: {zero_stdout}"
    );
    assert_eq!(
        zero_stdout.as_ref(),
        one_stdout.as_ref(),
        "concurrency 0 must behave exactly like the sequential run"
    );
}

#[test]
fn concurrency_above_task_count_completes_identically() {
    let workspace = setup_mock_workspace();

    let many = run_with_concurrency(workspace.path(), "64");
    let one = run_with_concurrency(workspace.path(), "1");

    let many_stdout = String::from_utf8_lossy(&many.stdout);
    let one_stdout = String::from_utf8_lossy(&one.stdout);
    assert!(
        many_stdout.contains("WORKSPACE BUDGET REPORT"),
        "excess concurrency must still complete every task, got: {many_stdout}"
    );
    assert_eq!(
        many_stdout.as_ref(),
        one_stdout.as_ref(),
        "clamped workers must render the same report as the sequential run"
    );
}
