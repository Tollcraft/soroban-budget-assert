//! Cross-check the glossary's metric entries against the measurement
//! documentation.
//!
//! Issue #1063: `docs/src/glossary.md` claimed `cargo budget-report` "does
//! not report memory bytes", while `MEASUREMENTS.md` records network
//! `mem_bytes` figures and the CLI has extracted `result.cost.memBytes` since
//! Protocol 22. Documentation and measurement evidence had drifted apart with
//! nothing in CI comparing them.
//!
//! These tests pin two things:
//!
//! 1. every metric the glossary says the tool **measures** is actually
//!    described in `MEASUREMENTS.md`, so a metric cannot be added to (or
//!    demoted from) the glossary without the measurement page agreeing; and
//! 2. the Memory Bytes entry and the derivation summary agree with each other
//!    and with the CLI's real behaviour (`result.cost.memBytes`,
//!    `mem_limit`, `#[budget_mem_lt(N)]`).

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate lives one level below the repo root")
        .to_path_buf()
}

fn read(relative: &str) -> String {
    let path = repo_root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

fn glossary() -> String {
    read("docs/src/glossary.md")
}

fn measurements() -> String {
    read("MEASUREMENTS.md")
}

/// Returns the body of the `## <title>` section, up to the next level-2
/// heading.
fn section<'a>(markdown: &'a str, title: &str) -> Option<&'a str> {
    let marker = format!("## {title}");
    let start = markdown.find(&marker)?;
    let body = &markdown[start + marker.len()..];
    let end = body.find("\n## ").unwrap_or(body.len());
    Some(&body[..end])
}

/// The metrics the glossary documents as measured, paired with the spellings
/// `MEASUREMENTS.md` uses for them. At least one spelling must appear in the
/// measurement page.
const MEASURED_METRICS: &[(&str, &[&str])] = &[
    ("CPU Instructions", &["cpu instructions", "cpu"]),
    ("Read Bytes", &["read bytes"]),
    ("Write Bytes", &["write bytes"]),
    ("Memory Bytes", &["memory bytes", "mem_bytes", "membytes"]),
];

#[test]
fn every_measured_metric_in_the_glossary_appears_in_measurements() {
    let glossary = glossary();
    let measurements = measurements().to_lowercase();

    for (metric, needles) in MEASURED_METRICS {
        let body = section(&glossary, metric)
            .unwrap_or_else(|| panic!("glossary has no `## {metric}` section"));
        assert!(
            body.contains("**Measurement status**: **measures**"),
            "`## {metric}` must be documented as measured"
        );

        let mut documented = false;
        for needle in *needles {
            if measurements.contains(needle) {
                documented = true;
                break;
            }
        }
        assert!(
            documented,
            "MEASUREMENTS.md does not describe the glossary's measured metric {metric:?} \
             (looked for any of {needles:?})"
        );
    }
}

#[test]
fn memory_bytes_is_documented_as_measured_by_the_cli() {
    let glossary = glossary();
    let memory = section(&glossary, "Memory Bytes").expect("glossary has a Memory Bytes section");

    assert!(
        memory.contains("**Measurement status**: **measures**"),
        "Memory Bytes must be documented as measured, got:\n{memory}"
    );
    assert!(
        !memory.contains("does not measure"),
        "the stale `does not measure` claim is back:\n{memory}"
    );
    assert!(
        !memory.contains("does not report memory bytes"),
        "the stale `does not report memory bytes` claim is back:\n{memory}"
    );
    // The field the CLI actually reads, and the flag/macro that gate it.
    assert!(
        memory.contains("result.cost.memBytes"),
        "Memory Bytes should name the JSON-RPC field it is read from:\n{memory}"
    );
    assert!(
        memory.contains("mem_limit"),
        "Memory Bytes should mention the `mem_limit` check:\n{memory}"
    );
    assert!(
        memory.contains("budget_mem_lt"),
        "Memory Bytes should cross-reference the Tier A macro:\n{memory}"
    );
    assert!(
        memory.contains("MEASUREMENTS.md"),
        "Memory Bytes should link to the measured figures:\n{memory}"
    );
}

#[test]
fn measurements_page_documents_the_memory_fixture() {
    let measurements = measurements();
    let memory = section(&measurements, "Memory bytes")
        .expect("MEASUREMENTS.md has a `## Memory bytes` section");
    assert!(
        memory.to_lowercase().contains("mem_bytes"),
        "the Memory bytes section should reference the captured `mem_bytes` figure"
    );
}

#[test]
fn derivation_summary_agrees_that_memory_bytes_is_reported() {
    let glossary = glossary();
    let summary = section(&glossary, "Derivation summary")
        .expect("glossary has a Derivation summary section");

    assert!(
        summary.contains("result.cost.memBytes"),
        "the derivation summary should list Memory Bytes as a reported metric:\n{summary}"
    );
    assert!(
        !summary.contains("reported only by the local macro"),
        "the derivation summary still claims the CLI does not report memory:\n{summary}"
    );
}

#[test]
fn glossary_contains_no_stale_memory_disclaimer() {
    let glossary = glossary();
    assert!(
        !glossary.contains("does not report memory bytes"),
        "the stale Memory Bytes disclaimer reappeared in docs/src/glossary.md"
    );
}
