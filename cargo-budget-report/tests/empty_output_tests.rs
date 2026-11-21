//! Issue #844: a run with no successful simulations must still honour the
//! requested output format instead of leaving `--json`/`--csv` consumers
//! with an empty document.

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
            fs::copy(entry.path(), &dst_path).expect("failed to copy fixture file");
        }
    }
}

/// The `no_exports_workspace` fixture contains no simulatable functions, so
/// every run through it hits the `measurements.is_empty()` early return.
fn setup_empty_workspace() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("failed to create tempdir");
    copy_dir_all(
        &manifest_dir().join("tests/fixtures/no_exports_workspace"),
        tmp.path(),
    );
    tmp
}

fn mocked_path() -> String {
    let real_path = std::env::var("PATH").unwrap_or_default();
    format!("{}:{}", fake_bin_dir().display(), real_path)
}

fn budget_report_cmd(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary should be built");
    cmd.current_dir(dir).env("PATH", mocked_path());
    cmd
}

#[test]
fn empty_run_with_json_writes_a_valid_envelope() {
    let workspace = setup_empty_workspace();

    let assert = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--json",
        ])
        .assert();
    // Exit code is preserved (the fixture has nothing simulatable, so the
    // run still fails); what matters here is that stdout is parseable.
    let output = assert.failure().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        !stdout.trim().is_empty(),
        "--json on an empty run must not produce a zero-byte document"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("--json on empty run must be valid JSON");
    assert_eq!(json["schema_version"], 1);
    assert_eq!(
        json["snapshots"].as_array().map(Vec::len),
        Some(0),
        "empty run snapshots: {stdout}"
    );
}

/// Validate a report document the way `json_output::validate_report_schema`
/// does for a wrapped envelope: an integer `schema_version` that is in the
/// accepted set (currently only `1`), plus a `snapshots` array.
///
/// This integration test runs against the built binary, not the crate's
/// modules, and `validate_report_schema` is `pub(crate)`, so it cannot be
/// called directly from here. The rule is mirrored rather than re-derived:
/// the unit tests in `src/json_output.rs` pin the function itself, and this
/// pins that the CLI's *empty* output satisfies it.
fn validate_report_schema(document: &serde_json::Value) -> Result<&Vec<serde_json::Value>, String> {
    let object = document
        .as_object()
        .ok_or_else(|| format!("expected a wrapped object, got: {document}"))?;

    match object
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
    {
        Some(1) => {}
        other => {
            return Err(format!(
                "missing, non-integer or unsupported schema_version ({other:?}): {document}"
            ))
        }
    }

    object
        .get("snapshots")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("missing or non-array `snapshots`: {document}"))
}

/// Issue #1104: an empty run's `--json` document must be a complete, valid
/// envelope — not just parseable JSON — because downstream CI tools read
/// `schema_version` before they look at `snapshots`, and parse `snapshots` as
/// an array even when nothing was measured.
#[test]
fn json_empty_output_produces_valid_envelope() {
    let workspace = setup_empty_workspace();

    let assert = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--json",
        ])
        .assert();
    // The fixture has nothing simulatable, so the run still fails by exit
    // code; the envelope contract is what this test is about.
    let output = assert.failure().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();

    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("empty --json output must be valid JSON");

    // (1) The version is the current schema version, as an integer.
    assert_eq!(json["schema_version"], 1, "stdout: {stdout}");
    assert!(
        json["schema_version"].is_u64(),
        "schema_version must be a JSON integer: {stdout}"
    );

    // (2) `snapshots` is present and specifically an empty array — `[]`, not
    // `null`, `{}` or an omitted key.
    assert!(
        json["snapshots"].is_array(),
        "snapshots must be an array: {stdout}"
    );
    assert_eq!(
        json["snapshots"].as_array().map(Vec::len),
        Some(0),
        "an empty run must emit an empty snapshots array: {stdout}"
    );

    // The envelope has no other top-level keys to confuse a consumer.
    let keys: Vec<&str> = json
        .as_object()
        .expect("envelope must be an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        vec!["schema_version", "snapshots"],
        "stdout: {stdout}"
    );

    // (3) The document passes schema validation, returning the empty array.
    let snapshots = validate_report_schema(&json)
        .unwrap_or_else(|err| panic!("empty JSON envelope must validate: {err}"));
    assert!(snapshots.is_empty(), "stdout: {stdout}");
}

#[test]
fn empty_run_with_csv_writes_the_header_row() {
    let workspace = setup_empty_workspace();

    let assert = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--csv",
        ])
        .assert();
    let output = assert.failure().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let mut lines = stdout.lines();
    assert_eq!(
        lines.next().unwrap_or(""),
        "package,function,metric,value",
        "empty --csv must still emit the header row: {stdout}"
    );
}

#[test]
fn empty_run_with_markdown_renders_header_and_footer() {
    let workspace = setup_empty_workspace();

    let assert = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--markdown",
        ])
        .assert();
    let output = assert.failure().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        stdout.contains("| Package | Function | Metric | Value |"),
        "empty --markdown keeps the table shell: {stdout}"
    );
}

#[test]
fn empty_run_with_html_still_writes_the_file() {
    let workspace = setup_empty_workspace();

    let assert = budget_report_cmd(workspace.path())
        .args([
            "budget-report",
            "--network",
            "local",
            "--source",
            "alice",
            "--html",
            "report.html",
        ])
        .assert();
    let output = assert.failure().get_output().clone();
    let html = fs::read_to_string(workspace.path().join("report.html"))
        .expect("HTML report must still be written on an empty run");
    assert!(
        html.contains("<html"),
        "HTML-on-empty behaviour is unchanged: {html:?} / stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
