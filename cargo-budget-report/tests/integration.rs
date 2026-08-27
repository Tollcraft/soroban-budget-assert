//! End-to-end integration tests for `cargo budget-report`.
//!
//! These run the real compiled binary (via `assert_cmd`) against the
//! isolated, deterministic mock workspace in `tests/fixtures/mock_workspace`
//! rather than against Tollcraft's own contracts, which made prior ad-hoc
//! testing brittle and circular.
//!
//! The mock workspace's two contracts are bare `no_std` WASM exports with no
//! dependencies (not real Soroban contracts), so `cargo build` for them is
//! near-instant. `cargo-budget-report` still shells out to the real `stellar`
//! CLI and `curl` to deploy/simulate, so both are replaced with deterministic
//! scripts in `tests/fixtures/fake_bin` (prepended to `PATH`
//! for the child process). This keeps the suite offline and reproducible:
//! no live network call, no funded/configured Stellar identity required.

use assert_cmd::Command;
use predicates::str::contains;
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

/// Recursively copies `src` into `dst`, creating `dst` if needed.
///
/// The mock workspace is copied into a fresh tempdir per test because
/// `cargo build` writes `Cargo.lock` and `target/` into its working
/// directory; running in place would leave build artifacts next to the
/// checked-in fixture.
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

/// Copies the mock workspace fixture into a fresh tempdir and returns it.
fn setup_mock_workspace() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("failed to create tempdir");
    copy_dir_all(&mock_workspace_fixture(), tmp.path());
    tmp
}

/// `PATH` with the fake `stellar`/`curl` scripts prepended so the CLI under
/// test resolves them ahead of (or instead of) any real installation, while
/// still finding the real `cargo`/`rustc` used to build the mock contracts.
fn mocked_path() -> String {
    let real_path = std::env::var("PATH").unwrap_or_default();
    format!("{}:{}", fake_bin_dir().display(), real_path)
}

/// Builds a ready-to-run `Command` for the compiled `cargo-budget-report`
/// binary, with its cwd set to `dir` and `PATH` mocked as above.
fn budget_report_cmd(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary should be built");
    cmd.current_dir(dir).env("PATH", mocked_path());
    cmd
}

#[test]
fn discovers_mock_workspace_and_reports_cleanly() {
    let workspace = setup_mock_workspace();

    let assert = budget_report_cmd(workspace.path())
        .args(["budget-report", "--network", "local", "--source", "alice"])
        .assert();

    let output = assert.success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("WORKSPACE BUDGET REPORT"),
        "expected report header, got: {stdout}"
    );
    assert!(stdout.contains("mock-contract-a"), "got: {stdout}");
    assert!(stdout.contains("mock-contract-b"), "got: {stdout}");
    assert!(stdout.contains("mock-contract-renamed"), "got: {stdout}");
    assert!(stdout.contains("CPU Instructions"), "got: {stdout}");
    assert!(stdout.contains("1,000,000 inst."), "got: {stdout}");
    assert!(stdout.contains("2,048 B"), "got: {stdout}");
    assert!(stdout.contains("4,096 B"), "got: {stdout}");
}

#[test]
fn json_output_reports_both_mock_contracts() {
    let workspace = setup_mock_workspace();

    let assert = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--json",
        ])
        .assert();

    let output = assert.success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let doc: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    // The --json output is the versioned envelope {schema_version, snapshots}.
    assert_eq!(doc["schema_version"], 1, "schema_version should be 1");
    let reports = doc["snapshots"]
        .as_array()
        .expect("snapshots should be a JSON array");

    let packages: std::collections::HashSet<&str> = reports
        .iter()
        .map(|r| r["package"].as_str().expect("package should be a string"))
        .collect();
    assert!(packages.contains("mock-contract-a"), "got: {reports:?}");
    assert!(packages.contains("mock-contract-b"), "got: {reports:?}");
    assert!(
        packages.contains("mock-contract-renamed"),
        "got: {reports:?}"
    );

    let cpu_entry = reports
        .iter()
        .find(|r| r["package"] == "mock-contract-a" && r["metric"] == "CPU Instructions")
        .expect("CPU Instructions entry for mock-contract-a should be present");
    assert_eq!(cpu_entry["value"], 1_000_000);

    let wasm_bytes_entry = reports
        .iter()
        .find(|r| r["package"] == "mock-contract-b" && r["metric"] == "WASM Bytes")
        .expect("WASM Bytes entry for mock-contract-b should be present");
    assert!(
        wasm_bytes_entry["value"].as_u64().unwrap_or(0) > 0,
        "got: {wasm_bytes_entry:?}"
    );
}

