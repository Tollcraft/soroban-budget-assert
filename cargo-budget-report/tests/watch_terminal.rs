use assert_cmd::Command;

#[test]
fn watch_mode_requires_an_interactive_terminal() {
    let output = Command::cargo_bin("cargo-budget-report")
        .expect("binary should be built")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "budget-report",
            "--watch",
            "--network",
            "local",
            "--source",
            "alice",
        ])
        .output()
        .expect("watch command should start");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("watch mode requires an interactive terminal"),
        "watch refusal should explain the tty requirement: {stderr}"
    );
}
