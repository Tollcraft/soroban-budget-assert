//! Parity test between `docs/src/measurements.md` and the canonical
//! `MEASUREMENTS.md`.
//!
//! The book page designates the root file as canonical but used to restate its
//! benchmark tables by hand, so a figure could be updated in one file and left
//! stale in the other. The tables are now links, and this test keeps them that
//! way: a numeric benchmark row may not appear on the page unless the identical
//! row exists in the canonical file, and the page must link to the canonical
//! sections it defers to.

use std::fs;
use std::path::PathBuf;

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

const CANONICAL_URL: &str =
    "https://github.com/Tollcraft/soroban-budget-assert/blob/main/MEASUREMENTS.md";

/// Sections whose tables must be linked, not copied. Each anchor must appear in
/// a link to the canonical file.
const CANONICAL_ANCHORS: &[&str] = &[
    "cpu-instructions",
    "per-version-calibration-table",
    "cross-version-comparison-local-only",
    "authorization-require_auth-measurement",
    "memory-bytes",
];

/// Markdown table rows, normalized so column padding and the Unicode minus do
/// not count as a difference.
fn table_rows(markdown: &str) -> Vec<String> {
    markdown
        .lines()
        .filter(|line| line.trim_start().starts_with('|'))
        .map(|line| {
            line.split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .replace('\u{2212}', "-")
        })
        .collect()
}

/// A row that quotes a benchmark figure: at least one thousands-grouped number
/// (`2,864,886`). This deliberately ignores issue numbers, versions and the
/// small counts in prose tables such as the column reference.
fn quotes_benchmark_figure(row: &str) -> bool {
    row.split(|c: char| !c.is_ascii_digit() && c != ',')
        .filter(|token| token.contains(','))
        .any(|token| {
            let groups: Vec<&str> = token.split(',').collect();
            groups.len() > 1
                && groups[0].len() <= 3
                && groups[1..]
                    .iter()
                    .all(|group| group.len() == 3 && group.chars().all(|c| c.is_ascii_digit()))
        })
}

#[test]
fn page_defers_to_the_canonical_measurement_sections() {
    let page = read("docs/src/measurements.md");
    for anchor in CANONICAL_ANCHORS {
        let link = format!("{CANONICAL_URL}#{anchor}");
        assert!(
            page.contains(&link),
            "docs/src/measurements.md must link to the canonical section {link} instead of \
             restating its table"
        );
    }
}

#[test]
fn shared_benchmark_rows_match_the_canonical_file() {
    let page = read("docs/src/measurements.md");
    let canonical = read("MEASUREMENTS.md");
    let canonical_rows = table_rows(&canonical);

    let mut checked = 0;
    for row in table_rows(&page) {
        if !quotes_benchmark_figure(&row) {
            continue;
        }
        checked += 1;
        assert!(
            canonical_rows.contains(&row),
            "docs/src/measurements.md quotes a benchmark row that MEASUREMENTS.md does not \
             contain:\n  {row}\nUpdate MEASUREMENTS.md and link it from the page, or drop the \
             duplicated row"
        );
    }
    assert!(
        checked > 0,
        "no benchmark rows were compared; the parity scan is checking nothing"
    );
}

#[test]
fn duplicated_canonical_tables_are_not_restated_on_the_page() {
    let page = read("docs/src/measurements.md");
    // Headers of the tables whose canonical copy lives in MEASUREMENTS.md.
    for header in [
        "| Operation type | Local estimate | Network figure | Delta | Fixture | Build profile | Toolchain | Date |",
        "| SDK version (pinned) | SDK version (resolved) | Local CPU estimate | Local mem estimate | Network CPU | Network mem | Delta CPU | Date | Toolchain |",
        "| Operation type | Local CPU | Local mem | Network CPU | Network mem | Delta CPU | Fixture | Build profile | Toolchain | Date |",
    ] {
        assert!(
            !page.contains(header),
            "docs/src/measurements.md restates a canonical table:\n  {header}\nLink the \
             MEASUREMENTS.md section instead so the figures cannot drift"
        );
    }
}

#[test]
fn page_links_only_to_fixtures_that_exist() {
    let page = read("docs/src/measurements.md");
    let root = repo_root();
    let mut rest = page.as_str();
    let prefix = "cargo-budget-report/fixtures/";
    let mut checked = 0;
    while let Some(index) = rest.find(prefix) {
        let tail = &rest[index..];
        let end = tail
            .find(|c: char| c.is_whitespace() || matches!(c, ')' | '"' | '`' | '>' | ',' | ';'))
            .unwrap_or(tail.len());
        let path = &tail[..end];
        // A link may carry a trailing fragment or query; the file part is
        // everything before it.
        let file = path.split(['#', '?']).next().unwrap_or(path);
        assert!(
            root.join(file).is_file(),
            "docs/src/measurements.md links `{path}`, which does not exist; the page must \
             cite the capture record a figure came from"
        );
        checked += 1;
        rest = &tail[end..];
    }
    assert!(
        checked > 0,
        "docs/src/measurements.md no longer links any capture fixture; the page must cite the \
         records behind its figures"
    );
}

#[test]
fn benchmark_figure_detection_ignores_versions_and_issue_numbers() {
    assert!(quotes_benchmark_figure("| get | 3,485 | 3,439 |"));
    assert!(quotes_benchmark_figure(
        "| SDK 22 | 6,606,666 | 1,942,982 |"
    ));
    assert!(!quotes_benchmark_figure("| #1083 | 20.5.0 | 1.92.0 |"));
    assert!(!quotes_benchmark_figure("| 1000 | pending |"));
    // A grouped input size still sits in a benchmark row, so it counts.
    assert!(quotes_benchmark_figure("| 1,000 | pending |"));
}

#[test]
fn table_rows_normalize_padding_and_unicode_minus() {
    assert_eq!(
        table_rows("|a|  b |\n|---||---|"),
        ["|a| b |", "|---||---|"]
    );
    assert_eq!(table_rows("| −17.2% |"), ["| -17.2% |"]);
}
