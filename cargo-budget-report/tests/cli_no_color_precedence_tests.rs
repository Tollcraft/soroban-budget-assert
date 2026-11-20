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
fn test_no_color_precedence_over_color_always() {
    let workspace = setup_mock_workspace();
    fs::write(
        workspace.path().join("budget.toml"),
        "network = \"local\"\n\
         source = \"alice\"\n\
         \n\
         [functions.ping]\n\
         cpu_limit = 100\n",
    )
    .expect("failed to write budget.toml");

    // 1. Run with --color always and NO_COLOR=1: escape bytes (0x1b) must be suppressed
    let output_no_color = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--check",
            "--color",
            "always",
        ])
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to execute command");

    assert!(
        !output_no_color.stdout.contains(&0x1b),
        "stdout should not contain ANSI escape bytes when NO_COLOR=1"
    );

    // 2. Run with --color always and NO_COLOR="" (empty): color codes are restored
    let output_color = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--check",
            "--color",
            "always",
        ])
        .env("NO_COLOR", "")
        .output()
        .expect("failed to execute command");

    assert!(
        output_color.stdout.contains(&0x1b),
        "stdout should contain ANSI escape bytes when NO_COLOR is empty"
    );
}
