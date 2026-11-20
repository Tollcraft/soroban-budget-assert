//! Reachability test for the benchmark capture fixtures under
//! `cargo-budget-report/fixtures/`.
//!
//! `MEASUREMENTS.md` publishes derived figures (gap percentages, per-operation
//! costs), and a reader auditing one of them needs to reach the capture record
//! it came from. A fixture that no published section links is effectively
//! unreachable evidence: the numbers are quotable but not checkable. The
//! reverse direction matters too — a link in `MEASUREMENTS.md` that points at a
//! renamed or deleted fixture is a broken citation.
//!
//! Scope: files matching `*_benchmark.json` are the measurement capture
//! records. The other fixtures in that directory (`simulate_transaction_response_*`,
//! `transaction_data_valid.json`, `tier_b_report_sample.json`) are CLI parsing
//! and replay inputs, not measurement evidence, so requiring `MEASUREMENTS.md`
//! to cite them would be noise.

use std::fs;
use std::path::PathBuf;

use serde_json::Value;

/// Repository-relative path of the fixture directory.
const FIXTURES_DIR: &str = "cargo-budget-report/fixtures";

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

/// Every `*_benchmark.json` capture fixture, as a path relative to the
/// repository root, sorted for stable failure messages.
fn benchmark_fixtures() -> Vec<String> {
    let dir = repo_root().join(FIXTURES_DIR);
    let mut fixtures: Vec<String> = fs::read_dir(&dir)
        .unwrap_or_else(|err| panic!("read {}: {err}", dir.display()))
        .map(|entry| {
            entry
                .expect("read fixture directory entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.ends_with("_benchmark.json"))
        .map(|name| format!("{FIXTURES_DIR}/{name}"))
        .collect();
    fixtures.sort();
    fixtures
}

/// Fixture paths cited anywhere in `MEASUREMENTS.md`, in document order and
/// without duplicates. A citation is any occurrence of the fixture directory
/// followed by a filename; Markdown link syntax, bare paths and trailing
/// punctuation are all accepted.
fn cited_fixtures(measurements: &str) -> Vec<String> {
    let prefix = format!("{FIXTURES_DIR}/");
    let mut found: Vec<String> = Vec::new();
    let mut rest = measurements;
    while let Some(index) = rest.find(&prefix) {
        let tail = &rest[index..];
        let name_end = tail
            .find(|c: char| c.is_whitespace() || matches!(c, ')' | '"' | '`' | '>' | ',' | ';'))
            .unwrap_or(tail.len());
        let path = tail[..name_end].to_string();
        if !found.contains(&path) {
            found.push(path);
        }
        rest = &tail[name_end..];
    }
    found
}

#[test]
fn every_benchmark_fixture_is_linked_from_measurements() {
    let measurements = read("MEASUREMENTS.md");
    let cited = cited_fixtures(&measurements);

    for fixture in benchmark_fixtures() {
        assert!(
            cited.iter().any(|path| path == &fixture),
            "{fixture} is a benchmark capture record but MEASUREMENTS.md never links it; \
             add a section that cites the fixture so the published figures stay auditable"
        );
    }
}

#[test]
fn every_fixture_link_in_measurements_exists() {
    let measurements = read("MEASUREMENTS.md");
    let root = repo_root();

    for cited in cited_fixtures(&measurements) {
        assert!(
            cited.ends_with(".json"),
            "MEASUREMENTS.md cites `{cited}`, which does not look like a fixture file"
        );
        assert!(
            root.join(&cited).is_file(),
            "MEASUREMENTS.md cites `{cited}`, but no such file exists; fix the link or restore \
             the capture record"
        );
    }
}

#[test]
fn benchmark_fixtures_are_valid_json_with_a_recorded_operation_type() {
    let root = repo_root();
    let fixtures = benchmark_fixtures();
    assert!(
        !fixtures.is_empty(),
        "no `*_benchmark.json` fixtures found under {FIXTURES_DIR}; the reachability scan is \
         checking nothing"
    );

    for fixture in fixtures {
        let raw = fs::read_to_string(root.join(&fixture))
            .unwrap_or_else(|err| panic!("read {fixture}: {err}"));
        let parsed: Value =
            serde_json::from_str(&raw).unwrap_or_else(|err| panic!("parse {fixture}: {err}"));
        assert!(
            parsed
                .get("operation_type")
                .and_then(Value::as_str)
                .is_some_and(|value| !value.trim().is_empty()),
            "{fixture} must record an `operation_type` so its published section can name it"
        );
    }
}

#[test]
fn cited_fixtures_are_deduplicated_and_include_both_directions() {
    // A fixture cited twice is one link, not two; a fixture cited zero times is
    // the failure the first test reports.
    let doc = "See `cargo-budget-report/fixtures/a_benchmark.json` and\n\
               [b](cargo-budget-report/fixtures/a_benchmark.json).\n\
               Also cargo-budget-report/fixtures/b_benchmark.json\n";
    assert_eq!(
        cited_fixtures(doc),
        [
            "cargo-budget-report/fixtures/a_benchmark.json",
            "cargo-budget-report/fixtures/b_benchmark.json",
        ]
    );
    assert!(cited_fixtures("no fixtures here").is_empty());
}
