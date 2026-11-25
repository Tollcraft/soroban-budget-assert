//! Guard against duplicated headings in the GitBook chapters under
//! `docs/src/`.
//!
//! Issue #1062: `docs/src/cross_contract_testing.md` carried the heading
//! `## Budget report for cross-contract contracts` twice in a row, with only
//! a blank line between them. GitBook (like mdBook) turns each heading into a
//! table-of-contents entry, so the first copy produced an anchor with no
//! content under it and every following subsection attached to the second.
//! Nothing in the repository caused a copy-paste duplicate like that to fail
//! CI, which is what this scan adds.
//!
//! The rule is narrow on purpose: a duplicate is only an error when the same
//! heading line appears consecutively with no intervening prose, list, code,
//! or other content. Repeated headings separated by real content are normal
//! (for example a `### Figures` heading under several different sections).

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate lives one level below the repo root")
        .to_path_buf()
}

fn docs_src() -> PathBuf {
    repo_root().join("docs/src")
}

/// Collects every Markdown chapter under `dir`, recursively and in a stable
/// order so a failure always names the same files.
fn markdown_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|entry| entry.expect("directory entry should be readable").path())
        .collect();
    entries.sort();

    for path in entries {
        if path.is_dir() {
            markdown_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            out.push(path);
        }
    }
}

/// A heading that repeats the previous non-blank line verbatim.
#[derive(Debug, PartialEq, Eq)]
struct DuplicateHeading {
    /// 1-based line number of the repeated heading.
    line: usize,
    /// The heading text, with surrounding whitespace removed.
    text: String,
}

/// Returns `true` for a Markdown ATX heading (`#` … `######` followed by a
/// space or end of line).
fn is_atx_heading(line: &str) -> bool {
    let level = line.chars().take_while(|c| *c == '#').count();
    (1..=6).contains(&level) && (line.len() == level || line[level..].starts_with(' '))
}

/// Finds headings that repeat the previous non-blank line.
///
/// Blank lines are skipped rather than treated as separators, because the
/// GitBook duplicate that motivated this check had exactly that shape:
///
/// ```text
/// ## Budget report for cross-contract contracts
///
/// ## Budget report for cross-contract contracts
/// ```
///
/// Fenced code blocks are ignored so a `#` comment inside a `rust` example
/// (mdBook hides those lines) is never mistaken for a heading.
fn duplicate_headings(source: &str) -> Vec<DuplicateHeading> {
    let mut duplicates = Vec::new();
    let mut previous_heading: Option<String> = None;
    let mut in_fence = false;

    for (index, raw_line) in source.lines().enumerate() {
        let line = raw_line.trim();

        // Fences open and close on lines whose trimmed form starts with
        // three backticks (or tildes); the fence lines themselves carry no
        // heading content.
        if line.starts_with("```") || line.starts_with("~~~") {
            in_fence = !in_fence;
            previous_heading = None;
            continue;
        }
        if in_fence || line.is_empty() {
            continue;
        }

        if is_atx_heading(line) {
            if previous_heading.as_deref() == Some(line) {
                duplicates.push(DuplicateHeading {
                    line: index + 1,
                    text: line.to_string(),
                });
            }
            previous_heading = Some(line.to_string());
        } else {
            // Any non-heading content ends the consecutive run.
            previous_heading = None;
        }
    }

    duplicates
}

#[test]
fn no_chapter_contains_consecutive_duplicate_headings() {
    let mut files = Vec::new();
    markdown_files(&docs_src(), &mut files);
    assert!(
        !files.is_empty(),
        "no Markdown chapters found under {}",
        docs_src().display()
    );

    let mut failures = Vec::new();
    for path in files {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        for duplicate in duplicate_headings(&source) {
            failures.push(format!(
                "{}:{}: duplicated heading {:?}",
                path.strip_prefix(repo_root()).unwrap_or(&path).display(),
                duplicate.line,
                duplicate.text
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "duplicate consecutive headings found:\n{}",
        failures.join("\n")
    );
}

#[test]
fn cross_contract_chapter_has_one_budget_report_heading() {
    // Pins the specific duplicate removed for issue #1062, so a future
    // copy-paste that reintroduces it fails even if the scanner above is
    // loosened.
    let chapter = fs::read_to_string(docs_src().join("cross_contract_testing.md"))
        .expect("cross_contract_testing.md should exist");
    let heading = "## Budget report for cross-contract contracts";
    let mut occurrences = 0;
    for line in chapter.lines() {
        if line.trim() == heading {
            occurrences += 1;
        }
    }
    assert_eq!(occurrences, 1, "{heading:?} must appear exactly once");
}

#[test]
fn scanner_detects_the_blank_line_separated_duplicate() {
    let source = "\
## Budget report for cross-contract contracts

## Budget report for cross-contract contracts

Body text.
";
    assert_eq!(
        duplicate_headings(source),
        vec![DuplicateHeading {
            line: 3,
            text: "## Budget report for cross-contract contracts".to_string(),
        }]
    );
}

#[test]
fn scanner_allows_repeated_headings_with_content_between() {
    let source = "\
## Section one

### Figures

Table.

## Section two

### Figures

Table.
";
    assert!(duplicate_headings(source).is_empty());
}

#[test]
fn scanner_ignores_hash_lines_inside_fenced_code() {
    let source = "\
## Example

```rust
#![no_std]
#![no_std]
```
";
    assert!(duplicate_headings(source).is_empty());
}

#[test]
fn scanner_requires_an_atx_heading_not_a_hashtag_line() {
    // `#hashtag` is not a heading (no space after the hashes), so two such
    // lines in a row are not a duplicate-heading failure.
    let source = "\
#hashtag
#hashtag
";
    assert!(duplicate_headings(source).is_empty());
}
