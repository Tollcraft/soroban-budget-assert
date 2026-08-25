//! Integration test for `scripts/pre-commit` + the two hook installers
//! (`scripts/install-hooks.sh` and `scripts/install-hooks.ps1`).
//!
//! Since these are shell/PowerShell scripts, not Rust code, the only
//! meaningful test is behavioral: install the hook into a real scratch git
//! repository and verify it actually blocks a commit when
//! `cargo fmt --all -- --check` would fail, and allows the commit once
//! formatting is fixed. The installer-specific tests additionally cover the
//! parity states called out in issue #476: a missing `.git/hooks`
//! directory, running the installer twice, and running it from a
//! subdirectory of the repo.
//!
//! The `install-hooks.ps1` coverage only runs when a `pwsh` or `powershell`
//! binary is present on `PATH`; this CI and the environment this test suite
//! was written in have neither, so that coverage has not actually been
//! exercised here. It is written to run for real wherever PowerShell is
//! available (e.g. a Windows CI runner), rather than being skipped
//! unconditionally.

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

/// Copies just `scripts/pre-commit` and `scripts/install-hooks.sh` into a
/// fresh git repo at `dir`, without running the installer, so tests can
/// exercise the installer themselves under specific starting conditions.
fn setup_scratch_repo_without_installing(dir: &Path) {
    let status = Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .status()
        .expect("git init should run");
    assert!(status.success(), "git init failed");

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
}

#[test]
fn install_hooks_sh_creates_missing_hooks_directory() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let dir = tmp.path();
    setup_scratch_repo_without_installing(dir);

    // Simulate a `.git/hooks` directory that does not exist.
    fs::remove_dir_all(dir.join(".git/hooks")).unwrap();
    assert!(!dir.join(".git/hooks").exists());

    let output = Command::new("bash")
        .arg("scripts/install-hooks.sh")
        .current_dir(dir)
        .output()
        .expect("install-hooks.sh should run");

    assert!(
        output.status.success(),
        "installer should succeed even when .git/hooks is missing; stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        dir.join(".git/hooks/pre-commit").exists(),
        "hook was not installed after .git/hooks was recreated"
    );
}

#[test]
fn install_hooks_sh_is_idempotent_when_run_twice() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let dir = tmp.path();
    setup_scratch_repo_without_installing(dir);

    for _ in 0..2 {
        let status = Command::new("bash")
            .arg("scripts/install-hooks.sh")
            .current_dir(dir)
            .status()
            .expect("install-hooks.sh should run");
        assert!(
            status.success(),
            "install-hooks.sh should succeed on every run"
        );
    }

    assert!(dir.join(".git/hooks/pre-commit").exists());
}

#[test]
fn install_hooks_sh_works_from_a_subdirectory() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let dir = tmp.path();
    setup_scratch_repo_without_installing(dir);

    let subdir = dir.join("some/nested/dir");
    fs::create_dir_all(&subdir).unwrap();

    let status = Command::new("bash")
        .arg(dir.join("scripts/install-hooks.sh"))
        .current_dir(&subdir)
        .status()
        .expect("install-hooks.sh should run");
    assert!(
        status.success(),
        "installer should succeed when invoked from a repo subdirectory"
    );
    assert!(dir.join(".git/hooks/pre-commit").exists());
}

#[test]
fn install_hooks_sh_fails_clearly_when_source_is_missing() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let dir = tmp.path();
    setup_scratch_repo_without_installing(dir);

    fs::remove_file(dir.join("scripts/pre-commit")).unwrap();

    let output = Command::new("bash")
        .arg("scripts/install-hooks.sh")
        .current_dir(dir)
        .output()
        .expect("install-hooks.sh should run");

    assert!(
        !output.status.success(),
        "installer should fail (not silently no-op) when the hook source is missing"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Hook source not found"),
        "expected a clear error about the missing source, got stderr={stderr:?}"
    );
    assert!(
        !dir.join(".git/hooks/pre-commit").exists(),
        "no hook should have been installed on failure"
    );
}

/// Finds a PowerShell binary on `PATH`, preferring `pwsh` (PowerShell Core,
/// cross-platform) over Windows PowerShell's `powershell`.
fn find_powershell() -> Option<&'static str> {
    ["pwsh", "powershell"].into_iter().find(|&candidate| {
        Command::new(candidate)
            .arg("-Command")
            .arg("$PSVersionTable.PSVersion")
            .output()
            .is_ok_and(|o| o.status.success())
    })
}

/// Covers `scripts/install-hooks.ps1` parity with `install-hooks.sh`:
/// installs into a real scratch repo and confirms the hook actually blocks
/// a badly formatted commit. Skips (rather than failing) when no `pwsh` /
/// `powershell` binary is available, which is the case in this repository's
/// current CI and was the case in the environment this test was written in.
#[test]
fn pre_commit_hook_installed_via_powershell_blocks_badly_formatted_code() {
    let Some(powershell) = find_powershell() else {
        eprintln!(
            "skipping: neither `pwsh` nor `powershell` found on PATH; \
             install-hooks.ps1 was not exercised by this test run"
        );
        return;
    };

    let tmp = tempfile::tempdir().expect("create tempdir");
    let dir = tmp.path();
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
        real_scripts_dir.join("install-hooks.ps1"),
        scratch_scripts_dir.join("install-hooks.ps1"),
    )
    .unwrap();

    let install_status = Command::new(powershell)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-File",
            "scripts/install-hooks.ps1",
        ])
        .current_dir(dir)
        .status()
        .expect("install-hooks.ps1 should run");
    assert!(install_status.success(), "install-hooks.ps1 failed");
    assert!(
        dir.join(".git/hooks/pre-commit").exists(),
        "hook was not installed at .git/hooks/pre-commit via install-hooks.ps1"
    );

    fs::write(
        dir.join("src/lib.rs"),
        "pub fn add(a:i32,b:i32)->i32{a+b}\n",
    )
    .unwrap();
    git_add_all(dir);
    let output = git_commit(dir, "add badly formatted code");

    assert!(
        !output.status.success(),
        "commit should have been blocked by the hook installed via install-hooks.ps1"
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
