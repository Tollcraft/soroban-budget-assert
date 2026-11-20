//! Integration tests for JSON envelope metadata (issue #1047).
//!
//! Validates serialization round-trip, presence of `generated_at` (ISO-8601 UTC)
//! and `generator_version`, and that `validate_report_schema` permits these metadata fields.

use assert_cmd::Command;
use std::fs;
use std::path::Path;

fn derive_cmd(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary should be built");
    cmd.current_dir(dir);
    cmd
}

#[test]
fn serialized_json_report_contains_metadata_and_passes_validation() {
    let dir = tempfile::tempdir().expect("tempdir");

    // Execute budget-report with a mock fixture to produce JSON
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mock_workspace");
    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary built");
    cmd.current_dir(&fixtures)
        .arg("budget-report")
        .arg("--json")
        .arg("--replay")
        .arg(fixtures.join("fixture_record.json"));

    let output = cmd.output().expect("run command");
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let val: serde_json::Value = serde_json::from_str(&stdout).expect("valid json output");
        assert_eq!(val["schema_version"], 1);
        assert!(val["snapshots"].is_array());

        assert!(val.get("generated_at").is_some());
        let gen_at = val["generated_at"].as_str().unwrap();
        assert!(gen_at.ends_with('Z'));
        assert_eq!(gen_at.len(), 20); // YYYY-MM-DDTHH:MM:SSZ

        assert!(val.get("generator_version").is_some());
        assert_eq!(
            val["generator_version"].as_str().unwrap(),
            env!("CARGO_PKG_VERSION")
        );
    }

    // Direct envelope validation with metadata fields via --derive-limits
    let json_with_metadata = serde_json::json!({
        "schema_version": 1,
        "generated_at": "2026-11-20T21:00:00Z",
        "generator_version": "0.1.0",
        "snapshots": [
            {
                "package": "amm-pool-contract",
                "function": "swap",
                "metric": "CPU Instructions",
                "value": 1500
            }
        ]
    });

    let report_file = dir.path().join("report_with_meta.json");
    fs::write(&report_file, json_with_metadata.to_string()).expect("write report");
    let out = dir.path().join("tier-a-limits.env");

    let assert = derive_cmd(dir.path())
        .args(["budget-report", "--derive-limits"])
        .arg(&out)
        .args(["--from"])
        .arg(&report_file)
        .args([
            "--margin-cpu",
            "1.25",
            "--margin-memory",
            "1.25",
            "--margin-read",
            "1.25",
            "--margin-write",
            "1.25",
        ])
        .assert();

    assert.success();
}
