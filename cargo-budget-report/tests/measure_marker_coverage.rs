//! Every calibration harness must be discoverable by the regeneration script.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn every_measurement_harness_has_a_valid_marker() {
    let root = repo_root();
    let output = Command::new("git")
        .args(["ls-files", "*/tests/*.rs"])
        .current_dir(&root)
        .output()
        .expect("list tracked test files");
    assert!(output.status.success(), "git ls-files failed");

    let mut harnesses = 0;
    for relative in String::from_utf8_lossy(&output.stdout).lines() {
        let name = relative.rsplit('/').next().unwrap_or_default();
        if !(name.starts_with("calibrate_") || name.starts_with("measure_"))
            || name == "measure_marker_coverage.rs"
        {
            continue;
        }

        harnesses += 1;
        let path = root.join(relative);
        let contents = fs::read_to_string(&path).expect("read measurement harness");
        let marker = contents
            .lines()
            .find(|line| line.trim_start().starts_with("// @measure "))
            .unwrap_or_else(|| panic!("{relative} has no // @measure marker"));
        let mode = marker
            .split("// @measure ")
            .nth(1)
            .and_then(|value| value.split_whitespace().next())
            .unwrap_or_default();
        assert!(
            mode == "local"
                || mode == "testnet"
                || mode.starts_with("local:")
                || mode.starts_with("testnet:"),
            "{relative} has invalid measurement mode {mode:?}"
        );
    }

    assert!(harnesses > 0, "expected measurement harnesses");
}
