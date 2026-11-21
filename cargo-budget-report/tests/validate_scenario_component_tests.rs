use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fake_bin_dir() -> PathBuf {
    manifest_dir().join("tests/fixtures/fake_bin")
}

const SKIPPED_ENTRIES: [&str; 2] = ["target", "Cargo.lock"];

fn copy_dir_all(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("failed to create destination directory");
    for entry in fs::read_dir(src).expect("failed to read source directory") {
        let entry = entry.expect("failed to read entry");
        let file_name = entry.file_name();
        let name_str = file_name.to_str().unwrap_or_default();
        if SKIPPED_ENTRIES.contains(&name_str) {
            continue;
        }
        let src_path = entry.path();
        let dst_path = dst.join(file_name);
        if src_path.is_dir() {
            copy_dir_all(&src_path, &dst_path);
        } else {
            fs::copy(&src_path, &dst_path).expect("failed to copy file");
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
fn test_validate_catches_misspelled_scenario_component() {
    let workspace = setup_mock_workspace();
    fs::write(
        workspace.path().join("budget.toml"),
        r#"network = "local"
source = "alice"

[scenarios.my_workflow]
package = "mock-contract-a"
functions = ["piing"]
"#,
    )
    .expect("failed to write budget.toml");

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

    let output = assert.failure().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let all = format!("{stdout}\n{stderr}");

    assert!(
        all.contains("my_workflow"),
        "error should identify scenario name, got:\n{all}"
    );
    assert!(
        all.contains("piing"),
        "error should identify misspelled component function name, got:\n{all}"
    );
    assert!(
        all.contains("ping"),
        "error should suggest closest match 'ping', got:\n{all}"
    );
}

#[test]
fn test_validate_catches_qualified_misspelled_scenario_component() {
    let workspace = setup_mock_workspace();
    fs::write(
        workspace.path().join("budget.toml"),
        r#"network = "local"
source = "alice"

[scenarios.my_workflow]
package = "mock-contract-a"
functions = ["mock-contract-a::piing"]
"#,
    )
    .expect("failed to write budget.toml");

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

    let output = assert.failure().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let all = format!("{stdout}\n{stderr}");

    assert!(
        all.contains("my_workflow"),
        "error should identify scenario name, got:\n{all}"
    );
    assert!(
        all.contains("piing"),
        "error should identify misspelled component function name, got:\n{all}"
    );
    assert!(
        all.contains("ping"),
        "error should suggest closest match 'ping', got:\n{all}"
    );
}

#[test]
fn test_validate_accepts_valid_scenario_components() {
    let workspace = setup_mock_workspace();
    fs::write(
        workspace.path().join("budget.toml"),
        r#"network = "local"
source = "alice"

[scenarios.my_workflow]
package = "mock-contract-a"
functions = ["ping"]
"#,
    )
    .expect("failed to write budget.toml");

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

    assert.success();
}
