//! Issue #1012 — pin the mutual-exclusion diagnostic clap prints for
//! `cargo-budget-report --json --csv`.
//!
//! `BudgetReportArgs::json` declares `conflicts_with = "csv"` in
//! `cargo-budget-report/src/cli/args.rs`, so clap rejects the pair before the
//! binary reaches the reporter. The `--json` flag alone must still succeed and
//! `--csv` alone must still succeed; the whole point of pinning this at the
//! process boundary is that a future refactor could quietly swap the clap-level
//! rejection for an internal `Error::Message` (which would exit with `1` and
//! lose the "usage" framing) and no unit test would notice.
//!
//! Asserted here for **both** orderings of the flags, because `conflicts_with`
//! is symmetric only if clap still enforces it in both directions after the
//! argument struct changes: the reverse order is the one a regression would
//! miss.

use assert_cmd::Command;
use predicates::prelude::*;

/// Builds a fresh `cargo-budget-report` invocation.
///
/// The conflict fires before the binary ever touches the workspace, so no
/// mock-CWD fixture is required — a plain subprocess is enough and keeps the
/// test independent of the fixture-copy machinery `exit_code_tests.rs` uses.
fn cargo_budget_report() -> Command {
    Command::cargo_bin("cargo-budget-report").expect("cargo-budget-report binary must be built")
}

/// Clap's usage-error exit code. `clap::Error::exit()` for a conflict
/// terminates the process with `2` and the `Usage: cargo budget-report ...`
/// prefix on stderr — matching `--help`-style exits for parse failures.
const CLAP_USAGE_EXIT_CODE: i32 = 2;

#[test]
fn json_then_csv_is_rejected_with_a_clap_usage_error() {
    cargo_budget_report()
        .args(["budget-report", "--json", "--csv"])
        .assert()
        .code(CLAP_USAGE_EXIT_CODE)
        .stderr(predicate::str::contains("--json"))
        .stderr(predicate::str::contains("--csv"))
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn csv_then_json_is_rejected_with_a_clap_usage_error() {
    // `conflicts_with` is declared on `--json`, so the reverse order is the
    // path a regression in clap's symmetric handling would break — pin it.
    cargo_budget_report()
        .args(["budget-report", "--csv", "--json"])
        .assert()
        .code(CLAP_USAGE_EXIT_CODE)
        .stderr(predicate::str::contains("--json"))
        .stderr(predicate::str::contains("--csv"))
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn the_conflict_message_names_both_flags_as_the_offenders() {
    // Both flags must appear on stderr, otherwise the operator cannot tell
    // which pair conflicted when several mutually-exclusive groups exist.
    let output = cargo_budget_report()
        .args(["budget-report", "--json", "--csv"])
        .output()
        .expect("binary should run");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--json"),
        "conflict stderr must name `--json`: {stderr}"
    );
    assert!(
        stderr.contains("--csv"),
        "conflict stderr must name `--csv`: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(CLAP_USAGE_EXIT_CODE),
        "conflict must exit with clap's usage code (2), not an internal \
         reporter error code: {stderr}",
    );
}

#[test]
fn json_alone_is_accepted_by_the_parser() {
    // A bare `--json` on an empty workspace still hits the reporter, so we
    // cannot assert exit 0 here; what the parse *must not* do is fail with
    // the clap conflict message. Any non-`conflict` outcome proves the flag
    // parses on its own.
    let output = cargo_budget_report()
        .args(["budget-report", "--json"])
        .output()
        .expect("binary should run");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("cannot be used with"),
        "`--json` alone must not trigger the conflict path: {stderr}"
    );
}

#[test]
fn csv_alone_is_accepted_by_the_parser() {
    let output = cargo_budget_report()
        .args(["budget-report", "--csv"])
        .output()
        .expect("binary should run");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("cannot be used with"),
        "`--csv` alone must not trigger the conflict path: {stderr}"
    );
}
