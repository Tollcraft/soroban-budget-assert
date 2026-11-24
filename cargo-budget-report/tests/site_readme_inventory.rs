//! Guard that `site/README.md` keeps a complete inventory of `site/`.
//!
//! Issue #1065: the file inventory stopped at `assets/dashboard.js`, so the
//! validator (`site/scripts/validate.sh`) that
//! `.github/workflows/deploy-site.yml` runs before publishing was invisible to
//! anyone editing the dashboard. The README already listed the HTML, CSS, and
//! JavaScript files; it simply never grew when the validator was added.
//!
//! These tests walk the real `site/` tree and require each tracked file to
//! appear as a backticked path in the README's `### Structure` section, so the
//! next added asset cannot silently omit itself from the inventory either.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate lives one level below the repo root")
        .to_path_buf()
}

fn site_dir() -> PathBuf {
    repo_root().join("site")
}

fn readme() -> String {
    let path = site_dir().join("README.md");
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Collects every file under `site/` as a `/`-separated path relative to
/// `site/`, sorted for deterministic failure messages.
fn site_files(dir: &Path, base: &Path, out: &mut Vec<String>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|entry| entry.expect("directory entry should be readable").path())
        .collect();
    entries.sort();

    for path in entries {
        if path.is_dir() {
            site_files(&path, base, out);
        } else {
            let stripped = path.strip_prefix(base);
            let relative_path = stripped.expect("site file lives under site/");
            let relative = relative_path.to_string_lossy().replace('\\', "/");
            out.push(relative);
        }
    }
}

/// Returns the body of the README's `### Structure` section, up to the next
/// heading of the same or higher level.
fn structure_section(readme: &str) -> &str {
    let marker = "### Structure";
    let start = readme
        .find(marker)
        .expect("site/README.md has a `### Structure` section");
    let body = &readme[start + marker.len()..];
    let mut end = body.len();
    let mut offset = 0;
    for line in body.lines() {
        if line.starts_with("### ") || line.starts_with("## ") {
            end = offset;
            break;
        }
        offset += line.len() + 1;
    }
    &body[..end]
}

/// Returns the backticked spans in `text`, e.g. `` `index.html` `` ->
/// `index.html`.
fn backticked_paths(text: &str) -> Vec<&str> {
    text.split('`')
        .enumerate()
        .filter(|(index, _)| *index % 2 == 1)
        .map(|(_, span)| span)
        .collect()
}

#[test]
fn every_tracked_site_file_is_in_the_inventory() {
    let base = site_dir();
    let mut files = Vec::new();
    site_files(&base, &base, &mut files);
    assert!(
        files.iter().any(|file| file.as_str() == "index.html"),
        "expected to find site/index.html; enumeration returned {files:?}"
    );

    let readme = readme();
    let listed = backticked_paths(structure_section(&readme));

    let missing: Vec<&String> = files
        .iter()
        .filter(|file| !listed.contains(&file.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "site/README.md's Structure inventory is missing tracked files: {missing:?}\nlisted: {listed:?}"
    );
}

#[test]
fn validator_script_is_documented_with_its_ci_role() {
    let readme = readme();
    let inventory = structure_section(&readme);

    assert!(
        backticked_paths(inventory).contains(&"scripts/validate.sh"),
        "the inventory must list `scripts/validate.sh`: {inventory}"
    );
    assert!(
        readme.contains("site/scripts/validate.sh"),
        "the README must name the validator's full path for local use"
    );
    assert!(
        inventory.contains("deploy-site.yml"),
        "the inventory entry must explain that CI runs the validator before publishing"
    );
    assert!(
        inventory.contains("before the site is published"),
        "the inventory entry must explain the pre-deployment check"
    );
}
