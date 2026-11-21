use assert_cmd::Command;
use std::fs;
use std::path::Path;

fn derive_cmd(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary should be built");
    cmd.current_dir(dir);
    cmd
}

#[test]
fn test_scenario_accumulation_overflow_errors() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let out = dir.path().join("tier-a-limits.env");

    let report_json = r#"{
  "_metadata": {
    "description": "test",
    "generated_for": "test",
    "build_profile": "release"
  },
  "schema_version": 1,
  "snapshots": [
    {
      "package": "amm-pool-contract",
      "function": "fn_a",
      "metric": "CPU Instructions",
      "value": 18446744073709551615
    },
    {
      "package": "amm-pool-contract",
      "function": "fn_b",
      "metric": "CPU Instructions",
      "value": 1
    }
  ]
}"#;
    let report_path = dir.path().join("overflow_report.json");
    fs::write(&report_path, report_json).expect("failed to write report");

    let budget_toml = r#"
[margin]
cpu_margin = 1.25
memory_margin = 1.25
read_margin = 1.25
write_margin = 1.25

[scenarios.overflow_scenario]
package = "amm-pool-contract"
functions = ["fn_a", "fn_b"]
"#;
    fs::write(dir.path().join("budget.toml"), budget_toml).expect("failed to write budget.toml");

    let assert = derive_cmd(dir.path())
        .args(["budget-report", "--derive-limits"])
        .arg(&out)
        .args(["--from"])
        .arg(&report_path)
        .assert();

    let output = assert.failure().get_output().clone();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("overflowed u64::MAX"),
        "stderr should mention overflowed u64::MAX, got:\n{stderr}"
    );
    assert!(
        stderr.contains("overflow_scenario"),
        "stderr should name the scenario, got:\n{stderr}"
    );
    assert!(
        stderr.contains("CPU Instructions"),
        "stderr should name the metric, got:\n{stderr}"
    );
    assert!(
        stderr.contains("fn_b"),
        "stderr should name the overflowing component, got:\n{stderr}"
    );
}
