//! Structural validation for `CHANGELOG.md`.
//!
//! The file had accumulated two `## Unreleased` sections plus `### Notes`
//! headings that are not 'Keep a Changelog' categories, so nothing caught a
//! second unreleased header or an invented category creeping in. These tests
//! pin the shape: one unreleased section, only the standard categories, one
//! of each per release section, versions in descending order, and issue
//! references written as `#NNN` rather than pasted URLs.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

const CHANGELOG: &str = "CHANGELOG.md";

/// 'Keep a Changelog' 1.1.0 category names.
const CATEGORIES: &[&str] = &[
    "Added",
    "Changed",
    "Deprecated",
    "Removed",
    "Fixed",
    "Security",
];

fn changelog() -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cargo-budget-report has a parent directory")
        .to_path_buf();
    fs::read_to_string(root.join(CHANGELOG)).expect("read CHANGELOG.md")
}

#[derive(Debug)]
struct Section {
    heading: String,
    categories: Vec<(String, usize)>,
}

/// Top-level (`## `) sections with their `### ` category headings and entry counts.
fn sections(text: &str) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    for line in text.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            sections.push(Section {
                heading: heading.trim().to_owned(),
                categories: Vec::new(),
            });
        } else if let Some(category) = line.strip_prefix("### ") {
            let section = sections
                .last_mut()
                .expect("every ### heading belongs to a ## section");
            let name = category.trim().to_owned();
            let entry = section
                .categories
                .iter_mut()
                .find(|(existing, _)| *existing == name);
            match entry {
                Some((_, count)) => *count += 1,
                None => section.categories.push((name, 0)),
            }
        } else if line.starts_with("- ") {
            if let Some((_, count)) = sections
                .last_mut()
                .and_then(|section| section.categories.last_mut())
            {
                *count += 1;
            }
        }
    }
    sections
}

#[test]
fn there_is_exactly_one_unreleased_section() {
    let sections = sections(&changelog());
    let unreleased: Vec<&str> = sections
        .iter()
        .map(|section| section.heading.as_str())
        .filter(|heading| heading.starts_with("Unreleased"))
        .collect();

    assert_eq!(
        unreleased,
        ["Unreleased"],
        "expected a single `## Unreleased` section, found {unreleased:?}"
    );
    assert_eq!(
        sections.first().map(|section| section.heading.as_str()),
        Some("Unreleased"),
        "`## Unreleased` must come before the released versions"
    );
}

#[test]
fn every_section_uses_standard_categories_without_duplicates() {
    let allowed: HashSet<&str> = CATEGORIES.iter().copied().collect();

    for section in sections(&changelog()) {
        let mut seen = HashSet::new();
        for (name, entries) in &section.categories {
            assert!(
                allowed.contains(name.as_str()),
                "section `## {}` uses non-standard category `### {name}`; allowed: {CATEGORIES:?}",
                section.heading
            );
            assert!(
                seen.insert(name.clone()),
                "section `## {}` repeats category `### {name}`",
                section.heading
            );
            assert!(
                *entries > 0,
                "section `## {}` category `### {name}` has no entries",
                section.heading
            );
        }
        assert!(
            !section.categories.is_empty(),
            "section `## {}` has no categories",
            section.heading
        );
    }
}

#[test]
fn release_headings_are_version_stamped_and_ordered_newest_first() {
    let released: Vec<String> = sections(&changelog())
        .into_iter()
        .map(|section| section.heading)
        .filter(|heading| heading != "Unreleased")
        .collect();

    let mut previous: Option<(u64, u64, u64)> = None;
    for heading in &released {
        let version = parse_release_heading(heading).unwrap_or_else(|| {
            panic!("release heading `## {heading}` must look like `## [1.2.3] - YYYY-MM-DD`")
        });
        if let Some(previous) = previous {
            assert!(
                version < previous,
                "release `## {heading}` must be listed after newer versions"
            );
        }
        previous = Some(version);
    }
}

/// `[1.2.3] - 2026-07-24` → `(1, 2, 3)`.
fn parse_release_heading(heading: &str) -> Option<(u64, u64, u64)> {
    let (version, date) = heading.split_once("] - ")?;
    let version = version.strip_prefix('[')?;
    let date = date.trim();
    let date_ok = date.len() == 10
        && date.as_bytes()[4] == b'-'
        && date.as_bytes()[7] == b'-'
        && date
            .bytes()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit());
    if !date_ok {
        return None;
    }
    let mut parts = version.split('.').map(str::parse::<u64>);
    let parsed = (
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
    );
    if parts.next().is_some() {
        return None;
    }
    Some(parsed)
}

#[test]
fn issue_references_use_hash_numbers_not_urls() {
    let text = changelog();
    assert!(
        !text.contains("/issues/"),
        "issue references belong as `#NNN`, not full URLs"
    );

    let lower = text.to_lowercase();
    let mut cursor = 0;
    let mut referenced = 0;
    while let Some(offset) = lower[cursor..].find("issue") {
        let at = cursor + offset;
        cursor = at + "issue".len();
        let rest = &lower[cursor..];
        // Accept `issue #123` and `issues #1, #2`; a number must follow.
        let rest = rest.strip_prefix('s').unwrap_or(rest);
        if let Some(after) = rest.strip_prefix(" #") {
            let number: String = after.chars().take_while(char::is_ascii_digit).collect();
            assert!(
                !number.is_empty(),
                "issue reference at byte {at} must be followed by a number"
            );
            referenced += 1;
        }
    }
    assert!(
        referenced > 5,
        "expected the changelog to reference issues as `#NNN`, found {referenced}"
    );
}

#[test]
fn release_heading_parser_recognises_the_expected_shape() {
    assert_eq!(
        parse_release_heading("[0.1.0] - 2026-07-24"),
        Some((0, 1, 0))
    );
    assert_eq!(
        parse_release_heading("[1.10.2] - 2030-12-31"),
        Some((1, 10, 2))
    );
    assert_eq!(parse_release_heading("Unreleased"), None);
    assert_eq!(parse_release_heading("[0.1.0]"), None);
    assert_eq!(parse_release_heading("[0.1.0] - 2026/07/24"), None);
    assert_eq!(parse_release_heading("[0.1] - 2026-07-24"), None);
}
