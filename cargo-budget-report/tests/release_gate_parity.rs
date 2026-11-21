//! Gate-parity test between the release workflow and the workspace CI gates.
//!
//! `.github/workflows/release.yml` runs on tags only, so the Code Quality and
//! Budget workflows' `push`/`pull_request` triggers on `main` never see the
//! tagged commit. If the release workflow builds and publishes without the
//! workspace gates, a tag can ship a binary that `main` would have rejected.
//! The `verify` job exists to close that gap; this test keeps it closed by
//! asserting that every gate CI enforces on `main` also runs before any release
//! job builds, checksums or publishes.

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cargo-budget-report has a parent directory")
        .to_path_buf()
}

fn read(relative: &str) -> String {
    fs::read_to_string(repo_root().join(relative))
        .unwrap_or_else(|err| panic!("read {relative}: {err}"))
}

/// The workspace gates CI enforces on `main`, with the workflow that owns each.
/// A release build is only as trustworthy as the weakest of these.
const GATES: &[(&str, &str)] = &[
    ("cargo fmt --all -- --check", "quality.yml"),
    (
        "cargo clippy --workspace --all-targets -- -D warnings",
        "quality.yml",
    ),
    ("cargo test --workspace", "budget.yml"),
    ("scripts/check-module-reachability.sh", "quality.yml"),
    ("bash scripts/check-cli-docs.sh", "quality.yml"),
    (".github/scripts/check-compatibility-docs.sh", "quality.yml"),
];

/// The jobs that can put a binary or crate in front of users.
const PUBLISHING_JOBS: &[&str] = &["build", "checksums", "publish"];

/// Text of a top-level job block, from its `  <job>:` header up to the next
/// job header. Comments and nested keys do not end the block.
fn job_block(workflow: &str, job: &str) -> String {
    let lines: Vec<&str> = workflow.lines().collect();
    let header = format!("  {job}:");
    let start = lines
        .iter()
        .position(|line| *line == header)
        .unwrap_or_else(|| panic!("workflow has no `{job}` job"));
    let end = lines[start + 1..]
        .iter()
        .position(|line| {
            line.starts_with("  ")
                && !line.starts_with("   ")
                && !line.trim_start().starts_with('#')
                && line.trim_end().ends_with(':')
        })
        .map_or(lines.len(), |offset| start + 1 + offset);
    lines[start..end].join("\n")
}

/// Every `run:` script in a job block, inline or block scalar.
fn run_scripts(block: &str) -> Vec<String> {
    let mut scripts = Vec::new();
    let lines: Vec<&str> = block.lines().collect();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        // `run:` is a step key in this repository's workflows but a `- run:`
        // list item in the compact form, so accept both.
        let trimmed = trimmed.strip_prefix("- ").unwrap_or(trimmed);
        let block_scalar = matches!(trimmed, "run:" | "run: |" | "run: >");
        if !block_scalar && trimmed.starts_with("run: ") {
            scripts.push(trimmed["run: ".len()..].trim().to_string());
        } else if block_scalar {
            let mut body = Vec::new();
            index += 1;
            while index < lines.len() {
                let candidate = lines[index];
                let candidate_trimmed = candidate.trim_start();
                let candidate_indent = candidate.len() - candidate_trimmed.len();
                if !candidate_trimmed.is_empty() && candidate_indent <= indent {
                    break;
                }
                body.push(candidate_trimmed);
                index += 1;
            }
            scripts.push(body.join("\n"));
            // `index` already points at the first line after the block scalar.
            continue;
        }
        index += 1;
    }
    scripts
}

/// Values of the job's `needs:` key, whether inline or in flow/block style.
fn needs(block: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in block.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("needs:") else {
            continue;
        };
        let rest = rest.trim();
        let rest = rest.trim_start_matches('[').trim_end_matches(']');
        for entry in rest.split(',') {
            let entry = entry.trim().trim_matches(|c| c == '"' || c == '\'');
            if !entry.is_empty() {
                found.push(entry.to_string());
            }
        }
    }
    found
}

#[test]
fn release_workflow_runs_every_workspace_gate_before_publishing() {
    let release = read(".github/workflows/release.yml");
    let verify = job_block(&release, "verify");
    let scripts = run_scripts(&verify).join("\n");

    for (gate, owner) in GATES {
        let owner_workflow = read(&format!(".github/workflows/{owner}"));
        assert!(
            owner_workflow.contains(gate),
            "`{gate}` is listed as a workspace gate but {owner} no longer runs it; \
             update GATES to match the gates CI actually enforces"
        );
        assert!(
            scripts.contains(gate),
            "release.yml's verify job does not run the workspace gate `{gate}` that {owner} \
             enforces on main; a tagged commit could publish without it"
        );
    }
}

#[test]
fn publishing_jobs_depend_on_the_verify_job() {
    let release = read(".github/workflows/release.yml");
    for job in PUBLISHING_JOBS {
        let block = job_block(&release, job);
        assert!(
            needs(&block).iter().any(|need| need == "verify"),
            "release.yml's `{job}` job does not declare `needs: verify`; it could run after a \
             tag build that failed the workspace gates"
        );
    }
}

#[test]
fn verify_job_cannot_itself_publish() {
    let release = read(".github/workflows/release.yml");
    let verify = job_block(&release, "verify");
    for forbidden in [
        "cargo publish",
        "softprops/action-gh-release",
        "actions/upload-artifact",
    ] {
        assert!(
            !verify.contains(forbidden),
            "the verify job must only verify, but it contains `{forbidden}`"
        );
    }
    assert!(
        verify.contains("actions/checkout"),
        "the verify job must check out the tagged commit before running the gates"
    );
}

#[test]
fn job_block_extraction_stops_at_the_next_job() {
    let workflow = "jobs:\n  verify:\n    runs-on: ubuntu-latest\n\n    # trailing comment\n    \
                    steps:\n      - run: echo one\n\n  build:\n    needs: verify\n";
    let verify = job_block(workflow, "verify");
    assert!(verify.starts_with("  verify:"));
    assert!(verify.contains("echo one"));
    assert!(!verify.contains("build:"));
    assert_eq!(needs(&job_block(workflow, "build")), ["verify"]);

    assert_eq!(
        run_scripts(
            "    steps:\n      - run: |\n          cargo test --workspace\n          \
                     cargo fmt --all -- --check\n      - run: echo done\n"
        ),
        [
            "cargo test --workspace\ncargo fmt --all -- --check",
            "echo done"
        ]
    );
    assert_eq!(
        needs("  publish:\n    needs: [verify, build, checksums]\n"),
        ["verify", "build", "checksums"]
    );
}
