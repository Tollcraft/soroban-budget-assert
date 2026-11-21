//! GFM table-cell escaping for `--markdown` (issue #1048).
//!
//! A raw `|` in a package or function name is a table column delimiter, so
//! emitting it verbatim shifted every following metric cell one column to the
//! right. These tests run the real binary over a report whose names contain
//! pipes and line breaks, and assert both the escaped cells and that every
//! table row keeps exactly the header's column count.

use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};

fn budget_report_cmd(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary should be built");
    cmd.current_dir(dir);
    cmd
}

fn write_report(dir: &Path, report: &serde_json::Value) -> PathBuf {
    let path = dir.join("report.json");
    fs::write(
        &path,
        serde_json::to_string(report).expect("fixture serialises"),
    )
    .expect("failed to write the report fixture");
    path
}

/// Count the `|` characters that GFM parses as delimiters: a pipe escaped
/// with a backslash does not split the cell.
fn delimiter_count(line: &str) -> usize {
    let mut count = 0;
    let mut escaped = false;
    for ch in line.chars() {
        match ch {
            '\\' => escaped = !escaped,
            '|' => {
                if !escaped {
                    count += 1;
                }
                escaped = false;
            }
            _ => escaped = false,
        }
    }
    count
}

fn table_rows(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .filter(|line| line.starts_with('|'))
        .collect()
}

/// Every table row must use the same number of delimiters as the header, or a
/// downstream Markdown parser misattributes the metric cells.
fn assert_columns_aligned(stdout: &str) {
    let rows = table_rows(stdout);
    assert!(!rows.is_empty(), "expected a markdown table:\n{stdout}");
    let expected = delimiter_count(rows[0]);
    for row in &rows {
        assert_eq!(
            delimiter_count(row),
            expected,
            "column count drifted in row {row:?}\n--- full output ---\n{stdout}"
        );
    }
}

#[test]
fn pipes_in_package_and_function_names_are_escaped() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let path = write_report(
        dir.path(),
        &serde_json::json!([
            {
                "package": "amm|pool",
                "function": "do|work",
                "metric": "CPU Instructions",
                "value": 1234
            }
        ]),
    );

    let output = budget_report_cmd(dir.path())
        .args(["budget-report", "--markdown", "--from"])
        .arg(&path)
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("| amm\\|pool | do\\|work | CPU Instructions | 1,234 |"),
        "pipes must be escaped as \\|:\n{stdout}"
    );
    assert_columns_aligned(&stdout);
}

#[test]
fn newlines_in_names_are_collapsed_into_single_row_cells() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let path = write_report(
        dir.path(),
        &serde_json::json!([
            {
                "package": "multi\nline",
                "function": "crlf\r\nand\n\nruns",
                "metric": "Read Bytes",
                "value": 42
            }
        ]),
    );

    let output = budget_report_cmd(dir.path())
        .args(["budget-report", "--markdown", "--from"])
        .arg(&path)
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("| multi line | crlf and runs | Read Bytes | 42 |"),
        "line breaks must collapse to a single space:\n{stdout}"
    );
    let data_rows: Vec<&str> = table_rows(&stdout)
        .into_iter()
        .filter(|line| line.contains("Read Bytes"))
        .collect();
    assert_eq!(data_rows.len(), 1, "one report row must stay one row");
    assert_columns_aligned(&stdout);
}

#[test]
fn escaped_names_keep_check_columns_aligned() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let path = write_report(
        dir.path(),
        &serde_json::json!([
            {
                "package": "pkg|with|pipes",
                "function": "fn|\nname",
                "metric": "Write Bytes",
                "value": 4096,
                "limit": 1000,
                "pass": false
            }
        ]),
    );

    let output = budget_report_cmd(dir.path())
        .args(["budget-report", "--markdown", "--from"])
        .arg(&path)
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("| Package | Function | Metric | Value | Limit | Status |"),
        "expected the check header:\n{stdout}"
    );
    assert!(
        stdout.contains("| pkg\\|with\\|pipes | fn\\| name | Write Bytes | 4,096 | 1,000 | FAIL |"),
        "{stdout}"
    );
    assert_columns_aligned(&stdout);
}
