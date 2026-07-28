//! End-to-end integration tests for `cargo budget-report`.
//!
//! These run the real compiled binary (via `assert_cmd`) against the
//! isolated, deterministic mock workspace in `tests/fixtures/mock_workspace`
//! rather than against Tollcraft's own contracts.
//!
//! The RPC calls (`uploadContractWasm`, `simulateTransaction`) are mocked
//! via `mockito` so the suite is offline and reproducible: no live network
//! call, no funded/configured Stellar identity required.

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

/// Recursively copies `src` into `dst`, creating `dst` if needed.
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

/// Base64-encoded `SorobanTransactionData` XDR that decodes to
/// instructions=1000000, read_bytes=2048, write_bytes=4096, resource_fee=0
/// — same fixture values used by the unit tests.
const FIXTURE_TRANSACTION_DATA_B64: &str = "AAAAAAAAAAAAAAAAAA9CQAAACAAAABAAAAAAAAAAAAA=";

/// Set up mockito mocks for the RPC endpoints the tool calls.
///
/// Returns the mockito server and mocks so they stay alive for the
/// duration of the test function.
#[allow(clippy::type_complexity)]
fn setup_rpc_mocks() -> (mockito::ServerGuard, Vec<mockito::Mock>) {
    let mut server = mockito::Server::new();

    // Mock uploadContractWasm — returns a dummy hash
    let upload_mock = server
        .mock("POST", "/")
        .match_body(mockito::Matcher::Regex(
            r#""method"\s*:\s*"uploadContractWasm""#.to_string(),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "hash": "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
                }
            })
            .to_string(),
        )
        .expect_at_least(2)
        .create();

    // Mock simulateTransaction — returns the fixture transaction data
    let simulate_mock = server
        .mock("POST", "/")
        .match_body(mockito::Matcher::Regex(
            r#""method"\s*:\s*"simulateTransaction""#.to_string(),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "transactionData": FIXTURE_TRANSACTION_DATA_B64,
                    "minResourceFee": "1000"
                }
            })
            .to_string(),
        )
        .expect_at_least(3)
        .create();

    (server, vec![upload_mock, simulate_mock])
}

/// Builds a ready-to-run `Command` for the compiled `cargo-budget-report`
/// binary, with its cwd set to `dir` and the RPC URL pointed at the mockito server.
fn budget_report_cmd(dir: &Path, rpc_url: &str) -> Command {
    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary should be built");
    // Use a valid Stellar public key (G...) as source
    const SOURCE_PUBKEY: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
    cmd.current_dir(dir).args([
        "budget-report",
        "--network",
        rpc_url,
        "--source",
        SOURCE_PUBKEY,
    ]);
    cmd
}

#[test]
fn discovers_mock_workspace_and_reports_cleanly() {
    let workspace = setup_mock_workspace();
    let (_server, _mocks) = setup_rpc_mocks();
    let rpc_url = _server.url();

    let assert = budget_report_cmd(workspace.path(), &rpc_url).assert();

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
fn function_filter_reports_only_the_selected_function() {
    let workspace = setup_mock_workspace();

    let assert = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--function",
            "ping",
            "--json",
        ])
        .assert();

    let output = assert.success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let reports: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    let reports = reports.as_array().expect("report should be a JSON array");

    assert!(
        !reports.is_empty(),
        "the selected function should be reported"
    );
    assert!(
        reports
            .iter()
            .all(|report| report["package"] == "mock-contract-a"),
        "--function ping should exclude mock-contract-b: {reports:?}"
    );
}

#[test]
fn function_filter_selects_a_function_from_the_other_contract() {
    let workspace = setup_mock_workspace();

    let assert = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--function",
            "pong",
            "--json",
        ])
        .assert();

    let output = assert.success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let reports: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    let reports = reports.as_array().expect("report should be a JSON array");

    assert!(
        !reports.is_empty(),
        "the selected function should be reported"
    );
    assert!(
        reports
            .iter()
            .all(|report| report["package"] == "mock-contract-b"),
        "--function pong should exclude mock-contract-a: {reports:?}"
    );
}

#[test]
fn json_output_reports_both_mock_contracts() {
    let workspace = setup_mock_workspace();
    let (_server, _mocks) = setup_rpc_mocks();
    let rpc_url = _server.url();

    let assert = budget_report_cmd(workspace.path(), &rpc_url)
        .arg("--json")
        .assert();

    let output = assert.success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let reports: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    let reports = reports.as_array().expect("report should be a JSON array");

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
    let (_server, _mocks) = setup_rpc_mocks();
    let rpc_url = _server.url();

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

    let assert = budget_report_cmd(workspace.path(), &rpc_url)
        .arg("--check")
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
    let (_server, _mocks) = setup_rpc_mocks();
    let rpc_url = _server.url();

    fs::write(
        workspace.path().join("budget.toml"),
        "[functions.ping]\n\
         cpu_limit = 10\n\
         \n\
         [functions.pong]\n\
         cpu_limit = 5000000\n",
    )
    .expect("failed to write budget.toml");

    let mut cmd = budget_report_cmd(workspace.path(), &rpc_url);
    cmd.arg("--check");

    cmd.assert()
        .failure()
        .stdout(contains("mock-contract-a::ping [CPU Instructions]"))
        .stdout(contains("FAIL"));
}
