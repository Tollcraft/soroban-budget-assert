//! Integration tests for issue #1017: derived Tier A environment keys use
//! the Cargo package name with hyphens replaced by underscores.
//!
//! The key formatters (`env_var_key` / `env_var_scenario_key`) live in
//! `cargo-budget-report/src/derive.rs` and are private. These tests therefore
//! drive the public `--derive-limits` path end to end: a Tier B report goes
//! in, the emitted `.env` file comes out, and its keys are checked against the
//! documented `TIER_A__<PACKAGE>__<FUNCTION>__<METRIC>` convention.

use assert_cmd::Command;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

fn derive_cmd(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary should be built");
    cmd.current_dir(dir);
    cmd
}

fn env_keys(path: &Path) -> BTreeSet<String> {
    fs::read_to_string(path)
        .expect("derived env file should be readable")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once('=').map(|(key, _)| key.trim().to_string()))
        .filter(|key| key.starts_with("TIER_A__"))
        .collect()
}

#[test]
fn hyphenated_packages_and_scenarios_yield_underscored_keys() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let out = dir.path().join("tier-a-limits.env");

    // Covers a single-hyphen package, a multi-hyphen package, an empty
    // function name (the two-segment form), and a scenario roll-up.
    let report = r#"{
      "schema_version": 1,
      "snapshots": [
        {
          "package": "amm-pool-contract",
          "function": "require_auth_only",
          "metric": "CPU Instructions",
          "value": 100
        },
        {
          "package": "multi-hyphen-package-name",
          "function": "init",
          "metric": "Memory Bytes",
          "value": 200
        },
        {
          "package": "amm-pool-contract",
          "function": "",
          "metric": "Memory Bytes",
          "value": 50
        }
      ]
    }"#;
    let report_path = dir.path().join("tier_b_report.json");
    fs::write(&report_path, report).expect("failed to write report");

    let budget_toml = r#"
[margin]
cpu_margin = 1.25
memory_margin = 1.10
read_margin = 1.50
write_margin = 2.00

[scenarios.swap_path]
package = "amm-pool-contract"
functions = ["require_auth_only"]
"#;
    fs::write(dir.path().join("budget.toml"), budget_toml).expect("failed to write budget.toml");

    derive_cmd(dir.path())
        .args(["budget-report", "--derive-limits"])
        .arg(&out)
        .args(["--from"])
        .arg(&report_path)
        .assert()
        .success();

    let expected: BTreeSet<String> = [
        // Single hyphen in the package name.
        "TIER_A__AMM_POOL_CONTRACT__REQUIRE_AUTH_ONLY__CPU",
        // Every hyphen in a multi-hyphen package name.
        "TIER_A__MULTI_HYPHEN_PACKAGE_NAME__INIT__MEM",
        // An empty function name collapses the function segment.
        "TIER_A__AMM_POOL_CONTRACT__MEM",
        // Scenario keys keep the package prefix and insert `SCENARIO`.
        "TIER_A__AMM_POOL_CONTRACT__SCENARIO__SWAP_PATH__CPU",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();

    assert_eq!(env_keys(&out), expected);
}
