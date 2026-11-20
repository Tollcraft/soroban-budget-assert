//! Integration tests for issue #1021:
//! Negative or NaN tolerance values in `budget.toml` are rejected during configuration loading.

use assert_cmd::Command;
use std::fs;

#[test]
fn test_negative_tolerance_in_budget_toml_rejected() {
    let tmp = tempfile::tempdir().expect("failed to create tempdir");
    fs::write(tmp.path().join("budget.toml"), "tolerance = -0.05\n")
        .expect("failed to write budget.toml");

    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary must build");
    cmd.current_dir(tmp.path())
        .arg("--replay")
        .arg("non_existent.json");

    let output = cmd.output().expect("failed to run binary");
    assert!(
        !output.status.success(),
        "run must fail due to negative tolerance"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stderr}\n{stdout}");

    assert!(
        combined.contains("tolerance"),
        "error must identify the tolerance key: {combined}"
    );
    assert!(
        combined.contains("-0.05"),
        "error must identify the invalid value: {combined}"
    );
    assert!(
        combined.contains("non-negative"),
        "error must mention non-negative requirement: {combined}"
    );
}

#[test]
fn test_nan_tolerance_in_budget_toml_rejected() {
    let tmp = tempfile::tempdir().expect("failed to create tempdir");
    fs::write(tmp.path().join("budget.toml"), "tolerance = nan\n")
        .expect("failed to write budget.toml");

    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary must build");
    cmd.current_dir(tmp.path())
        .arg("--replay")
        .arg("non_existent.json");

    let output = cmd.output().expect("failed to run binary");
    assert!(
        !output.status.success(),
        "run must fail due to nan tolerance"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stderr}\n{stdout}");

    assert!(
        combined.contains("tolerance"),
        "error must identify the tolerance key: {combined}"
    );
}
