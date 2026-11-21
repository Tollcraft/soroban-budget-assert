use assert_cmd::Command;
use std::fs;
use std::path::Path;

fn derive_cmd(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary should be built");
    cmd.current_dir(dir);
    cmd
}

#[test]
fn test_all_schema_rejection_branches_name_accepted_versions() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let out = dir.path().join("tier-a-limits.env");

    let test_cases = [
        ("missing", r#"{"snapshots": []}"#),
        ("null", r#"{"schema_version": null, "snapshots": []}"#),
        ("unsupported", r#"{"schema_version": 99, "snapshots": []}"#),
        (
            "invalid_type",
            r#"{"schema_version": "invalid", "snapshots": []}"#,
        ),
    ];

    for (name, json_content) in test_cases {
        let file_path = dir.path().join(format!("{name}.json"));
        fs::write(&file_path, json_content).expect("failed to write test file");

        let assert = derive_cmd(dir.path())
            .args(["budget-report", "--derive-limits"])
            .arg(&out)
            .args(["--from"])
            .arg(&file_path)
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

        let output = assert.failure().get_output().clone();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("accepted versions are [1]")
                || stderr.contains("accepted versions are"),
            "case '{name}' should report accepted versions in stderr, got:\n{stderr}"
        );
    }
}
