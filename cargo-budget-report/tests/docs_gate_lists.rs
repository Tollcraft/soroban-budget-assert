use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cargo-budget-report has a parent directory")
        .to_path_buf()
}

#[test]
fn release_checklist_covers_the_ci_quality_and_budget_gates() {
    let root = repo_root();
    let release_process = fs::read_to_string(root.join("docs/src/release_process.md"))
        .expect("read release process documentation");
    let quality_workflow = fs::read_to_string(root.join(".github/workflows/quality.yml"))
        .expect("read quality workflow");
    let budget_workflow = fs::read_to_string(root.join(".github/workflows/budget.yml"))
        .expect("read budget workflow");
    let ci_workflows = format!("{quality_workflow}\n{budget_workflow}");

    for gate in [
        "cargo fmt --all -- --check",
        "cargo clippy --workspace --all-targets -- -D warnings",
        "scripts/check-module-reachability.sh",
        "bash scripts/check-cli-docs.sh",
        ".github/scripts/check-compatibility-docs.sh",
        "cargo install cargo-machete --version 0.6.2 --locked",
        "cargo machete",
        "stellar contract build --package",
        "cargo test --workspace",
        "cargo test -p amm-pool-contract --test budget_test test_export_tier_a_budget_report",
    ] {
        assert!(
            ci_workflows.contains(gate),
            "expected CI workflows to define gate `{gate}`"
        );
        assert!(
            release_process.contains(gate),
            "release checklist is missing CI gate `{gate}`"
        );
    }
}
