//! Every `dtolnay/rust-toolchain` step in `.github/workflows/` must pin the
//! workspace channel from `rust-toolchain.toml`.
//!
//! `.github/workflows/quality.yml` and `budget.yml` already passed
//! `toolchain: <pin>`, but `release.yml` invoked the action without one, so
//! published binaries were compiled with whichever stable version the runner
//! had that day — non-deterministic release artifacts. This test fails if any
//! workflow step (existing or new) omits the pin or drifts from
//! `rust-toolchain.toml`, which is the single source of truth.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cargo-budget-report has a parent directory")
        .to_path_buf()
}

/// The channel declared in `rust-toolchain.toml`, e.g. `1.92.0`.
fn workspace_toolchain_pin() -> String {
    let manifest = fs::read_to_string(repo_root().join("rust-toolchain.toml"))
        .expect("rust-toolchain.toml should exist");
    manifest
        .lines()
        .find_map(|line| line.trim().strip_prefix("channel"))
        .and_then(|rest| rest.split('"').nth(1))
        .map(str::to_owned)
        .expect("rust-toolchain.toml should declare channel = \"<version>\"")
}

fn workflow_files() -> Vec<PathBuf> {
    let dir = repo_root().join(".github/workflows");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .expect(".github/workflows should exist")
        .map(|entry| entry.expect("workflow entry should be readable").path())
        .filter(|path| {
            matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("yml") | Some("yaml")
            )
        })
        .collect();
    files.sort();
    files
}

/// Every `dtolnay/rust-toolchain` step, as `(file, line number, step body)`.
fn toolchain_steps(file: &Path) -> Vec<(String, usize, String)> {
    let text = fs::read_to_string(file).expect("workflow should be readable");
    let lines: Vec<&str> = text.lines().collect();
    let mut steps = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        if !line.contains("dtolnay/rust-toolchain") {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        // Collect the rest of this step: blank lines, sibling keys such as
        // `with:` (same indent as `uses:`), and anything indented deeper. A
        // shallower line or the next `- ` list item ends the step.
        let mut body = String::new();
        for follow in &lines[index + 1..] {
            let trimmed = follow.trim();
            let follow_indent = follow.len() - follow.trim_start().len();
            let next_step = trimmed.starts_with("- ") || follow_indent < indent;
            if !trimmed.is_empty() && next_step {
                break;
            }
            body.push_str(follow);
            body.push('\n');
        }
        steps.push((file.display().to_string(), index + 1, body));
    }

    steps
}

#[test]
fn every_workflow_toolchain_step_pins_the_workspace_channel() {
    let pin = workspace_toolchain_pin();
    let mut checked = 0;

    for file in workflow_files() {
        for (path, line, body) in toolchain_steps(&file) {
            checked += 1;
            let declared = body
                .lines()
                .find_map(|candidate| try_toolchain_value(candidate.trim()).map(str::to_owned));
            match declared {
                Some(value) => assert_eq!(
                    value, pin,
                    "{path}:{line} pins toolchain `{value}`, but rust-toolchain.toml declares `{pin}`"
                ),
                None => panic!(
                    "{path}:{line} uses dtolnay/rust-toolchain without `toolchain: {pin}`; \
                     release and CI builds would use the runner's default stable compiler"
                ),
            }
        }
    }

    assert!(
        checked >= 5,
        "expected at least the five known dtolnay/rust-toolchain steps, found {checked}"
    );
}

/// `toolchain: 1.92.0` → `Some("1.92.0")`; anything else → `None`.
fn try_toolchain_value(trimmed: &str) -> Option<&str> {
    let rest = trimmed.strip_prefix("toolchain:")?;
    let value = rest.trim().trim_matches('"').trim_matches('\'');
    if value.is_empty() {
        return None;
    }
    Some(value)
}

#[test]
fn release_jobs_all_pin_the_toolchain() {
    let pin = workspace_toolchain_pin();
    let release = repo_root().join(".github/workflows/release.yml");
    let steps = toolchain_steps(&release);

    assert!(
        steps.len() >= 3,
        "release.yml should install Rust in its build, checksums and publish jobs"
    );
    for (path, line, body) in steps {
        assert!(
            body.contains(&format!("toolchain: {pin}")),
            "{path}:{line} must pass `toolchain: {pin}` so release binaries are reproducible"
        );
    }
}

#[test]
fn toolchain_value_parser_rejects_missing_or_bare_keys() {
    assert_eq!(try_toolchain_value("toolchain: 1.92.0"), Some("1.92.0"));
    assert_eq!(try_toolchain_value("toolchain: \"1.92.0\""), Some("1.92.0"));
    assert_eq!(try_toolchain_value("toolchain:"), None);
    assert_eq!(try_toolchain_value("toolchain: ''"), None);
    assert_eq!(try_toolchain_value("target: wasm32v1-none"), None);
}
