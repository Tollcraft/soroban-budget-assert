//! Integration test for `scripts/pre-commit` + `scripts/install-hooks.sh`.
//!
//! Since these are shell scripts, not Rust code, the only meaningful test is
//! behavioral: install the hook into a real scratch git repository and verify
//! it actually blocks a commit when `cargo fmt --all -- --check` would fail,
//! and allows the commit once formatting is fixed.

use std::fs;
use std::path::Path;
use std::process::Command;

/// Absolute path to the real repository root (where `scripts/` lives),
/// regardless of the cwd the test binary is invoked from.
fn repo_root() -> std::path::PathBuf {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .expect("cargo-budget-report has a parent directory")
        .to_path_buf()
}

/// Builds a throwaway git + cargo project in `dir`, installs the real
/// `scripts/install-hooks.sh` / `scripts/pre-commit` into it, and returns
/// once the hook is in place.
fn setup_scratch_repo(dir: &Path) {
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"scratch\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();

    let status = Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .status()
        .expect("git init should run");
    assert!(status.success(), "git init failed");

    // Minimal identity so `git commit` doesn't fail on a fresh CI/dev machine.
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(dir)
        .status()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir)
        .status()
        .unwrap();

    let real_scripts_dir = repo_root().join("scripts");
    let scratch_scripts_dir = dir.join("scripts");
    fs::create_dir_all(&scratch_scripts_dir).unwrap();
    fs::copy(
        real_scripts_dir.join("pre-commit"),
        scratch_scripts_dir.join("pre-commit"),
    )
    .unwrap();
    fs::copy(
        real_scripts_dir.join("install-hooks.sh"),
        scratch_scripts_dir.join("install-hooks.sh"),
    )
    .unwrap();

    let status = Command::new("bash")
        .arg("scripts/install-hooks.sh")
        .current_dir(dir)
        .status()
        .expect("install-hooks.sh should run");
    assert!(status.success(), "install-hooks.sh failed");

    assert!(
        dir.join(".git/hooks/pre-commit").exists(),
        "hook was not installed at .git/hooks/pre-commit"
    );
}

fn git_add_all(dir: &Path) {
    let status = Command::new("git")
        .args(["add", "."])
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(status.success());
}

fn git_commit(dir: &Path, message: &str) -> std::process::Output {
    Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(dir)
        .output()
        .expect("git commit should run")
}

#[test]
fn pre_commit_hook_blocks_badly_formatted_code() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let dir = tmp.path();
    setup_scratch_repo(dir);

    // Deliberately malformed: extra spaces, no rustfmt-standard layout.
    fs::write(
        dir.join("src/lib.rs"),
        "pub fn add(a:i32,b:i32)->i32{a+b}\n",
    )
    .unwrap();

    git_add_all(dir);
    let output = git_commit(dir, "add badly formatted code");

    assert!(
        !output.status.success(),
        "commit should have been blocked by the pre-commit hook"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Formatting check failed") || stderr.contains("Formatting check failed"),
        "expected hook's failure message in output, got stdout={stdout:?} stderr={stderr:?}"
    );
}

#[test]
fn pre_commit_hook_allows_well_formatted_code() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let dir = tmp.path();
    setup_scratch_repo(dir);

    fs::write(
        dir.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
    )
    .unwrap();

    git_add_all(dir);
    let output = git_commit(dir, "add well formatted code");

    assert!(
        output.status.success(),
        "commit should have succeeded; stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
