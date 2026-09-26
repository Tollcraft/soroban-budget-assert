//! Compile-time behavior of the budget macros.
//!
//! `tests/ui/*.rs` must fail to compile, with the diagnostic pinned in the
//! matching `.stderr`. `tests/ui/pass/*.rs` must compile *and run*: those cases
//! exercise each supported test-body shape against the mock `env` in
//! `tests/ui/support/mock_env.rs` and assert which cost and limit the injected
//! check reports, so a body shape that silently stops being checked fails here.
//!
//! Regenerate the `.stderr` snapshots with `TRYBUILD=overwrite cargo test -p budget-macros`.
//!
//! On top of the trybuild run itself, the unit tests below hold the harness's
//! *inputs* to account: every compile-fail fixture must ship a pinned `.stderr`
//! (trybuild silently treats a missing snapshot as new), every pass fixture must
//! be a standalone program, and the fixtures named by issues #623, #626, #627
//! and #630 are asserted directly so a rename or a dropped snapshot is a test
//! failure rather than a quiet loss of coverage.

use std::fs;
use std::path::{Path, PathBuf};

/// Root of the trybuild fixtures, anchored at the crate so the tests run from
/// any working directory.
fn ui_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("ui")
}

/// `.rs` files directly inside `dir`, sorted, mirroring trybuild's
/// `tests/ui/*.rs` (and `tests/ui/pass/*.rs`) non-recursive globs.
fn rust_fixtures(dir: &Path) -> Vec<PathBuf> {
    let entries = fs::read_dir(dir).expect("cannot read fixture directory");
    let mut fixtures: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| matches!(path.extension(), Some(ext) if ext == "rs"))
        .collect();
    fixtures.sort();
    fixtures
}

/// Reads a file, failing with its path when it cannot be read.
fn read(path: &Path) -> String {
    match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => panic!("cannot read {}: {err}", path.display()),
    }
}

/// Reads a compile-fail fixture and its pinned snapshot.
fn fixture_and_snapshot(name: &str) -> (String, String) {
    let fixture = ui_dir().join(name);
    let snapshot = fixture.with_extension("stderr");
    (read(&fixture), read(&snapshot))
}

#[test]
fn ui_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
    t.pass("tests/ui/pass/*.rs");
}

#[test]
fn harness_covers_both_compile_fail_and_pass_fixtures() {
    assert!(
        !rust_fixtures(&ui_dir()).is_empty(),
        "no compile-fail fixtures found"
    );
    let pass_dir = ui_dir().join("pass");
    assert!(
        !rust_fixtures(&pass_dir).is_empty(),
        "no pass fixtures found"
    );
}

#[test]
fn every_compile_fail_fixture_pins_its_diagnostic() {
    for fixture in rust_fixtures(&ui_dir()) {
        let snapshot = fixture.with_extension("stderr");
        assert!(
            snapshot.is_file(),
            "{}: snapshot missing",
            fixture.display()
        );
        let text = read(&snapshot);
        assert!(
            !text.trim().is_empty(),
            "{}: snapshot empty",
            snapshot.display()
        );
    }
}

#[test]
fn every_pass_fixture_is_a_standalone_program() {
    for fixture in rust_fixtures(&ui_dir().join("pass")) {
        let text = read(&fixture);
        assert!(
            text.contains("fn main"),
            "{}: needs main()",
            fixture.display()
        );
    }
}

/// Issue #627: `budget_read_bytes_lt` must reject a body with no `env` source.
#[test]
fn missing_env_read_fixture_pins_the_missing_env_error() {
    let (source, snapshot) = fixture_and_snapshot("missing_env_read.rs");

    assert!(
        source.contains("budget_read_bytes_lt"),
        "must use the read-bytes macro"
    );
    assert!(!source.contains("env ="), "must omit the env source");
    assert!(!source.contains("env_file"), "must omit an env_file source");
    assert!(
        snapshot.contains("budget_read_bytes_lt"),
        "diagnostic must name the macro"
    );
    assert!(
        snapshot.contains("E0423"),
        "expected rustc's E0423 for a missing env"
    );
}

/// Issue #626: percentages outside 1-100 must be rejected at compile time.
#[test]
fn pct_out_of_range_fixture_pins_the_range_error() {
    let (source, snapshot) = fixture_and_snapshot("pct_out_of_range.rs");

    assert!(source.contains("pct = 0"), "must use an out-of-range value");
    assert!(
        snapshot.contains("between 1 and 100"),
        "diagnostic must state the range"
    );
}

/// Issue #630: `pct` without a reference (`of = ...`) must be rejected.
#[test]
fn pct_no_of_fixture_pins_the_missing_of_error() {
    let (source, snapshot) = fixture_and_snapshot("pct_no_of.rs");

    assert!(source.contains("pct = 25"), "must request a percentage");
    assert!(
        snapshot.contains("requires `of`"),
        "diagnostic must explain the missing reference"
    );
}