#[test]
fn check_flag_passes_when_limits_are_generous() {
    let workspace = setup_mock_workspace();
    fs::write(
        workspace.path().join("budget.toml"),
        "[functions.ping]\n\
         cpu_limit = 5000000\n\
         read_limit = 5000\n\
         write_limit = 5000\n\
         \n\
         [functions.pong]\n\
         cpu_limit = 5000000\n\
         read_limit = 5000\n\
         write_limit = 5000\n",
    )
    .expect("failed to write budget.toml");

    let assert = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--check",
        ])
        .assert();

    let output = assert.success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("=== BUDGET CHECKS ==="), "got: {stdout}");
    // Only ping (3 metrics) + pong (3 metrics) are configured in budget.toml.
    // The new greet function has no config, so it adds 0 checks.
    // WASM Bytes are not checked because limit_for_metric returns None.
    assert!(
        stdout.contains("Summary: 6 check(s) passed, 0 failed"),
        "got: {stdout}"
    );
}

#[test]
fn check_flag_fails_when_a_limit_is_exceeded() {
    let workspace = setup_mock_workspace();
    fs::write(
        workspace.path().join("budget.toml"),
        "[functions.ping]\n\
         cpu_limit = 10\n\
         \n\
         [functions.pong]\n\
         cpu_limit = 5000000\n",
    )
    .expect("failed to write budget.toml");

    let mut cmd = budget_report_cmd(workspace.path());
    cmd.args([
        "budget-report",
        "--network",
        "local",
        "--source",
        "alice",
        "--check",
    ]);

    cmd.assert()
        .failure()
        .stdout(contains("mock-contract-a::ping [CPU Instructions]"))
        .stdout(contains("FAIL"));
}

#[test]
fn html_output_renders_both_mock_contracts() {
    let workspace = setup_mock_workspace();

    let assert = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--html",
        ])
        .assert();

    let output = assert.success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.starts_with("<!doctype html>"),
        "expected an HTML document, got: {stdout}"
    );
    assert!(stdout.contains("mock-contract-a"), "got: {stdout}");
    assert!(stdout.contains("mock-contract-b"), "got: {stdout}");
    assert!(stdout.contains("mock-contract-renamed"), "got: {stdout}");
    assert!(stdout.contains("CPU Instructions"), "got: {stdout}");
    // Thousands separators for the values the fake RPC returns.
    assert!(stdout.contains("1,000,000"), "got: {stdout}");
    assert!(stdout.contains("2,048"), "got: {stdout}");
    // The page must be fully self-contained: no linked CSS or external scripts.
    assert!(
        !stdout.contains("<link"),
        "page must not link external CSS: {stdout}"
    );
    assert!(
        !stdout.contains("<script src"),
        "page must not load external scripts: {stdout}"
    );
}

#[test]
fn html_output_check_mode_shows_pass_and_fail_rows() {
    let workspace = setup_mock_workspace();
    fs::write(
        workspace.path().join("budget.toml"),
        "[functions.ping]\n\
         cpu_limit = 10\n\
         read_limit = 5000\n\
         write_limit = 5000\n\
         \n\
         [functions.pong]\n\
         cpu_limit = 5000000\n",
    )
    .expect("failed to write budget.toml");

    let assert = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--html",
            "--check",
        ])
        .assert();

    // ping's CPU limit (10) is breached, so `--check` exits non-zero.
    let output = assert.failure().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("&#10007; FAIL"), "got: {stdout}");
    assert!(stdout.contains("&#10003; PASS"), "got: {stdout}");
}

// ── Retry mechanism integration tests ───────────────────────────────────

#[test]
fn retry_mechanism_succeeds_after_transient_deploy_failures() {
    let workspace = setup_mock_workspace();
    let fail_count_file = workspace.path().join(".mock_stellar_fail_count");
    let _ = fs::remove_file(&fail_count_file);

    let assert = budget_report_cmd(workspace.path())
        .args(["budget-report", "--network", "local", "--source", "alice"])
        .env("MOCK_STELLAR_FAIL_COUNT", "3")
        .env(
            "MOCK_STELLAR_FAIL_COUNT_FILE",
            fail_count_file.to_str().unwrap(),
        )
        .assert();

    let output = assert.success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should still produce a valid report even after 3 retries.
    assert!(
        stdout.contains("WORKSPACE BUDGET REPORT"),
        "report should succeed after transient failures, got: {stdout}"
    );
    assert!(stdout.contains("mock-contract-a"), "got: {stdout}");

    // The stderr should contain retry messages.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Retrying in"),
        "stderr should contain retry messages, got: {stderr:?}"
    );
}

