use std::path::PathBuf;
use std::process::Command;

fn exe_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("..");
    path.push("target");
    path.push("debug");
    path.push("cargo-budget-report");
    path
}

/// Integration test: malformed budget.toml → non-zero exit with a clear message.
/// This test runs the compiled binary in a temp directory with a broken TOML
/// and does NOT require network access or the stellar CLI.
#[test]
fn test_malformed_toml_exits_with_error() {
    let dir = tempfile::tempdir().unwrap();
    let toml_path = dir.path().join("budget.toml");
    std::fs::write(&toml_path, "[[[broken toml]]").unwrap();

    let output = Command::new(exe_path())
        .arg("budget-report")
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("network") || stderr.contains("missing"),
        "Expected error about missing network config, got: {stderr}"
    );
}

/// Integration test: no budget.toml at all → non-zero exit.
#[test]
fn test_no_toml_exits_with_error() {
    let dir = tempfile::tempdir().unwrap();

    let output = Command::new(exe_path())
        .arg("budget-report")
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("network") || stderr.contains("missing"),
        "Expected error about missing network config, got: {stderr}"
    );
}
