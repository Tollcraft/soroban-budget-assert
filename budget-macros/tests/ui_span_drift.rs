//! Pinned `.stderr` spans must still fit the fixture they quote (#992).
//!
//! `every_compile_fail_fixture_pins_its_diagnostic` in `tests/ui.rs` checks that
//! each compile-fail fixture has a non-empty snapshot; it never reads the
//! `--> tests/ui/…:line:col` reference or the caret line under it. Both are
//! copied out of rustc's output, so the two ways they go stale are invisible:
//! a fixture edited shorter leaves an underline hanging past the end of the line
//! it annotates, and a snapshot hand-copied from another revision can point at a
//! line the fixture no longer has. Either way the snapshot keeps passing while
//! describing a diagnostic the compiler no longer emits in that shape.
//!
//! rustc's gutter is fixed per snapshot: the source line is
//! `<number> | <content>` and the annotation below it is `<spaces> | <markers>`,
//! with the markers starting in the same visual column as the content they
//! underline. Stripping the one-space gap after the `|` on both kinds of line
//! therefore puts marker index `i` over content index `i`, which is the
//! alignment [`violations`] compares.

use std::fs;
use std::path::{Path, PathBuf};

/// Root of the trybuild fixtures, anchored at the crate so the test runs from
/// any working directory.
fn ui_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("ui")
}

/// Every pinned snapshot, in the order trybuild discovers them.
fn snapshots() -> Vec<PathBuf> {
    let entries = match fs::read_dir(ui_dir()) {
        Ok(entries) => entries,
        Err(err) => panic!("cannot read {}: {err}", ui_dir().display()),
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| matches!(path.extension(), Some(ext) if ext == "stderr"))
        .collect();
    files.sort();
    files
}

fn read(path: &Path) -> String {
    match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => panic!("cannot read {}: {err}", path.display()),
    }
}

/// A `--> path:line:col` reference, as rustc writes it after trimming.
fn reference(line: &str) -> Option<(String, usize, usize)> {
    let target = line.trim_start().strip_prefix("--> ")?.trim_end();
    let mut parts = target.rsplitn(3, ':');
    let column = parts.next()?.parse().ok()?;
    let number = parts.next()?.parse().ok()?;
    let path = parts.next()?.to_string();
    Some((path, number, column))
}

/// The text after the `|` gutter, with rustc's one-space gap removed, so the
/// content of a source line and the markers of the line below it share indices.
fn past_gutter(line: &str) -> Option<&str> {
    let (_, after) = line.split_once('|')?;
    Some(after.strip_prefix(' ').unwrap_or(after))
}

/// A numbered source line: its line number and the content rustc displays.
fn displayed(line: &str) -> Option<(usize, &str)> {
    let (before, _) = line.split_once('|')?;
    Some((before.trim().parse().ok()?, past_gutter(line)?))
}

/// Each contiguous run of `^` or `~` markers on an annotation line, as
/// `(start index, length)`.
fn runs(line: &str) -> Vec<(usize, usize)> {
    let text = match past_gutter(line) {
        Some(text) if displayed(line).is_none() => text,
        _ => return Vec::new(),
    };
    let chars: Vec<char> = text.chars().collect();
    let mut found = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let marker = match chars[index] {
            '^' | '~' => chars[index],
            _ => {
                index += 1;
                continue;
            }
        };
        let start = index;
        while index < chars.len() && chars[index] == marker {
            index += 1;
        }
        found.push((start, index - start));
    }
    found
}

/// Every span in `snapshot` that does not fit the file it quotes.
///
/// `fixture` resolves a reference path to that file's text, or `None` when it
/// cannot be read. Taking it as an argument is what lets the failure modes below
/// be shown against a scratch fixture instead of by editing a real one.
fn violations(snapshot: &str, fixture: &dyn Fn(&str) -> Option<String>) -> Vec<String> {
    let mut reports = Vec::new();
    // The file the references and the displayed lines describe; re-read only
    // when a snapshot moves to a second file.
    let mut source: Option<(String, Vec<String>)> = None;
    let mut pending: Option<(usize, usize)> = None;
    let mut shown: Option<(usize, String)> = None;

    for line in snapshot.lines() {
        if let Some((path, number, column)) = reference(line) {
            pending = Some((number, column));
            shown = None;
            if !matches!(&source, Some((known, _)) if *known == path) {
                source = match fixture(&path) {
                    Some(text) => Some((path.clone(), text.lines().map(str::to_string).collect())),
                    None => {
                        reports.push(format!("`{path}` is referenced but cannot be read"));
                        None
                    }
                };
            }
            if let Some((_, lines)) = &source {
                if number > lines.len() {
                    reports.push(format!(
                        "`{path}`: reference points at line {number}, which the file does \
                         not have (it holds {} lines)",
                        lines.len()
                    ));
                }
            }
            continue;
        }

        if let Some((number, content)) = displayed(line) {
            let width = content.chars().count();
            if let Some((referred, column)) = pending {
                if referred == number && column > width {
                    reports.push(format!(
                        "line {number}: reference points at column {column}, past the {width} \
                         characters the line holds"
                    ));
                }
            }
            shown = Some((number, content.to_string()));
            continue;
        }

        let markers = runs(line);
        if markers.is_empty() {
            continue;
        }
        match shown.take() {
            None => reports.push(format!(
                "marker run {:?} has no displayed source line above it",
                markers
            )),
            Some((number, content)) => {
                let width = content.chars().count();
                for (start, length) in markers {
                    if start + length > width {
                        reports.push(format!(
                            "line {number}: a run of {length} markers starts at column {} and \
                             reaches past the {width} characters the line holds",
                            start + 1
                        ));
                    }
                }
            }
        }
        pending = None;
    }

    reports
}

