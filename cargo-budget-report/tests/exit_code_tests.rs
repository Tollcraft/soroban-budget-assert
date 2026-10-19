//! Process-level integration tests for `cargo-budget-report` exit codes.
//!
//! Issue #406: cargo-budget-report exits with distinct codes so CI can
//! branch on the outcome. These tests run the real compiled binary and
//! assert on the exact process exit code, not just success/failure.
//!
//! Exit code contract (from `cargo-budget-report/src/error.rs`):
//!   0 = success
//!   1 = generic failure
//!   3 = config error
//!   4 = budget exceeded
//!   5 = regression
//!   6 = network failure

use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn mock_workspace_fixture() -> PathBuf {
    manifest_dir().join("tests/fixtures/mock_workspace")
}

fn fake_bin_dir() -> PathBuf {
    manifest_dir().join("tests/fixtures/fake_bin")
}

/// Build outputs that `cargo build` leaves in the fixture if it was ever
/// built in place (both are in the fixture's `.gitignore`). Copying them
/// would duplicate an entire `target/` tree into every test's tempdir, and a
/// stale `Cargo.lock` would leak into the test run.
const SKIPPED_ENTRIES: [&str; 2] = ["target", "Cargo.lock"];

/// Recursively copies `src` into `dst`, skipping [`SKIPPED_ENTRIES`].
fn copy_dir_all(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("failed to create destination directory");
    for entry in fs::read_dir(src).expect("failed to read source directory") {
        let entry = entry.expect("failed to read directory entry");
        let file_name = entry.file_name();
        if SKIPPED_ENTRIES.iter().any(|skip| file_name == *skip) {
            continue;
        }
        let file_type = entry.file_type().expect("failed to read file type");
        let dst_path = dst.join(&file_name);
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &dst_path);
        } else {
            fs::copy(entry.path(), &dst_path).expect("failed to copy fixture file");
        }
    }
}

fn setup_mock_workspace() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("failed to create tempdir");
    copy_dir_all(&mock_workspace_fixture(), tmp.path());
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

// ── Exit code constants (must match error.rs) ─────────────────────────
const EXIT_SUCCESS: i32 = 0;
const EXIT_GENERIC_FAILURE: i32 = 1;
const EXIT_CONFIG_ERROR: i32 = 3;
const EXIT_BUDGET_EXCEEDED: i32 = 4;
const EXIT_REGRESSION: i32 = 5;
const EXIT_NETWORK_FAILURE: i32 = 6;

/// Arguments shared by every invocation in this file.
const BASE_ARGS: [&str; 5] = ["budget-report", "--network", "local", "--source", "alice"];

