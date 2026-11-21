//! Documentation parity for the trybuild fixture suite.
//!
//! `budget-macros/README.md` inventories every fixture under `tests/ui/` and
//! `tests/ui/pass/` and quotes, per fixture, a fragment of the diagnostic or
//! panic text it pins. This test keeps that inventory honest against the files:
//! the row set must equal the fixture set in both directions, and each quoted
//! fragment must still appear verbatim in the file it claims to come from.
//!
//! Only the crate's own files are read — nothing here invokes the compiler, so
//! the snapshots themselves stay covered by `tests/ui.rs`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// A fixture's table row: its crate-relative path, which directory it belongs
/// to, and the other two columns.
struct Row {
    path: String,
    dir: &'static str,
    purpose: String,
    claim: String,
}

/// The crate root, so the test runs from any working directory.
fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Reads a file, reporting which file could not be read.
fn read(path: &Path) -> String {
    match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => panic!("cannot read {}: {err}", path.display()),
    }
}

/// Crate-relative path of a `.rs` file sitting directly in `dir`.
fn fixture_paths(dir: &Path, prefix: &str) -> BTreeSet<String> {
    let entries =
        fs::read_dir(dir).unwrap_or_else(|err| panic!("cannot read {}: {err}", dir.display()));
    entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| matches!(path.extension(), Some(ext) if ext == "rs"))
        .map(|path| {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_else(|| panic!("{}: file name is not valid UTF-8", path.display()));
            format!("{prefix}/{name}")
        })
        .collect()
}

/// The two fixture directories, keyed by the prefix their rows use.
fn fixtures_on_disk() -> [(&'static str, BTreeSet<String>); 2] {
    let root = manifest_dir();
    [
        (
            "tests/ui",
            fixture_paths(&root.join("tests").join("ui"), "tests/ui"),
        ),
        (
            "tests/ui/pass",
            fixture_paths(&root.join("tests").join("ui").join("pass"), "tests/ui/pass"),
        ),
    ]
}

/// The fixture directory a row belongs to, or `None` when the row does not
/// name a fixture.
///
/// The README also carries a layout table whose first column is a `.rs` path
/// (`tests/ui.rs`, `tests/ui/support/mock_env.rs`); neither is a fixture, and
/// matching the whole parent directory is what keeps such a row out of the
/// inventory — a pass fixture and a compile-fail fixture in the same column
/// would otherwise be indistinguishable by prefix alone.
fn fixture_dir(path: &str) -> Option<&'static str> {
    let (dir, name) = path.rsplit_once('/').unwrap_or(("", path));
    if name.is_empty() || !path.ends_with(".rs") {
        return None;
    }
    match dir {
        "tests/ui" => Some("tests/ui"),
        "tests/ui/pass" => Some("tests/ui/pass"),
        _ => None,
    }
}

/// Every inventory row in the README.
///
/// A table row is five `|`-separated cells (`| a | b | c |` splits into five
/// pieces with empty outer ones); header and separator rows fall out because
/// their first cell is not a fixture path. The first cell loses its surrounding
/// backticks, which no path contains, while the claim column is kept as written
/// apart from whitespace: it is matched literally, and a claim may well start or
/// end with a backtick.
fn inventory_rows(readme: &str) -> Vec<Row> {
    readme
        .lines()
        .filter(|line| line.trim_start().starts_with('|'))
        .filter_map(|line| {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            let ["", raw_path, purpose, claim, ""] = cells.as_slice() else {
                return None;
            };
            let path = raw_path.trim_matches('`');
            let dir = fixture_dir(path)?;
            Some(Row {
                path: path.to_string(),
                dir,
                purpose: (*purpose).to_string(),
                claim: (*claim).to_string(),
            })
        })
        .collect()
}

fn inventory() -> (Vec<Row>, [(&'static str, BTreeSet<String>); 2]) {
    (
        inventory_rows(&read(&manifest_dir().join("README.md"))),
        fixtures_on_disk(),
    )
}

/// The rows are the fixture set — no fixture undocumented, no row left behind
/// by a rename or a deletion, and nothing listed twice.
#[test]
fn every_fixture_has_exactly_one_table_row() {
    let (rows, fixtures) = inventory();

    for (prefix, on_disk) in fixtures {
        let documented: Vec<&String> = rows
            .iter()
            .filter(|row| row.dir == prefix)
            .map(|row| &row.path)
            .collect();
        let unique: BTreeSet<String> = documented.iter().map(|path| (*path).clone()).collect();

        assert_eq!(
            documented.len(),
            unique.len(),
            "{prefix}: a fixture is listed more than once in README.md"
        );
        assert_eq!(
            unique.len(),
            on_disk.len(),
            "{prefix}: README.md lists {unique:?} but the directory holds {on_disk:?}"
        );

        let undocumented: Vec<&String> = on_disk.difference(&unique).collect();
        assert!(
            undocumented.is_empty(),
            "{prefix}: fixtures with no README.md row: {undocumented:?}"
        );
    }
}

/// Each compile-fail row still quotes text its pinned `.stderr` produces, and
/// each pass row still quotes text its own source contains.
#[test]
fn every_row_quotes_text_that_still_exists() {
    let (rows, _) = inventory();
    let root = manifest_dir();

    for row in &rows {
        assert!(
            !row.purpose.is_empty(),
            "{}: empty purpose column",
            row.path
        );
        assert!(!row.claim.is_empty(), "{}: empty claim column", row.path);

        let fixture = root.join(&row.path);
        let (checked, haystack) = if row.dir == "tests/ui/pass" {
            (row.path.clone(), read(&fixture))
        } else {
            let snapshot = fixture.with_extension("stderr");
            (snapshot.display().to_string(), read(&snapshot))
        };
        assert!(
            haystack.contains(&row.claim),
            "{}: README.md claims {:?} but it is not in {checked}",
            row.path,
            row.claim
        );
    }
}

/// The counts the prose quotes are the counts on disk, so a fixture added
/// without editing the sentence fails here too.
#[test]
fn documented_counts_match_the_files() {
    let readme = read(&manifest_dir().join("README.md"));

    for (prefix, on_disk) in fixtures_on_disk() {
        let phrase = match prefix {
            "tests/ui" => "compile-fail fixtures",
            _ => "pass fixtures",
        };
        let expected = format!("{} {}", on_disk.len(), phrase);
        assert!(
            readme.contains(&expected),
            "README.md must state the {prefix} count as {expected:?}"
        );
    }
}

/// The instruction a contributor cannot infer from the files — how to re-record
/// a snapshot — stays documented, along with how to run this check.
#[test]
fn regeneration_command_is_documented() {
    let readme = read(&manifest_dir().join("README.md"));

    for needle in [
        "TRYBUILD=overwrite cargo test -p budget-macros",
        "cargo test -p budget-macros --test fixture_inventory",
    ] {
        assert!(
            readme.contains(needle),
            "README.md must document {needle:?}"
        );
    }
}
