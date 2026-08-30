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
    setup_fixture_workspace("mock_workspace")
}

/// Copies the named fixture workspace under `tests/fixtures/` into a fresh
/// tempdir. Used for fixtures other than the default mock workspace (e.g.
/// `no_exports_workspace`, whose crates deliberately produce nothing
/// simulatable).
fn setup_fixture_workspace(name: &str) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("failed to create tempdir");
    copy_dir_all(
        &manifest_dir().join("tests/fixtures").join(name),
        tmp.path(),
    );
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

/// A `PATH` containing only the fake `stellar` script and a `bash` symlink,
/// with `curl` (and everything else on the real `PATH`) absent, so
/// `run_preflight_checks` sees `stellar` but cannot find `curl`.
///
/// `bash` must be reachable because the fake `stellar` script's
/// `#!/usr/bin/env bash` shebang resolves it via `PATH`, not an absolute
/// path. Symlinking whatever `bash` this machine actually has (rather than
/// filtering the real `PATH` down to e.g. `/bin`) keeps the test portable:
/// on some Linux distributions `/bin` is a symlink to `/usr/bin`, which
/// would pull `curl` back in alongside `bash`.
fn stellar_only_path_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    fs::copy(fake_bin_dir().join("stellar"), dir.path().join("stellar"))
        .expect("failed to copy fake stellar script");

    #[cfg(unix)]
    {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let perms = fs::Permissions::from_mode(0o755);
        fs::set_permissions(dir.path().join("stellar"), perms)
            .expect("failed to set fake stellar script permissions");

        let real_bash = ["/bin/bash", "/usr/bin/bash"]
            .into_iter()
            .map(PathBuf::from)
            .find(|p| p.exists())
            .expect("no bash found at /bin/bash or /usr/bin/bash");
        symlink(&real_bash, dir.path().join("bash")).expect("failed to symlink bash");
    }

    dir
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

// ── Preflight check integration tests ───────────────────────────────────

#[test]
fn preflight_fails_fast_when_curl_is_missing() {
    let workspace = setup_mock_workspace();
    let stellar_only = stellar_only_path_dir();

    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary should be built");
    cmd.current_dir(workspace.path())
        .env("PATH", stellar_only.path())
        .args(["budget-report", "--network", "local", "--source", "alice"]);

    let assert = cmd.assert();
    let output = assert.failure().get_output().clone();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains("curl is not installed"),
        "stderr should report missing curl, got: {stderr:?}"
    );
    // Preflight must fail before any build is attempted.
    assert!(
        !stderr.contains("Building package"),
        "curl check should run before build, got: {stderr:?}"
    );
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
    // The mock error is a friendbot rate-limit, so the guidance should be
    // the rate-limit one — not the old one-size-fits-all "ensure your
    // source account is funded" line.
    assert!(
        stderr.contains("rate limiting") && stderr.contains("60 seconds"),
        "rate-limit failures should get rate-limit guidance with a wait: {stderr:?}"
    );
    // Each backoff should have reported which failure it was waiting on.
    assert!(
        stderr.contains("Deploy attempt 1/4 failed: ")
            && stderr.contains("rate-limited")
            && stderr.contains("Retrying in"),
        "each retry should report the reason it is retrying: {stderr:?}"
    );
}

#[test]
fn unfunded_account_deploy_failure_gets_account_guidance() {
    let workspace = setup_mock_workspace();
    let fail_count_file = workspace.path().join(".mock_stellar_fail_count_unfunded");
    let _ = fs::remove_file(&fail_count_file);

    let assert = budget_report_cmd(workspace.path())
        .args(["budget-report", "--network", "testnet", "--source", "alice"])
        .env("MOCK_STELLAR_FAIL_COUNT", "10")
        .env(
            "MOCK_STELLAR_FAIL_COUNT_FILE",
            fail_count_file.to_str().unwrap(),
        )
        .env(
            "MOCK_STELLAR_DEPLOY_ERROR",
            "error: transaction submission failed: txInsufficientBalance (source account underfunded)",
        )
        .assert();

    let stderr =
        String::from_utf8_lossy(&assert.failure().get_output().stderr.clone()).into_owned();
    assert!(
        stderr.contains("source account 'alice' is missing or unfunded on testnet"),
        "an unfunded-account failure names the identity and network: {stderr}"
    );
    assert!(
        stderr.contains("stellar keys fund alice --network testnet")
            && stderr.contains("not resolve by waiting"),
        "and gives the exact fix: {stderr}"
    );
}

