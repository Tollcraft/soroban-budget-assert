//! Integration tests for Markdown table cell escaping (issue #1048).
//!
//! When a package or function name contains a vertical pipe (`|`) or embedded
//! newlines, the markdown renderer must escape them so that GFM table parsers
//! do not misinterpret the delimiters.

use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fake_bin_dir() -> PathBuf {
    manifest_dir().join("tests/fixtures/fake_bin")
}

fn copy_dir_all(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("failed to create destination directory");
    for entry in fs::read_dir(src).expect("failed to read source directory") {
        let entry = entry.expect("failed to read directory entry");
        let file_type = entry.file_type().expect("failed to read file type");
        let dst_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &dst_path);
        } else {
            fs::copy(entry.path(), &dst_path).expect("failed to copy file");
        }
    }
}

/// Build a JSON report containing a snapshot with the given package name.
fn report_json(package: &str, function: &str) -> String {
    serde_json::json!({
        "schema_version": 1,
        "snapshots": [{
            "package": package,
            "function": function,
            "metric": "CPU Instructions",
            "value": 42
        }]
    })
    .to_string()
}

/// Run `cargo-budget-report --markdown --from <file>` and return stdout.
fn render_markdown_from(dir: &Path, report_path: &Path) -> String {
    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary should be built");
    cmd.current_dir(dir)
        .arg("budget-report")
        .arg("--markdown")
        .arg("--from")
        .arg(report_path)
        .env("PATH", fake_bin_dir());
    let output = cmd.output().expect("failed to execute command");
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn pipe_in_package_name_is_escaped_in_markdown_table() {
    let dir = tempfile::tempdir().expect("tempdir");
    let fixtures = manifest_dir().join("tests/fixtures/workspace");
    if fixtures.exists() {
        copy_dir_all(&fixtures, dir.path());
    }

    let report_path = dir.path().join("pipe-report.json");
    fs::write(&report_path, report_json("a|b", "do_work")).expect("write");

    let md = render_markdown_from(dir.path(), &report_path);

    // The pipe in the package name must be escaped
    assert!(
        md.contains(r"a\|b"),
        "pipe in package name should be escaped as `a\\|b`, got:\n{md}"
    );
    // The escaped row should not break the table structure: count pipes per
    // data row (excluding header rows starting with `|---`)
    for line in md.lines() {
        if line.starts_with("| ") && !line.starts_with("|---") {
            // A well-formed 4-column row has exactly 5 pipe characters
            // (opening, 3 separators, closing)
            let pipe_count = line.matches('|').count() - line.matches(r"\|").count();
            assert!(
                pipe_count >= 5,
                "row should have at least 5 unescaped pipes for 4 columns, got {pipe_count}: {line}"
            );
        }
    }
}

#[test]
fn newline_in_function_name_is_collapsed_in_markdown_table() {
    let dir = tempfile::tempdir().expect("tempdir");
    let fixtures = manifest_dir().join("tests/fixtures/workspace");
    if fixtures.exists() {
        copy_dir_all(&fixtures, dir.path());
    }

    let report_path = dir.path().join("newline-report.json");
    fs::write(&report_path, report_json("pkg", "fn\nwith_newline")).expect("write");

    let md = render_markdown_from(dir.path(), &report_path);

    // Newlines must not appear inside a table row
    for line in md.lines() {
        if line.starts_with("| ") {
            assert!(
                !line.contains('\n'),
                "table row must not contain embedded newlines: {line}"
            );
        }
    }
    // The function name should appear with the newline collapsed to a space
    assert!(
        md.contains("fn with_newline"),
        "newline should be collapsed to space, got:\n{md}"
    );
}

#[test]
fn pipe_and_newline_combined_in_name() {
    let dir = tempfile::tempdir().expect("tempdir");
    let fixtures = manifest_dir().join("tests/fixtures/workspace");
    if fixtures.exists() {
        copy_dir_all(&fixtures, dir.path());
    }

    let report_path = dir.path().join("combined-report.json");
    fs::write(&report_path, report_json("x|y\nz", "a\r\nb|c")).expect("write");

    let md = render_markdown_from(dir.path(), &report_path);

    assert!(
        md.contains(r"x\|y z"),
        "pipe escaped and newline collapsed in package, got:\n{md}"
    );
    assert!(
        md.contains(r"a b\|c"),
        "carriage-return+newline collapsed and pipe escaped in function, got:\n{md}"
    );
}