#[test]
fn every_pinned_span_fits_its_fixture() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut reports = Vec::new();
    for snapshot in snapshots() {
        let text = read(&snapshot);
        let missing = text
            .lines()
            .filter_map(reference)
            .filter(|(path, _, _)| !manifest.join(path).is_file())
            .map(|(path, _, _)| path)
            .collect::<Vec<String>>();
        let named = |path: &str| {
            if missing.contains(&path.to_string()) {
                return None;
            }
            Some(read(&manifest.join(path)))
        };
        let found = violations(&text, &named);
        if !found.is_empty() {
            reports.push(format!("{}:\n  {}", snapshot.display(), found.join("\n  ")));
        }
    }
    assert!(
        reports.is_empty(),
        "pinned spans no longer fit their fixtures:\n{}",
        reports.join("\n")
    );
}

#[test]
fn the_sweep_reads_snapshots_that_carry_references() {
    // Without this, a parser that matched nothing would make the test above
    // pass over an empty set — the exact failure this file is meant to catch.
    let files = snapshots();
    assert!(!files.is_empty(), "no `.stderr` snapshots found to sweep");
    let text: Vec<String> = files.iter().map(|path| read(path)).collect();
    let references: usize = text
        .iter()
        .map(|snapshot| {
            snapshot
                .lines()
                .filter(|line| reference(line).is_some())
                .count()
        })
        .sum();
    assert!(
        references >= files.len(),
        "the {files:?} snapshots carry {references} `-->` references between them; \
         a real snapshot pins at least one"
    );
    for (path, snapshot) in files.iter().zip(&text) {
        assert!(
            snapshot
                .lines()
                .any(|line| !runs(line).is_empty() && displayed(line).is_none()),
            "{}: no marker line was read, so nothing here checks its span",
            path.display()
        );
    }
}

#[test]
fn a_reference_past_the_end_of_the_file_is_reported() {
    let fixture = "fn main() {}\n";
    let snapshot = "error: nothing\n  --> tests/ui/scratch.rs:999:1\n   |\n   |\n";
    let found = violations(snapshot, &|_| Some(fixture.to_string()));
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("line 999") && found[0].contains("it holds 1 lines"),
        "{}",
        found[0]
    );
}

#[test]
fn a_caret_longer_than_its_line_is_reported() {
    // The drift this file exists for: the fixture lost characters and the
    // underline did not.
    let fixture = "#[m(abc)]\nfn main() {}\n";
    let snapshot = concat!(
        "error: nope\n",
        " --> tests/ui/scratch.rs:1:3\n",
        "  |\n",
        "1 | #[m(abc)]\n",
        "  |   ^^^^^^^^^^\n",
    );
    let found = violations(snapshot, &|_| Some(fixture.to_string()));
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("a run of 10 markers") && found[0].contains("the 9 characters"),
        "{}",
        found[0]
    );
}

#[test]
fn a_column_past_the_end_of_its_line_is_reported() {
    let fixture = "#[m(abc)]\nfn main() {}\n";
    let snapshot = concat!(
        "error: nope\n",
        " --> tests/ui/scratch.rs:1:40\n",
        "  |\n",
        "1 | #[m(abc)]\n",
        "  |   ^^^\n",
    );
    let found = violations(snapshot, &|_| Some(fixture.to_string()));
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("column 40") && found[0].contains("the 9 characters"),
        "{}",
        found[0]
    );
}

#[test]
fn a_caret_with_no_source_line_above_it_is_reported() {
    let fixture = "#[m(abc)]\nfn main() {}\n";
    let snapshot = concat!(
        "error: nope\n",
        " --> tests/ui/scratch.rs:1:3\n",
        "  |\n",
        "  |   ^^^\n",
    );
    let found = violations(snapshot, &|_| Some(fixture.to_string()));
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("no displayed source line above it"),
        "{}",
        found[0]
    );
}

#[test]
fn a_referenced_file_that_cannot_be_read_is_reported() {
    let snapshot = "error: nope\n  --> tests/ui/gone.rs:1:1\n   |\n";
    let found = violations(snapshot, &|_| None);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("`tests/ui/gone.rs` is referenced but cannot be read"),
        "{}",
        found[0]
    );
}

#[test]
fn notes_and_help_lines_are_not_spans() {
    // trybuild's snapshots carry `= note` text and continuation gutters. None of
    // it is a span, and a sweep that mistook it for one would fail on every
    // snapshot in the crate.
    let text = concat!(
        "error: aborting due to 1 previous error\n",
        "\n",
        "note: the Lang Items could not be found\n",
        "  = note: try `cargo clean`\n",
        "  = help: something\n",
        "  | |\n",
        "  | +-- not a span\n",
    );
    for line in text.lines() {
        assert!(runs(line).is_empty(), "read a span in {line:?}");
        assert!(displayed(line).is_none(), "read a source line in {line:?}");
        assert!(reference(line).is_none(), "read a reference in {line:?}");
    }
    assert!(violations(text, &|_| Some(String::new())).is_empty());
}