#[test]
fn retry_mechanism_fails_after_exhausting_all_attempts() {
    let workspace = setup_mock_workspace();
    let fail_count_file = workspace.path().join(".mock_stellar_fail_count_2");
    let _ = fs::remove_file(&fail_count_file);

    let assert = budget_report_cmd(workspace.path())
        .args(["budget-report", "--network", "local", "--source", "alice"])
        .env("MOCK_STELLAR_FAIL_COUNT", "10")
        .env(
            "MOCK_STELLAR_FAIL_COUNT_FILE",
            fail_count_file.to_str().unwrap(),
        )
        .assert();

    let output = assert.failure().get_output().clone();
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should fail after MAX_DEPLOY_ATTEMPTS attempts.
    assert!(
        stderr.contains("after 4 attempts"),
        "stderr should mention exhausted retries, got: {stderr:?}"
    );
    assert!(
        stderr.contains("source account is funded"),
        "stderr should mention source account funding, got: {stderr:?}"
    );
}

// ── Configurable retry policy integration tests ─────────────────────────

/// `--max-retry-attempts 1` must disable retry entirely: a transient
/// deploy failure fails the run on the first attempt with no backoff.
#[test]
fn max_retry_attempts_one_disables_retry_via_cli() {
    let workspace = setup_mock_workspace();
    let fail_count_file = workspace.path().join(".mock_stellar_fail_count_disabled");
    let _ = fs::remove_file(&fail_count_file);

    let output = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--max-retry-attempts",
            "1",
        ])
        .env("MOCK_STELLAR_FAIL_COUNT", "3")
        .env(
            "MOCK_STELLAR_FAIL_COUNT_FILE",
            fail_count_file.to_str().unwrap(),
        )
        .assert()
        .failure()
        .get_output()
        .clone();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("Retrying in"),
        "retry disabled must not print retry messages, got: {stderr:?}"
    );
    assert!(
        stderr.contains("after 1 attempts"),
        "should report exhaustion after exactly one attempt, got: {stderr:?}"
    );
}

/// `[retry] max_attempts = 1` in budget.toml also disables retry.
#[test]
fn max_retry_attempts_one_disables_retry_via_budget_toml() {
    let workspace = setup_mock_workspace();
    fs::write(
        workspace.path().join("budget.toml"),
        "[retry]\nmax_attempts = 1\n",
    )
    .expect("failed to write budget.toml");
    let fail_count_file = workspace.path().join(".mock_stellar_fail_count_toml_off");
    let _ = fs::remove_file(&fail_count_file);

    let output = budget_report_cmd(workspace.path())
        .args(["budget-report", "--network", "local", "--source", "alice"])
        .env("MOCK_STELLAR_FAIL_COUNT", "3")
        .env(
            "MOCK_STELLAR_FAIL_COUNT_FILE",
            fail_count_file.to_str().unwrap(),
        )
        .assert()
        .failure()
        .get_output()
        .clone();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("Retrying in"),
        "retry disabled must not print retry messages, got: {stderr:?}"
    );
}

/// budget.toml `[retry].max_attempts` is honored when no CLI flag is given:
/// two configured attempts against three failures exhausts the budget.
#[test]
fn budget_toml_retry_max_attempts_is_respected() {
    let workspace = setup_mock_workspace();
    fs::write(
        workspace.path().join("budget.toml"),
        "[retry]\nmax_attempts = 2\ninitial_backoff_secs = 0\n",
    )
    .expect("failed to write budget.toml");
    let fail_count_file = workspace.path().join(".mock_stellar_fail_count_toml_2");
    let _ = fs::remove_file(&fail_count_file);

    let output = budget_report_cmd(workspace.path())
        .args(["budget-report", "--network", "local", "--source", "alice"])
        .env("MOCK_STELLAR_FAIL_COUNT", "10")
        .env(
            "MOCK_STELLAR_FAIL_COUNT_FILE",
            fail_count_file.to_str().unwrap(),
        )
        .assert()
        .failure()
        .get_output()
        .clone();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("after 2 attempts"),
        "budget.toml max_attempts = 2 should bound attempts, got: {stderr:?}"
    );
}

