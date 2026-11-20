//! `scripts/check-docs-anchors.sh` must resolve the real book and fail on a
//! renamed heading. Cross-page anchors like `reference.md#mainnet-guard` were
//! never checked before, so a heading reword silently produced dead links.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cargo-budget-report has a parent directory")
        .to_path_buf()
}

fn run_checker(dir: &Path) -> std::process::Output {
    Command::new("bash")
        .arg(repo_root().join("scripts/check-docs-anchors.sh"))
        .arg(dir)
        .output()
        .expect("run check-docs-anchors.sh")
}

#[test]
fn current_book_anchors_all_resolve() {
    let output = run_checker(&repo_root().join("docs/src"));
    assert!(
        output.status.success(),
        "docs anchors should resolve:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("anchor(s) resolved"),
        "expected a resolution summary, got: {stdout}"
    );
}

#[test]
fn missing_anchor_reports_file_and_line() {
    let dir = tempfile::tempdir().expect("temp dir");
    fs::write(
        dir.path().join("guide.md"),
        "# Guide\n\nSee [the reference](reference.md#no-such-heading).\n",
    )
    .expect("write guide");
    fs::write(
        dir.path().join("reference.md"),
        "# Reference\n\n## Mainnet Guard\n\nBody.\n",
    )
    .expect("write reference");

    let output = run_checker(dir.path());
    assert!(
        !output.status.success(),
        "a dead anchor must fail the check"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("guide.md:3"),
        "error should name the source file and line: {stderr}"
    );
    assert!(
        stderr.contains("#no-such-heading"),
        "error should name the missing fragment: {stderr}"
    );
    assert!(
        stderr.contains("reference.md"),
        "error should name the destination: {stderr}"
    );
}

#[test]
fn heading_slug_rules_match_github_and_mdbook() {
    let dir = tempfile::tempdir().expect("temp dir");
    // `--check` loses its punctuation, the emoji leaves a leading hyphen, and
    // the `<name>` in a code span is text under GitHub/mdBook 0.5 rules.
    fs::write(
        dir.path().join("page.md"),
        concat!(
            "# Page\n\n",
            "## Mainnet Guard\n\n",
            "## `--check`: enforcing limits\n\n",
            "## ⚙️ Supported Versions & Compatibility\n\n",
            "## `functions.<name>`: per-function configuration\n\n",
            "## Example\n\n## Example\n\n",
            "[a](#mainnet-guard) [b](#--check-enforcing-limits) ",
            "[c](#-supported-versions--compatibility) ",
            "[d](#functionsname-per-function-configuration) ",
            "[e](#example-1)\n",
        ),
    )
    .expect("write page");

    let output = run_checker(dir.path());
    assert!(
        output.status.success(),
        "expected the GitHub/mdBook slugs to resolve:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn same_page_anchor_is_checked() {
    let dir = tempfile::tempdir().expect("temp dir");
    fs::write(
        dir.path().join("index.md"),
        "# Index\n\n[jump](#present)\n\n## Present\n",
    )
    .expect("write page");
    assert!(run_checker(dir.path()).status.success());

    fs::write(
        dir.path().join("index.md"),
        "# Index\n\n[jump](#gone)\n\n## Present\n",
    )
    .expect("write page");
    let output = run_checker(dir.path());
    assert!(!output.status.success(), "same-page dead anchors must fail");
    assert!(String::from_utf8_lossy(&output.stderr).contains("#gone"));
}