/// Runs the binary in `dir` with [`BASE_ARGS`] plus `extra_args` and asserts
/// the exact exit code. The captured output is borrowed, not cloned, so its
/// stdout/stderr buffers are never copied just to read the status.
fn assert_exit_code(dir: &Path, extra_args: &[&str], expected: i32, msg: &str) {
    let assert = budget_report_cmd(dir)
        .args(BASE_ARGS)
        .args(extra_args)
        .assert();
    let output = assert.get_output();
    assert_eq!(
        output.status.code(),
        Some(expected),
        "{msg}\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn exit_code_zero_on_success() {
    let workspace = setup_mock_workspace();

    // No budget.toml — the tool runs a plain report with no limits.
    assert_exit_code(
        workspace.path(),
        &[],
        EXIT_SUCCESS,
        "clean run should exit 0",
    );
}

#[test]
fn exit_code_config_error_on_malformed_budget_toml() {
    let workspace = setup_mock_workspace();

    // Write a TOML file that is syntactically invalid.
    fs::write(
        workspace.path().join("budget.toml"),
        "this is not valid toml {{{{",
    )
    .expect("failed to write budget.toml");

    assert_exit_code(
        workspace.path(),
        &[],
        EXIT_CONFIG_ERROR,
        "malformed budget.toml should exit 3 (config error)",
    );
}

#[test]
fn exit_code_budget_exceeded_when_check_fails() {
    let workspace = setup_mock_workspace();

    // Set a CPU limit that the mock contract will exceed.
    fs::write(
        workspace.path().join("budget.toml"),
        "[functions.ping]\ncpu_limit = 10\n\n[functions.pong]\ncpu_limit = 5000000\n",
    )
    .expect("failed to write budget.toml");

    assert_exit_code(
        workspace.path(),
        &["--check"],
        EXIT_BUDGET_EXCEEDED,
        "budget limit breach should exit 4 (budget exceeded)",
    );
}

#[test]
fn exit_code_zero_when_check_passes() {
    let workspace = setup_mock_workspace();

    // Set generous limits that the mock contract will not breach.
    fs::write(
        workspace.path().join("budget.toml"),
        "[functions.ping]\ncpu_limit = 5000000\n\n[functions.pong]\ncpu_limit = 5000000\n",
    )
    .expect("failed to write budget.toml");

    assert_exit_code(
        workspace.path(),
        &["--check"],
        EXIT_SUCCESS,
        "check with generous limits should exit 0",
    );
}

#[test]
fn copy_dir_all_skips_build_outputs() {
    let src = tempfile::tempdir().expect("failed to create src tempdir");
    let dst = tempfile::tempdir().expect("failed to create dst tempdir");

    fs::write(src.path().join("Cargo.toml"), "[workspace]\n").expect("write Cargo.toml");
    fs::write(src.path().join("Cargo.lock"), "stale").expect("write Cargo.lock");
    fs::create_dir_all(src.path().join("target/debug")).expect("create target/");
    fs::write(src.path().join("target/debug/artifact"), "big").expect("write artifact");
    fs::create_dir_all(src.path().join("member/src")).expect("create member/src");
    fs::write(src.path().join("member/src/lib.rs"), "").expect("write lib.rs");
    fs::create_dir_all(src.path().join("member/target")).expect("create member/target");

    copy_dir_all(src.path(), dst.path());

    assert!(dst.path().join("Cargo.toml").is_file());
    assert!(dst.path().join("member/src/lib.rs").is_file());
    assert!(!dst.path().join("Cargo.lock").exists());
    assert!(!dst.path().join("target").exists());
    assert!(!dst.path().join("member/target").exists());
}

// ── Issue #843: deterministic validation failures are not network faults ──

/// A `--validate` decode mismatch is deterministic and reproducible, so it
/// must exit with the generic (non-retryable) code 1, never the network
/// code 6. The fake `stellar` script below decodes to values that disagree
/// with the fake RPC payload, forcing a mismatch on every function.
#[test]
fn exit_code_generic_on_validate_mismatch() {
    let workspace = setup_mock_workspace();

    // Shadow `fake_bin` with a `stellar` whose `xdr decode` disagrees with
    // the RPC payload (instructions 2 vs the ~1,000,000 the fixture XDR
    // carries), while `curl` stays the stock fake. `fs::copy` carries the
    // executable bit over and `fs::write` truncates in place, so the shadow
    // scripts stay executable without a platform-specific chmod.
    let shadow = tempfile::tempdir().expect("failed to create shadow bindir");
    for bin in ["curl", "stellar"] {
        fs::copy(fake_bin_dir().join(bin), shadow.path().join(bin)).expect("copy fake bin");
    }
    let stellar_src =
        fs::read_to_string(shadow.path().join("stellar")).expect("read shadow stellar");
    let mismatching = stellar_src.replace("\"instructions\": 1000000,", "\"instructions\": 2,");
    assert_ne!(mismatching, stellar_src, "fixture must contain the pin");
    fs::write(shadow.path().join("stellar"), &mismatching).expect("write shadow stellar");
    let real_path = std::env::var("PATH").unwrap_or_default();
    let shadow_path = format!("{}:{}", shadow.path().display(), real_path);

    let assert = Command::cargo_bin("cargo-budget-report")
        .expect("binary should be built")
        .current_dir(workspace.path())
        .env("PATH", shadow_path)
        .args(BASE_ARGS)
        .args(["--validate"])
        .assert();
    let output = assert.get_output();
    assert_eq!(
        output.status.code(),
        Some(EXIT_GENERIC_FAILURE),
        "validate mismatch should exit 1 (generic), not 6\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("VALIDATION FAILED"),
        "mismatch diagnostic should surface: {stderr}"
    );
}

/// Simulations that never produce metrics (here: every invoke-build fails
/// deterministically) are transient infrastructure faults and keep the
/// retryable network code 6.
#[test]
fn exit_code_network_failure_when_simulations_produce_no_metrics() {
    let workspace = setup_mock_workspace();
    let fail_count_file = workspace.path().join(".mock_invoke_fail_permanent");
    let _ = fs::remove_file(&fail_count_file);

    let assert = budget_report_cmd(workspace.path())
        .args(BASE_ARGS)
        .args(["--retry-backoff-secs", "0"])
        .env("MOCK_STELLAR_INVOKE_FAIL_COUNT", "50")
        .env("MOCK_STELLAR_INVOKE_FAIL_MODE", "permanent")
        .env(
            "MOCK_STELLAR_FAIL_COUNT_FILE",
            fail_count_file.to_str().unwrap(),
        )
        .assert();
    let output = assert.get_output();
    assert_eq!(
        output.status.code(),
        Some(EXIT_NETWORK_FAILURE),
        "simulations with no metrics should exit 6\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No successful simulations to report."),
        "network failure should explain that no metrics were produced: {stderr}"
    );
}