#[test]
fn unreachable_network_deploy_failure_gets_connectivity_guidance() {
    let workspace = setup_mock_workspace();
    let fail_count_file = workspace.path().join(".mock_stellar_fail_count_net");
    let _ = fs::remove_file(&fail_count_file);

    let assert = budget_report_cmd(workspace.path())
        .args(["budget-report", "--network", "testnet", "--source", "alice"])
        .env("MOCK_STELLAR_FAIL_COUNT", "10")
        .env(
            "MOCK_STELLAR_FAIL_COUNT_FILE",
            fail_count_file.to_str().unwrap(),
        )
        .env(
            "MOCK_STELLAR_DEPLOY_ERROR",
            "error sending request: connection reset by peer",
        )
        .assert();

    let stderr =
        String::from_utf8_lossy(&assert.failure().get_output().stderr.clone()).into_owned();
    assert!(
        stderr.contains("network could not be reached"),
        "a connectivity failure is distinguished from rate limiting: {stderr}"
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

// ── End-to-end offline test ─────────────────────────────────────────────

/// Comprehensive end-to-end test that exercises the complete pipeline:
/// configuration resolution → contract build → export discovery →
/// simulation → report rendering, entirely offline without network access
/// or stellar CLI.
///
/// Expected runtime: <2s (no network, no real stellar CLI, instant mock builds)
///
/// This test validates:
/// 1. Configuration loading (network, source from CLI args)
/// 2. Workspace discovery and contract enumeration
/// 3. WASM export parsing to find callable functions
/// 4. Contract deployment and function simulation (via fake binaries)
/// 5. Metric extraction from simulation responses
/// 6. Full report rendering in both text and JSON formats
/// 7. Budget checking with configured limits
///
/// Failure modes are explicit:
/// - Missing header → configuration or discovery stage failed
/// - Missing package name → export discovery or simulation failed
/// - Missing metric → metric extraction failed
/// - Check failure → budget validation logic broken
///
/// Uses existing mock infrastructure (fake_bin/stellar and fake_bin/curl)
/// prepended to PATH, so no network access or real stellar installation required.
/// Passes on fork PRs without any external dependencies.
#[test]
fn end_to_end_offline_full_pipeline() {
    let workspace = setup_mock_workspace();

    // Create a budget.toml with specific limits to test the check logic
    fs::write(
        workspace.path().join("budget.toml"),
        "network = \"local\"\n\
         source = \"alice\"\n\
         \n\
         [functions.ping]\n\
         cpu_limit = 5000000\n\
         read_limit = 5000\n\
         write_limit = 5000\n\
         \n\
         [functions.pong]\n\
         cpu_limit = 2000000\n\
         read_limit = 3000\n\
         write_limit = 5000\n\
         \n\
         [functions.greet]\n\
         cpu_limit = 5000000\n\
         read_limit = 5000\n\
         write_limit = 5000\n",
    )
    .expect("failed to write budget.toml");

    // ── Stage 1: Basic report generation (text format) ──────────────────

    let assert = budget_report_cmd(workspace.path())
        .args(["budget-report"])
        .assert();

    let output = assert.success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Configuration stage: report header should be present
    assert!(
        stdout.contains("WORKSPACE BUDGET REPORT"),
        "Stage FAILED: Configuration/Discovery - missing report header, got: {stdout}"
    );

    // Export discovery stage: all three contract packages should be found
    assert!(
        stdout.contains("mock-contract-a"),
        "Stage FAILED: Export Discovery - mock-contract-a not found, got: {stdout}"
    );
    assert!(
        stdout.contains("mock-contract-b"),
        "Stage FAILED: Export Discovery - mock-contract-b not found, got: {stdout}"
    );
    assert!(
        stdout.contains("mock-contract-renamed"),
        "Stage FAILED: Export Discovery - mock-contract-renamed not found, got: {stdout}"
    );

    // Simulation stage: function names should appear in the report
    assert!(
        stdout.contains("ping"),
        "Stage FAILED: Simulation - ping function not simulated, got: {stdout}"
    );
    assert!(
        stdout.contains("pong"),
        "Stage FAILED: Simulation - pong function not simulated, got: {stdout}"
    );
    assert!(
        stdout.contains("greet"),
        "Stage FAILED: Simulation - greet function not simulated, got: {stdout}"
    );

    // Metric extraction stage: all three metrics should be present
    assert!(
        stdout.contains("CPU Instructions"),
        "Stage FAILED: Metric Extraction - CPU Instructions metric missing, got: {stdout}"
    );
    assert!(
        stdout.contains("Read Bytes"),
        "Stage FAILED: Metric Extraction - Read Bytes metric missing, got: {stdout}"
    );
    assert!(
        stdout.contains("Write Bytes"),
        "Stage FAILED: Metric Extraction - Write Bytes metric missing, got: {stdout}"
    );

    // Report rendering stage: formatted values should use commas and units
    assert!(
        stdout.contains("1,000,000 inst."),
        "Stage FAILED: Report Rendering - formatted CPU value missing, got: {stdout}"
    );
    assert!(
        stdout.contains("2,048 B"),
        "Stage FAILED: Report Rendering - formatted read bytes missing, got: {stdout}"
    );
    assert!(
        stdout.contains("4,096 B"),
        "Stage FAILED: Report Rendering - formatted write bytes missing, got: {stdout}"
    );

    // ── Stage 2: JSON output format ──────────────────────────────────────

    let json_assert = budget_report_cmd(workspace.path())
        .args(["budget-report", "--json"])
        .assert();

    let json_output = json_assert.success().get_output().clone();
    let json_stdout = String::from_utf8_lossy(&json_output.stdout);
    let reports: serde_json::Value =
        serde_json::from_str(&json_stdout).expect("Stage FAILED: JSON serialization invalid");
    // `--json` emits the versioned envelope `{schema_version, snapshots}`;
    // the rows themselves are unchanged from the pre-versioning format.
    assert_eq!(
        reports["schema_version"].as_u64(),
        Some(1),
        "Stage FAILED: JSON output should carry schema_version 1"
    );
    let reports_array = reports["snapshots"]
        .as_array()
        .expect("Stage FAILED: JSON output should have a `snapshots` array");

    // JSON should contain entries for all three packages
    let packages: std::collections::HashSet<&str> = reports_array
        .iter()
        .map(|r| r["package"].as_str().expect("package should be a string"))
        .collect();
    assert!(
        packages.contains("mock-contract-a"),
        "Stage FAILED: JSON output - mock-contract-a missing"
    );
    assert!(
        packages.contains("mock-contract-b"),
        "Stage FAILED: JSON output - mock-contract-b missing"
    );
    assert!(
        packages.contains("mock-contract-renamed"),
        "Stage FAILED: JSON output - mock-contract-renamed missing"
    );

    // Validate metric values are correct for mock-contract-a::ping
    let cpu_entry = reports_array
        .iter()
        .find(|r| r["package"] == "mock-contract-a" && r["metric"] == "CPU Instructions")
        .expect("Stage FAILED: JSON - CPU entry for mock-contract-a not found");
    assert_eq!(
        cpu_entry["value"], 1_000_000,
        "Stage FAILED: Metric Extraction - incorrect CPU value in JSON"
    );

    let read_entry = reports_array
        .iter()
        .find(|r| r["package"] == "mock-contract-a" && r["metric"] == "Read Bytes")
        .expect("Stage FAILED: JSON - Read Bytes entry for mock-contract-a not found");
    assert_eq!(
        read_entry["value"], 2048,
        "Stage FAILED: Metric Extraction - incorrect read bytes value in JSON"
    );

    let write_entry = reports_array
        .iter()
        .find(|r| r["package"] == "mock-contract-a" && r["metric"] == "Write Bytes")
        .expect("Stage FAILED: JSON - Write Bytes entry for mock-contract-a not found");
    assert_eq!(
        write_entry["value"], 4096,
        "Stage FAILED: Metric Extraction - incorrect write bytes value in JSON"
    );

    // ── Stage 3: Budget checking with configured limits ─────────────────

    let check_assert = budget_report_cmd(workspace.path())
        .args(["budget-report", "--check"])
        .assert();

    let check_output = check_assert.success().get_output().clone();
    let check_stdout = String::from_utf8_lossy(&check_output.stdout);

    // Check summary should be present
    assert!(
        check_stdout.contains("=== BUDGET CHECKS ==="),
        "Stage FAILED: Budget Check - missing check header, got: {check_stdout}"
    );

    // Should show summary with passed/failed counts
    // We expect 9 checks total (3 functions × 3 metrics each)
    // All should pass since limits are generous
    assert!(
        check_stdout.contains("Summary:"),
        "Stage FAILED: Budget Check - missing summary line, got: {check_stdout}"
    );
    assert!(
        check_stdout.contains("9 check(s) passed"),
        "Stage FAILED: Budget Check - expected 9 passing checks, got: {check_stdout}"
    );
    assert!(
        check_stdout.contains("0 failed"),
        "Stage FAILED: Budget Check - expected 0 failures, got: {check_stdout}"
    );

    // ── Stage 4: Budget checking with exceeded limits ───────────────────

    // Rewrite budget.toml with a CPU limit that ping will exceed
    fs::write(
        workspace.path().join("budget.toml"),
        "network = \"local\"\n\
         source = \"alice\"\n\
         \n\
         [functions.ping]\n\
         cpu_limit = 100\n",
    )
    .expect("failed to write budget.toml with tight limit");

    let fail_check = budget_report_cmd(workspace.path())
        .args(["budget-report", "--check"])
        .assert();

    let fail_output = fail_check.failure().get_output().clone();
    let fail_stdout = String::from_utf8_lossy(&fail_output.stdout);

    // Should show FAIL status for the exceeded limit
    assert!(
        fail_stdout.contains("FAIL"),
        "Stage FAILED: Budget Check - should show FAIL for exceeded limit, got: {fail_stdout}"
    );
    assert!(
        fail_stdout.contains("mock-contract-a::ping"),
        "Stage FAILED: Budget Check - should identify failing function, got: {fail_stdout}"
    );
    assert!(
        fail_stdout.contains("CPU Instructions"),
        "Stage FAILED: Budget Check - should identify failing metric, got: {fail_stdout}"
    );

    // Should show at least 1 failure in summary
    assert!(
        fail_stdout.contains("1 failed") || fail_stdout.contains("failed"),
        "Stage FAILED: Budget Check - summary should show failures, got: {fail_stdout}"
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

#[test]
fn mainnet_is_refused_and_nothing_is_built() {
    let workspace = setup_mock_workspace();

    let assert = budget_report_cmd(workspace.path())
        .args(["budget-report", "--network", "mainnet", "--source", "alice"])
        .assert();

    let output = assert.failure().get_output().clone();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Mainnet"), "names the network: {stderr}");
    assert!(
        stderr.contains("--allow-mainnet"),
        "says how to proceed: {stderr}"
    );
    // The guard runs before workspace discovery, so no package is built.
    assert!(
        !stderr.contains("Building package"),
        "guard must stop the run before building: {stderr}"
    );
}

#[test]
fn unrecognised_network_is_refused_without_opt_in() {
    let workspace = setup_mock_workspace();

    let assert = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "some-private-net",
            "--source",
            "alice",
        ])
        .assert();

    let stderr =
        String::from_utf8_lossy(&assert.failure().get_output().stderr.clone()).into_owned();
    assert!(
        stderr.contains("unrecognised network"),
        "an unknown network is treated as unsafe: {stderr}"
    );
}

#[test]
fn mainnet_with_opt_in_proceeds() {
    let workspace = setup_mock_workspace();

    let assert = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "mainnet",
            "--source",
            "alice",
            "--allow-mainnet",
        ])
        .assert();

    let stdout =
        String::from_utf8_lossy(&assert.success().get_output().stdout.clone()).into_owned();
    assert!(
        stdout.contains("WORKSPACE BUDGET REPORT"),
        "with --allow-mainnet the run proceeds normally: {stdout}"
    );
}

