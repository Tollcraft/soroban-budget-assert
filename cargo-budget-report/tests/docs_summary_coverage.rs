use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

fn docs_src() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cargo-budget-report has a parent directory")
        .join("docs/src")
}

fn summary_targets(summary: &str) -> Vec<PathBuf> {
    let mut targets = Vec::new();
    let mut remaining = summary;

    while let Some((_, after_link_start)) = remaining.split_once("](") {
        let Some((raw_target, after_link)) = after_link_start.split_once(')') else {
            break;
        };
        let target = raw_target
            .split(['#', '?'])
            .next()
            .unwrap_or_default()
            .trim()
            .trim_start_matches('<')
            .trim_end_matches('>');

        if !target.is_empty() && !target.contains("://") && !target.starts_with("mailto:") {
            targets.push(PathBuf::from(target));
        }
        remaining = after_link;
    }

    targets
}

fn chapter_pages(docs_src: &Path) -> HashSet<PathBuf> {
    fs::read_dir(docs_src)
        .expect("docs/src should exist")
        .map(|entry| {
            entry
                .expect("documentation entry should be readable")
                .path()
        })
        .filter(|path| {
            path.is_file()
                && path.extension().and_then(|extension| extension.to_str()) == Some("md")
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name != "README.md" && name != "SUMMARY.md")
        })
        .map(|path| {
            path.strip_prefix(docs_src)
                .expect("chapter should be under docs/src")
                .to_path_buf()
        })
        .collect()
}

#[test]
fn every_documentation_chapter_is_listed_in_summary() {
    let docs_src = docs_src();
    let summary = fs::read_to_string(docs_src.join("SUMMARY.md"))
        .expect("docs/src/SUMMARY.md should be readable");
    let listed: HashSet<_> = summary_targets(&summary).into_iter().collect();
    let missing: Vec<_> = chapter_pages(&docs_src)
        .difference(&listed)
        .map(|path| path.display().to_string())
        .collect();

    assert!(
        missing.is_empty(),
        "documentation chapters missing from SUMMARY.md:\n{}",
        missing.join("\n")
    );
}

#[test]
fn every_summary_markdown_link_resolves_to_a_file() {
    let docs_src = docs_src();
    let summary = fs::read_to_string(docs_src.join("SUMMARY.md"))
        .expect("docs/src/SUMMARY.md should be readable");
    let missing: Vec<_> = summary_targets(&summary)
        .into_iter()
        .filter(|target| !docs_src.join(target).is_file())
        .map(|target| target.display().to_string())
        .collect();

    assert!(
        missing.is_empty(),
        "SUMMARY.md contains links to missing files:\n{}",
        missing.join("\n")
    );
}
