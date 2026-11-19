use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cargo-budget-report has a parent directory")
        .to_path_buf()
}

#[test]
fn workflow_actions_use_full_commit_shas() {
    let workflows_dir = repo_root().join(".github/workflows");
    let mut workflows: Vec<_> = fs::read_dir(&workflows_dir)
        .expect(".github/workflows should exist")
        .map(|entry| entry.expect("workflow entry should be readable").path())
        .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("yml"))
        .collect();
    workflows.sort();

    let mut violations = Vec::new();
    for workflow in workflows {
        let source = fs::read_to_string(&workflow)
            .unwrap_or_else(|error| panic!("reading {}: {error}", workflow.display()));

        for (line_number, line) in source.lines().enumerate() {
            let Some(action) = line.trim().strip_prefix("uses:") else {
                continue;
            };
            let action = action.split_whitespace().next().unwrap_or_default();
            if action.starts_with("./") {
                continue;
            }

            let sha = action.rsplit_once('@').map(|(_, sha)| sha);
            if !sha.is_some_and(|sha| {
                sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit())
            }) {
                violations.push(format!(
                    "{}:{}: {action}",
                    workflow.display(),
                    line_number + 1
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "workflow actions must use immutable 40-character commit SHAs (local actions are exempt):\n{}",
        violations.join("\n")
    );
}