#[test]
fn testnet_is_unaffected_by_the_guard() {
    let workspace = setup_mock_workspace();

    let assert = budget_report_cmd(workspace.path())
        .args(["budget-report", "--network", "testnet", "--source", "alice"])
        .assert();

    assert.success().stdout(contains("WORKSPACE BUDGET REPORT"));
}

#[test]
fn contract_that_exports_nothing_reports_the_specific_cause() {
    // The fixture workspace has one crate per failure mode; the fixture wasm
    // is built here rather than checked in.
    let workspace = setup_fixture_workspace("no_exports_workspace");

    let assert = budget_report_cmd(workspace.path())
        .args(["budget-report", "--network", "local", "--source", "alice"])
        .assert();

    let output = assert.failure().get_output().clone();
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Cause 1: a soroban-sdk crate that is not a cdylib.
    assert!(
        stderr.contains("helper-only") && stderr.contains("cdylib"),
        "names the non-cdylib crate and what to change: {stderr}"
    );
    // Cause 2: a cdylib with no function exports.
    assert!(
        stderr.contains("no-exports") && stderr.contains("no function exports"),
        "distinguishes 'no exports at all': {stderr}"
    );
    // Cause 3: a cdylib exporting only toolchain symbols, and it lists them.
    assert!(
        stderr.contains("runtime-only")
            && stderr.contains("calling convention")
            && stderr.contains("_start"),
        "distinguishes 'exports present, none simulatable' and lists what was found: {stderr}"
    );
    // The vague pre-existing message is gone.
    assert!(
        !stderr.contains("No exported functions found in"),
        "the old undifferentiated message should not appear: {stderr}"
    );
}

