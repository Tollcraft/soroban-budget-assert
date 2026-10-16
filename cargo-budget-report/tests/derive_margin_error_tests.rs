//! Regression tests for issue #862: `run_derive_mode` margin-resolution
//! error branches, driven through the real binary.
//!
//! Each test runs `cargo budget-report --derive-limits` in a fresh tempdir
//! (so no `budget.toml` margin is present unless the test writes one) with
//! the checked-in Tier B sample report as `--from` input, and asserts the
//! run surfaces the margin error instead of deriving limits.

use assert_cmd::Command;
use std::path::{Path, PathBuf};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn tier_b_sample() -> PathBuf {
    manifest_dir()
        .join("fixtures")
        .join("tier_b_report_sample.json")
}

fn derive_cmd(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary should be built");
    cmd.current_dir(dir);
    cmd
}

fn full_margin_args() -> Vec<String> {
    vec![
        "--margin-cpu".to_string(),
        "1.5".to_string(),
        "--margin-memory".to_string(),
        "1.5".to_string(),
        "--margin-read".to_string(),
        "1.5".to_string(),
        "--margin-write".to_string(),
        "1.5".to_string(),
    ]
}

#[test]
fn derive_fails_on_non_numeric_margin_cpu() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let out = dir.path().join("tier-a-limits.env");

    let assert = derive_cmd(dir.path())
        .args(["budget-report", "--derive-limits"])
        .arg(&out)
        .args(["--from"])
        .arg(tier_b_sample())
        .args([
            "--margin-cpu",
            "not-a-number",
            "--margin-memory",
            "1.5",
            "--margin-read",
            "1.5",
            "--margin-write",
            "1.5",
        ])
        .assert();

    let output = assert.failure().get_output().clone();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalid --margin-cpu"),
        "should report the parse error rather than a default, got: {stderr}"
    );
    assert!(
        !out.exists(),
        "no limits artifact may be written when margin parsing fails"
    );
}

#[test]
fn derive_fails_on_partial_cli_margin_set() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let out = dir.path().join("tier-a-limits.env");

    // Only cpu + memory supplied; read + write are missing.
    let assert = derive_cmd(dir.path())
        .args(["budget-report", "--derive-limits"])
        .arg(&out)
        .args(["--from"])
        .arg(tier_b_sample())
        .args(["--margin-cpu", "1.5", "--margin-memory", "1.5"])
        .assert();

    let output = assert.failure().get_output().clone();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("CLI margin is partially set"),
        "got: {stderr}"
    );
    assert!(
        stderr.contains("read") && stderr.contains("write"),
        "should name the missing flags, got: {stderr}"
    );
    assert!(
        !out.exists(),
        "derivation must not proceed on a partial margin set"
    );
}

#[test]
fn derive_fails_when_no_margin_is_supplied() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let out = dir.path().join("tier-a-limits.env");

    // No --margin-* flags and no budget.toml at all.
    let assert = derive_cmd(dir.path())
        .args(["budget-report", "--derive-limits"])
        .arg(&out)
        .args(["--from"])
        .arg(tier_b_sample())
        .assert();

    let output = assert.failure().get_output().clone();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no margin supplied"), "got: {stderr}");
    assert!(
        !out.exists(),
        "derivation must not proceed without a margin"
    );
}

#[test]
fn derive_fails_on_incomplete_toml_margin_without_cli_flags() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let out = dir.path().join("tier-a-limits.env");
    std::fs::write(
        dir.path().join("budget.toml"),
        "[margin]\ncpu_margin = 1.5\n",
    )
    .expect("failed to write budget.toml");

    let assert = derive_cmd(dir.path())
        .args(["budget-report", "--derive-limits"])
        .arg(&out)
        .args(["--from"])
        .arg(tier_b_sample())
        .assert();

    let output = assert.failure().get_output().clone();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no margin supplied"),
        "an incomplete [margin] table is the same error as none, got: {stderr}"
    );
    assert!(
        !out.exists(),
        "derivation must not proceed on an incomplete [margin] table"
    );
}

#[test]
fn derive_succeeds_with_a_complete_cli_margin_set() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let out = dir.path().join("tier-a-limits.env");

    let assert = derive_cmd(dir.path())
        .args(["budget-report", "--derive-limits"])
        .arg(&out)
        .args(["--from"])
        .arg(tier_b_sample())
        .args(full_margin_args())
        .assert();

    assert.success();
    assert!(
        out.exists(),
        "a complete margin set must produce the limits artifact"
    );
}
