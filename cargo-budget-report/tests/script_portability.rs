//! Portability tests for the shell scripts under `scripts/`.
//!
//! `CONTRIBUTING.md` tells contributors on every platform to run these
//! scripts, and `.github/workflows/quality.yml` runs
//! `check-module-reachability.sh` as a CI gate. macOS still ships bash 3.2
//! as `/bin/bash`, so a bash 4-only construct makes a script abort with
//! `mapfile: command not found` on a stock macOS checkout while passing on
//! the Ubuntu runner.
//!
//! The first test is a source scan, because the construct check has to fail
//! on the Ubuntu runner too, where bash is 5.x and the scripts would
//! otherwise run fine. The remaining tests exercise
//! `check-module-reachability.sh` end to end against a scratch tree so the
//! bash 3.2-compatible rewrite of its visited-set and work-queue is held to
//! the behaviour it replaced.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Absolute path to the real repository root (where `scripts/` lives),
/// regardless of the cwd the test binary is invoked from.
fn repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .expect("cargo-budget-report has a parent directory")
        .to_path_buf()
}

/// Bash 4-only constructs, paired with the reason each one is unavailable
/// on bash 3.2.
const BASH_4_ONLY: &[(&str, &str)] = &[
    ("mapfile ", "`mapfile` is a bash 4 builtin"),
    ("mapfile\t", "`mapfile` is a bash 4 builtin"),
    ("readarray ", "`readarray` is a bash 4 builtin"),
    ("readarray\t", "`readarray` is a bash 4 builtin"),
    ("declare -A", "associative arrays are bash 4"),
    ("local -A", "associative arrays are bash 4"),
    ("typeset -A", "associative arrays are bash 4"),
];

/// Every executable shell script the contributor docs or CI ask people to
/// run directly.
fn shell_scripts() -> Vec<PathBuf> {
    let scripts_dir = repo_root().join("scripts");
    let mut scripts: Vec<PathBuf> = fs::read_dir(&scripts_dir)
        .expect("scripts/ should exist")
        .map(|entry| entry.expect("scripts/ entry should be readable").path())
        .filter(|path| {
            path.is_file()
                && match path.extension().and_then(|ext| ext.to_str()) {
                    Some("sh") => true,
                    // `scripts/pre-commit` is a shell script with no extension.
                    None => true,
                    _ => false,
                }
        })
        .collect();
    scripts.sort();
    assert!(
        !scripts.is_empty(),
        "expected at least one shell script under scripts/"
    );
    scripts
}

/// True for a line that is entirely a `#` comment, so prose *about* a
/// construct is not mistaken for a use of it.
fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

#[test]
fn shell_scripts_avoid_bash_4_only_constructs() {
    let mut offences = Vec::new();

    for script in shell_scripts() {
        let source = fs::read_to_string(&script)
            .unwrap_or_else(|err| panic!("reading {}: {err}", script.display()));

        for (number, line) in source.lines().enumerate() {
            if is_comment(line) {
                continue;
            }
            for (construct, reason) in BASH_4_ONLY {
                if line.contains(construct) {
                    offences.push(format!(
                        "{}:{}: {} ({})",
                        script.display(),
                        number + 1,
                        construct.trim(),
                        reason
                    ));
                }
            }
        }
    }

    assert!(
        offences.is_empty(),
        "these scripts use constructs that abort on macOS's bash 3.2:\n{}",
        offences.join("\n")
    );
}

/// Lays out a scratch workspace containing one crate whose module tree is
/// `main.rs -> used.rs` and `src/bin/tool.rs -> src/bin/helpers/mod.rs`, plus a copy of the real
/// `check-module-reachability.sh` at the path it expects (`scripts/`
/// directly under the tree root).
fn setup_scratch_tree(dir: &Path) {
    let src = dir.join("scratch-crate").join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("main.rs"), "mod used;\n\nfn main() {}\n").unwrap();
    fs::write(src.join("used.rs"), "pub fn used() {}\n").unwrap();

    let bin = src.join("bin");
    let bin_helpers = bin.join("helpers");
    fs::create_dir_all(&bin_helpers).unwrap();
    fs::write(bin.join("tool.rs"), "mod helpers;\n\nfn main() {}\n").unwrap();
    fs::write(bin_helpers.join("mod.rs"), "pub fn used() {}\n").unwrap();

    let scratch_scripts = dir.join("scripts");
    fs::create_dir_all(&scratch_scripts).unwrap();
    fs::copy(
        repo_root()
            .join("scripts")
            .join("check-module-reachability.sh"),
        scratch_scripts.join("check-module-reachability.sh"),
    )
    .unwrap();
}

fn run_reachability_check(dir: &Path) -> std::process::Output {
    Command::new("bash")
        .arg("scripts/check-module-reachability.sh")
        .current_dir(dir)
        .output()
        .expect("bash should run the reachability check")
}

#[test]
fn reachability_check_passes_when_every_module_is_declared() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let dir = tmp.path();
    setup_scratch_tree(dir);

    let output = run_reachability_check(dir);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "a fully declared tree should pass.\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("all .rs files under src/ are reachable"),
        "expected the success line, got: {stdout}"
    );
}

#[test]
fn reachability_check_names_the_orphaned_module() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let dir = tmp.path();
    setup_scratch_tree(dir);

    fs::write(
        dir.join("scratch-crate").join("src").join("orphan.rs"),
        "pub fn orphan() {}\n",
    )
    .unwrap();

    let output = run_reachability_check(dir);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !output.status.success(),
        "an orphaned module should fail the check.\nstdout: {stdout}"
    );
    assert!(
        stdout.contains("orphaned: ./scratch-crate/src/orphan.rs"),
        "expected the orphan to be named, got: {stdout}"
    );
    assert!(
        !stdout.contains("used.rs"),
        "a declared module must not be reported as orphaned, got: {stdout}"
    );
}

#[test]
fn reachability_check_names_an_orphan_under_a_bin_root() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let dir = tmp.path();
    setup_scratch_tree(dir);

    fs::write(
        dir.join("scratch-crate")
            .join("src")
            .join("bin")
            .join("helpers")
            .join("orphan.rs"),
        "pub fn orphan() {}\n",
    )
    .unwrap();

    let output = run_reachability_check(dir);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !output.status.success(),
        "an orphan under a binary crate root should fail the check.\nstdout: {stdout}"
    );
    assert!(
        stdout.contains("orphaned: ./scratch-crate/src/bin/helpers/orphan.rs"),
        "expected the bin submodule orphan to be named, got: {stdout}"
    );
}