#[test]
fn check_baseline_markdown_renders_a_diff_table() {
    let workspace = setup_mock_workspace();

    // A hand-written baseline: ping's cpu is well under the ~1,000,000 the
    // fake RPC reports, so it must show as a tolerance breach; pong matches
    // exactly, so it is unchanged.
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
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--check-baseline",
            "budget-baseline.toml",
            "--markdown",
        ])
        .assert();

    // A regression exits non-zero.
    let stdout =
        String::from_utf8_lossy(&assert.failure().get_output().stdout.clone()).into_owned();

    // Valid GitHub pipe table.
    assert!(stdout
        .contains("| Function | Metric | Baseline | Current | Change | Change % | Dir | Status |"));
    assert!(stdout.contains("|---|---|--:|--:|--:|--:|:-:|:--|"));
    // ping's cpu breached; the status names the ceiling and it is not colour.
    assert!(
        stdout.contains("`mock-contract-a::ping`") && stdout.contains("BREACH (max 110,000)"),
        "cpu breach row present: {stdout}"
    );
    assert!(!stdout.contains('\u{1b}'), "no ANSI colour codes: {stdout}");
    // pong is unchanged -> collapsed into <details> by default.
    assert!(
        stdout.contains("<details>") && stdout.contains("unchanged metric(s)"),
        "unchanged rows collapsed: {stdout}"
    );
}