/// The CLI flag overrides the budget.toml `[retry]` section: the config
/// says retry is off, but `--max-retry-attempts 2` re-enables one retry,
/// which is enough for the single transient failure.
#[test]
fn cli_max_retry_attempts_overrides_budget_toml() {
    let workspace = setup_mock_workspace();
    fs::write(
        workspace.path().join("budget.toml"),
        "[retry]\nmax_attempts = 1\n",
    )
    .expect("failed to write budget.toml");
    let fail_count_file = workspace.path().join(".mock_stellar_fail_count_cli_wins");
    let _ = fs::remove_file(&fail_count_file);

    let output = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--max-retry-attempts",
            "2",
            "--retry-backoff-secs",
            "0",
        ])
        .env("MOCK_STELLAR_FAIL_COUNT", "1")
        .env(
            "MOCK_STELLAR_FAIL_COUNT_FILE",
            fail_count_file.to_str().unwrap(),
        )
        .assert()
        .success()
        .get_output()
        .clone();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("WORKSPACE BUDGET REPORT"),
        "CLI override should re-enable retry and succeed, got: {stdout}"
    );
}

/// Retry applies to the invoke-build call site too: a transient
/// (`connection reset by peer`) invoke failure is retried and the run
/// still succeeds.
#[test]
fn transient_invoke_build_failure_is_retried_and_recovers() {
    let workspace = setup_mock_workspace();
    let fail_count_file = workspace.path().join(".mock_stellar_invoke_fail_transient");
    let _ = fs::remove_file(&fail_count_file);

    let output = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--retry-backoff-secs",
            "0",
        ])
        .env("MOCK_STELLAR_INVOKE_FAIL_COUNT", "2")
        .env(
            "MOCK_STELLAR_FAIL_COUNT_FILE",
            fail_count_file.to_str().unwrap(),
        )
        .assert()
        .success()
        .get_output()
        .clone();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("WORKSPACE BUDGET REPORT"),
        "report should succeed after transient invoke failures, got: {stdout}"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Invoke build attempt 1/4 failed"),
        "stderr should show invoke-build retry progress, got: {stderr:?}"
    );
}

/// A deterministic invoke failure (contract does not exist) must NOT be
/// retried: the run fails on the first attempt with no retry messages.
#[test]
fn permanent_invoke_build_failure_is_not_retried() {
    let workspace = setup_mock_workspace();
    let fail_count_file = workspace.path().join(".mock_stellar_invoke_fail_permanent");
    let _ = fs::remove_file(&fail_count_file);

    let output = budget_report_cmd(workspace.path())
        .args(["budget-report", "--network", "local", "--source", "alice"])
        .env("MOCK_STELLAR_INVOKE_FAIL_COUNT", "5")
        .env("MOCK_STELLAR_INVOKE_FAIL_MODE", "permanent")
        .env(
            "MOCK_STELLAR_FAIL_COUNT_FILE",
            fail_count_file.to_str().unwrap(),
        )
        .assert()
        .failure()
        .get_output()
        .clone();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("Retrying in"),
        "permanent failures must not be retried, got: {stderr:?}"
    );
    assert!(
        stderr.contains("does not exist"),
        "the underlying deterministic error should surface, got: {stderr:?}"
    );
}

// ── Record / replay transport integration tests ─────────────────────────

#[test]
fn record_then_replay_produces_identical_report() {
    let workspace = setup_mock_workspace();
    let fixture_path = workspace.path().join("fixture.json");

    // First run records every transport response into the fixture. It runs
    // against the fake stellar/curl scripts like the other tests.
    let record = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--record",
            fixture_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .get_output()
        .clone();
    let recorded_stdout = String::from_utf8_lossy(&record.stdout);
    assert!(
        recorded_stdout.contains("WORKSPACE BUDGET REPORT"),
        "recorded run should produce a report, got: {recorded_stdout}"
    );

    // The fixture must exist and contain the expected entry keys.
    let fixture: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&fixture_path).expect("fixture should be written"))
            .expect("fixture should be valid JSON");
    assert_eq!(fixture["fixture_version"], 1);
    let entries = fixture["entries"]
        .as_object()
        .expect("fixture should have an entries object");
    assert!(
        entries.keys().any(|k| k.starts_with("deploy:")),
        "fixture should contain deploy entries, got keys: {:?}",
        entries.keys()
    );
    assert!(
        entries.keys().any(|k| k.starts_with("simulate:")),
        "fixture should contain simulate entries, got keys: {:?}",
        entries.keys()
    );

    // Replay with a PATH that excludes the fake stellar/curl scripts: the
    // whole pipeline must run with no `stellar` CLI and no `curl` at all.
    let mut replay_cmd = Command::cargo_bin("cargo-budget-report").expect("binary should be built");
    replay_cmd
        .current_dir(workspace.path())
        .env("PATH", std::env::var("PATH").unwrap_or_default());
    let replay = replay_cmd
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--replay",
            fixture_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .get_output()
        .clone();
    let replayed_stdout = String::from_utf8_lossy(&replay.stdout);

    assert_eq!(
        recorded_stdout, replayed_stdout,
        "replaying the fixture must reproduce the recorded report byte-for-byte"
    );
}
