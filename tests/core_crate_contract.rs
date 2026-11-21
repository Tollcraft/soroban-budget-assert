//! Pins the packaging decision recorded in the crate-level status note in
//! `src/lib.rs` (#827): `soroban-budget-assert-core` is internal groundwork that
//! is deliberately kept and deliberately unpublished.
//!
//! The note says the crate becomes load-bearing — and the note must be revised —
//! the moment a shipped crate takes it as a normal dependency. These tests fail
//! at exactly that boundary so the decision is re-made rather than drifting:
//!
//! - the compiled package is the workspace-root crate and sets `publish = false`;
//! - every other workspace manifest that names the crate does so only under
//!   `[dev-dependencies]`;
//! - the crate-level note still states the status and the promotion triggers.

use std::fs;
use std::path::PathBuf;

const CRATE_NAME: &str = "soroban-budget-assert-core";

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Splits a manifest into `(header, body)` pairs, one per `[section]` line.
fn sections(manifest: &str) -> Vec<(String, Vec<String>)> {
    let mut sections: Vec<(String, Vec<String>)> = Vec::new();
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            sections.push((trimmed.to_string(), Vec::new()));
        } else if let Some((_, body)) = sections.last_mut() {
            body.push(line.to_string());
        }
    }
    sections
}

fn body_of<'a>(sections: &'a [(String, Vec<String>)], header: &str) -> Option<&'a [String]> {
    sections
        .iter()
        .find(|(name, _)| name == header)
        .map(|(_, body)| body.as_slice())
}

fn collect_quoted(text: &str, out: &mut Vec<String>) {
    for value in text.split('"').skip(1).step_by(2) {
        out.push(value.to_string());
    }
}

/// Reads the `members` array out of the root manifest's `[workspace]` table.
fn workspace_members(root_manifest: &str) -> Vec<String> {
    let sections = sections(root_manifest);
    let workspace = body_of(&sections, "[workspace]").expect("root Cargo.toml defines [workspace]");
    let mut members = Vec::new();
    let mut inside = false;
    for line in workspace {
        let trimmed = line.trim();
        if trimmed.starts_with("members") {
            inside = true;
        }
        if inside {
            collect_quoted(trimmed, &mut members);
            inside = !trimmed.contains(']');
        }
    }
    members
}

/// Whether a manifest line names the crate as a dependency key.
fn names_crate(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.starts_with('#') && trimmed.starts_with(CRATE_NAME)
}

#[test]
fn package_is_the_unpublished_workspace_root_crate() {
    assert_eq!(
        env!("CARGO_PKG_NAME"),
        CRATE_NAME,
        "the packaging decision covers the workspace-root crate; a rename must revisit #827"
    );

    let manifest =
        fs::read_to_string(manifest_dir().join("Cargo.toml")).expect("root Cargo.toml is readable");
    let sections = sections(&manifest);
    let package = body_of(&sections, "[package]").expect("root Cargo.toml defines [package]");
    let normalized: Vec<&str> = package.iter().map(|line| line.trim()).collect();

    assert!(
        normalized.contains(&format!("name = \"{CRATE_NAME}\"").as_str()),
        "root [package] must remain {CRATE_NAME}"
    );
    assert!(
        normalized.contains(&"publish = false"),
        "{CRATE_NAME} is internal groundwork and must stay unpublishable"
    );
}

#[test]
fn only_dev_dependencies_name_the_core_crate() {
    let root_manifest =
        fs::read_to_string(manifest_dir().join("Cargo.toml")).expect("root Cargo.toml is readable");
    let members = workspace_members(&root_manifest);
    assert!(
        members.contains(&"cargo-budget-report".to_string()),
        "workspace members were not parsed from the root manifest: {members:?}"
    );

    let mut dev_dependency_hits = Vec::new();
    for member in &members {
        let path = manifest_dir().join(member).join("Cargo.toml");
        let manifest = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()));

        for (header, body) in sections(&manifest) {
            if !body.iter().any(|line| names_crate(line)) {
                continue;
            }
            assert_eq!(
                header, "[dev-dependencies]",
                "{member}/Cargo.toml names {CRATE_NAME} under {header}. A normal dependency \
                 makes the crate load-bearing: update the status note in src/lib.rs and this \
                 test together (#827), or move the import behind a dev-dependency."
            );
            dev_dependency_hits.push(member.clone());
        }
    }

    assert!(
        !dev_dependency_hits.is_empty(),
        "no workspace member dev-depends on {CRATE_NAME}; the packaging decision is stale (#827)"
    );
}

#[test]
fn crate_note_records_the_keep_decision() {
    let source =
        fs::read_to_string(manifest_dir().join("src/lib.rs")).expect("src/lib.rs is readable");
    let note = source
        .split("pub mod ")
        .next()
        .expect("src/lib.rs starts with its crate-level note");

    for needle in [
        "internal groundwork",
        "not published to crates.io",
        "[`BudgetAssert`] / [`impls::BudgetBounds`]",
        "onto [`CostMeasurer`]",
        "normal dependency",
    ] {
        assert!(
            note.contains(needle),
            "the crate-level note in src/lib.rs lost {needle:?}; the keep-vs-retire decision \
             recorded for #827 must stay documented"
        );
    }
}