#[test]
fn exit_code_regression_when_baseline_is_exceeded() {
    let workspace = setup_mock_workspace();
    fs::write(
        workspace.path().join("budget-baseline.toml"),
        "[\"mock-contract-a::ping\"]\n\
         cpu_instructions = 100000\n\
         read_bytes = 2048\n\
         write_bytes = 4096\n\
         \n\
         [\"mock-contract-b::pong\"]\n\
         cpu_instructions = 1000000\n\
         read_bytes = 2048\n\
         write_bytes = 4096\n",
    )
    .expect("failed to write baseline");

    let assert = budget_report_cmd(workspace.path())
        .args(BASE_ARGS)
        .args(["--check-baseline", "budget-baseline.toml"])
        .assert();
    let output = assert.get_output();
    assert_eq!(
        output.status.code(),
        Some(EXIT_REGRESSION),
        "baseline regression should exit 5\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("regressions: 1"),
        "regression summary should be human-readable: {stdout}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Simulating function 'ping'..."),
        "stderr should retain the simulation progress message: {stderr}"
    );
    assert!(
        !stderr.contains("error:"),
        "regression should not be reported as a generic error: {stderr}"
    );
}

// ── Issue: --profile must reach the cargo build argv ────────────────────────

/// The `--profile` flag is spliced straight into the child `cargo build`
/// argv at `src/main.rs:1990-1999`. This test asserts it actually arrives
/// there, using the existing fake-binary / mock-workspace harness. If the
/// `"--profile", build_profile` args are ever dropped from the build
/// command, this test fails.
fn read_captured_build_argv(workspace: &tempfile::TempDir) -> Vec<String> {
    let path = workspace.path().join(".mock_cargo_build_argv.txt");
    let content = fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "cargo build argv capture file not found at {}",
            path.display()
        )
    });
    content.lines().map(|l| l.to_string()).collect()
}

#[test]
fn build_argv_carries_profile_when_flag_given() {
    let workspace = setup_mock_workspace();

    budget_report_cmd(workspace.path())
        .args(BASE_ARGS)
        .args(["--profile", "release-opt"])
        .assert()
        .success();

    let argv = read_captured_build_argv(&workspace);
    assert!(
        argv.iter()
            .any(|line| line.contains("--profile") && line.contains("release-opt")),
        "expected at least one cargo build argv line with --profile release-opt, got: {argv:?}"
    );
    // Every --profile entry must be the explicit value, not the default.
    assert!(
        argv.iter()
            .all(|line| !line.contains("--profile") || line.contains("release-opt")),
        "unexpected default profile line alongside explicit profile: {argv:?}"
    );
}

#[test]
fn build_argv_carries_default_profile_when_flag_absent() {
    let workspace = setup_mock_workspace();

    budget_report_cmd(workspace.path())
        .args(BASE_ARGS)
        .assert()
        .success();

    let argv = read_captured_build_argv(&workspace);
    assert!(
        argv.iter()
            .any(|line| line.contains("--profile") && line.contains("release")),
        "expected at least one cargo build argv line with default --profile release, got: {argv:?}"
    );
    assert!(
        !argv.iter().any(|line| line.contains("release-opt")),
        "release-opt should not appear when --profile was not given: {argv:?}"
    );
}
