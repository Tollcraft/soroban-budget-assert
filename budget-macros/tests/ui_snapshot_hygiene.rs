//! `.stderr` snapshots must stay portable between machines (issue #1006).
//!
//! trybuild records the compiler's output verbatim, and a macro-expanded
//! diagnostic can carry an absolute path: a `env_file` path resolved against
//! the build directory, a `file!()`/`env!("CARGO_MANIFEST_DIR")` value that
//! reached a `compile_error!`, or a temporary directory from a fixture that
//! creates one. When that happens locally the host path is committed into the
//! snapshot, and every later run on a different machine fails on a line nobody
//! changed — the mismatch is in the working directory, not in the code.
//!
//! The harness in `tests/ui.rs` already checks that each compile-fail fixture
//! *has* a non-empty snapshot; this file checks what is *inside* them, so a
//! regenerated snapshot is reviewed for content as well as existence.
//!
//! Two checks run over the same set of files:
//!
//! * the host-path markers below, which cover the layouts a developer machine
//!   actually has (Linux home and temp, macOS home and the `/private/var/folders`
//!   and `/private/tmp` forms macOS resolves a temp path to);
//! * this build's own manifest directory, which catches any absolute path on
//!   the machine running the test even if it does not match a known layout.

use std::fs;
use std::path::{Path, PathBuf};

/// Substrings that identify a machine-specific absolute path. Each is paired
/// with the layout it names, so a failure says which host leaked in.
const HOST_PATH_MARKERS: &[(&str, &str)] = &[
    ("/home/", "a Linux home directory"),
    ("/Users/", "a macOS home directory"),
    ("/tmp/", "a POSIX temporary directory"),
    ("/private/tmp/", "the macOS-resolved temporary directory"),
    ("/var/folders/", "a macOS per-user temporary root"),
    (
        "/private/var/folders/",
        "the macOS-resolved per-user temporary root",
    ),
];

/// Root of the trybuild fixtures, anchored at the crate so the test runs from
/// any working directory — the same anchoring `tests/ui.rs` uses.
fn ui_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("ui")
}

/// Every `.rs` fixture in `dir`, mirroring trybuild's non-recursive glob.
fn rust_fixtures(dir: &Path) -> Vec<PathBuf> {
    let entries = fs::read_dir(dir).expect("cannot read fixture directory");
    let mut fixtures: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| matches!(path.extension(), Some(ext) if ext == "rs"))
        .collect();
    fixtures.sort();
    fixtures
}

/// Every `.stderr` file under `dir`, recursively, sorted. The walk is
/// recursive so a snapshot in a subdirectory cannot escape the check, unlike
/// trybuild's own glob.
fn stderr_snapshots(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("cannot read snapshot directory") {
        let path = entry.expect("unreadable directory entry").path();
        if path.is_dir() {
            stderr_snapshots(&path, found);
        } else if matches!(path.extension(), Some(ext) if ext == "stderr") {
            found.push(path);
        }
    }
    found.sort();
}

/// The line a snapshot would have to contain a marker on, for the failure
/// message to point at it.
fn offending_line(text: &str, marker: &str) -> Option<String> {
    text.lines()
        .find(|line| line.contains(marker))
        .map(|line| line.trim().to_string())
}

#[test]
fn snapshots_are_found_and_cover_the_fixture_set() {
    // A scan that silently finds nothing would pass forever, so the count is
    // asserted against the fixtures that require a snapshot: every compile-fail
    // `.rs` directly in `tests/ui/` must have a `.stderr` beside it.
    let mut snapshots = Vec::new();
    stderr_snapshots(&ui_dir(), &mut snapshots);
    let fixtures = rust_fixtures(&ui_dir());
    assert!(
        !snapshots.is_empty(),
        "no .stderr snapshots found under {}",
        ui_dir().display()
    );
    for fixture in &fixtures {
        let snapshot = fixture.with_extension("stderr");
        assert!(
            snapshot.is_file(),
            "{}: compile-fail fixture without a snapshot",
            fixture.display()
        );
    }
    assert!(
        snapshots.len() >= fixtures.len(),
        "found {} snapshots for {} compile-fail fixtures; the scan is not \
         reaching the fixtures it is meant to cover",
        snapshots.len(),
        fixtures.len()
    );
}

#[test]
fn no_snapshot_embeds_a_host_path() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let mut snapshots = Vec::new();
    stderr_snapshots(&ui_dir(), &mut snapshots);

    for snapshot in &snapshots {
        let text = fs::read_to_string(snapshot)
            .unwrap_or_else(|err| panic!("cannot read {}: {err}", snapshot.display()));

        for (marker, layout) in HOST_PATH_MARKERS {
            if let Some(line) = offending_line(&text, marker) {
                panic!(
                    "{}: snapshot embeds {layout} ({marker:?}), which only exists on \
                     the machine that generated it.\n  offending line: {line}\n\
                     Regenerate with TRYBUILD=overwrite from a checkout at a relative \
                     path, or remove the path from the diagnostic.",
                    snapshot.display()
                );
            }
        }

        if let Some(line) = offending_line(&text, manifest_dir) {
            panic!(
                "{}: snapshot embeds this build's manifest directory \
                 ({manifest_dir}), which is not portable.\n  offending line: {line}",
                snapshot.display()
            );
        }
    }
}
