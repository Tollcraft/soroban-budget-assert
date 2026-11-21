//! Versioned JSON envelope for `cargo budget-report --json`.
//!
//! `--json` is the machine-readable interface, so its document shape is a
//! compatibility surface: CI scripts and downstream tools parse it. Rather
//! than emitting a bare array of snapshots, the output is wrapped in an
//! envelope carrying [`SCHEMA_VERSION`], which lets a consumer check the
//! version once and reject (or adapt to) a document it does not understand —
//! instead of failing later, somewhere deep in its own parsing, on a field
//! that moved.
//!
//! The wrapper is deliberately additive. Every snapshot inside `snapshots` is
//! the same [`crate::CostReport`] row the table, CSV and Markdown renderers
//! print, so nothing about a snapshot has to be re-learned for JSON output.
//! `json_output::render_json` is the single place that produces the envelope;
//! the versioning policy itself is documented in `docs/src/reference.md`.

use crate::error::{Error, Result};
use serde::Serialize;
use std::path::Path;

/// Current JSON schema version for budget report output.
///
/// Increment this value when the JSON structure changes in a way that
/// requires consumers to update their parsing logic (see
/// `docs/src/reference.md` for the full versioning policy). Removing a field,
/// renaming one, changing a field's type, or changing the meaning of an
/// existing value all qualify. *Adding* a field does not: a consumer that
/// ignores unknown fields keeps working, which is why the envelope starts at
/// `1` rather than tracking every additive change.
pub(crate) const SCHEMA_VERSION: u32 = 1;

/// Schema versions accepted by report readers ([`crate::load_cost_reports`]
/// and [`crate::derive::load_tier_b_report`]).
///
/// `SCHEMA_VERSION` is the single source of truth for the latest version; this
/// list defines the complete set of supported versions so that readers never
/// disagree on compatibility.
pub(crate) const ACCEPTED_SCHEMA_VERSIONS: &[u32] = &[SCHEMA_VERSION];

/// Validate a JSON report document's schema version.
///
/// Returns `Ok(Option<&'a Vec<serde_json::Value>>)` where:
/// - `Ok(None)` means the document is a historical bare array (implicit version 0).
/// - `Ok(Some(snapshots))` means a valid wrapped envelope with a supported version was parsed,
///   returning a reference to the `snapshots` array.
/// - `Err(Error::Json(...))` (mapping to exit code 3 / [`crate::error::EXIT_CONFIG_ERROR`])
///   if the document is wrapped but has a missing, null, non-integer, negative, or
///   unsupported `schema_version`.
///
/// Crucially, `schema_version` is validated *first* before parsing the `snapshots` array,
/// ensuring that unsupported versions are reported immediately even if the snapshots
/// would fail to parse under the current types.
/// Top-level keys recognised in a wrapped JSON report envelope.
///
/// Any key not in this set triggers a validation error so that typos like
/// `snapshot` (instead of `snapshots`) are caught immediately.
const ALLOWED_TOP_LEVEL_KEYS: &[&str] = &[
    "schema_version",
    "snapshots",
    "generated_at",
    "generator_version",
    "_metadata",
];

pub(crate) fn validate_report_schema<'a>(
    root: &'a serde_json::Value,
    path: &Path,
) -> Result<Option<&'a Vec<serde_json::Value>>> {
    if root.is_array() {
        return Ok(None);
    }
    if let Some(obj) = root.as_object() {
        let version_val = match obj.get("schema_version") {
            Some(v) => v,
            None => {
                return Err(Error::Json(serde::de::Error::custom(format!(
                    "missing schema_version in wrapped report {}; accepted versions are {:?}",
                    path.display(),
                    ACCEPTED_SCHEMA_VERSIONS
                ))));
            }
        };

        if version_val.is_null() {
            return Err(Error::Json(serde::de::Error::custom(format!(
                "invalid schema_version: null in {}; accepted versions are {:?}",
                path.display(),
                ACCEPTED_SCHEMA_VERSIONS
            ))));
        }

        if let Some(n) = version_val.as_u64() {
            if let Ok(v32) = u32::try_from(n) {
                if ACCEPTED_SCHEMA_VERSIONS.contains(&v32) {
                    // Reject unknown top-level keys before accepting the report.
                    for key in obj.keys() {
                        if !ALLOWED_TOP_LEVEL_KEYS.contains(&key.as_str()) {
                            return Err(Error::Json(serde::de::Error::custom(format!(
                                "unknown top-level key `{}` in {}; allowed keys are {:?}",
                                key,
                                path.display(),
                                ALLOWED_TOP_LEVEL_KEYS
                            ))));
                        }
                    }

                    let snapshots =
                        obj.get("snapshots")
                            .and_then(|s| s.as_array())
                            .ok_or_else(|| {
                                Error::Json(serde::de::Error::custom(format!(
                                    "missing or invalid `snapshots` array in wrapped report {}",
                                    path.display()
                                )))
                            })?;
                    return Ok(Some(snapshots));
                }
            }
            return Err(Error::Json(serde::de::Error::custom(format!(
                "unsupported schema_version {} in {}; accepted versions are {:?}",
                n,
                path.display(),
                ACCEPTED_SCHEMA_VERSIONS
            ))));
        } else {
            return Err(Error::Json(serde::de::Error::custom(format!(
                "invalid schema_version {} in {}; accepted versions are {:?}",
                version_val,
                path.display(),
                ACCEPTED_SCHEMA_VERSIONS
            ))));
        }
    }

    Err(Error::Json(serde::de::Error::custom(format!(
        "invalid report JSON in {}: expected bare array or wrapped object with `schema_version` and `snapshots`",
        path.display()
    ))))
}

/// Top-level wrapper emitted by `cargo budget-report --json`.
///
/// Every JSON document produced by the budget report is an object with a
/// `schema_version` integer and a `snapshots` array. The individual
/// snapshot objects are unchanged from the pre-versioning format.
///
/// ```json
/// {
///   "schema_version": 1,
///   "snapshots": [
///     {
///       "package": "amm-pool-contract",
///       "function": "do_expensive_work",
///       "metric": "CPU Instructions",
///       "value": 2770850,
///       "limit": 3500000,
///       "pass": true
///     }
///   ]
/// }
/// ```
///
/// The `limit` and `pass` fields are present in `--check` mode only, and
/// `Option` fields are omitted entirely when unset (see [`crate::CostReport`]).
fn now_utc_iso8601() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = now / 86_400;
    let rem = now % 86_400;
    let hh = rem / 3600;
    let mm = (rem % 3600) / 60;
    let ss = rem % 60;

    let mut year = 1970u64;
    let mut remaining_days = days;
    let is_leap_year =
        |y: u64| (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400);
    loop {
        let is_leap = is_leap_year(year);
        let year_len = if is_leap { 366 } else { 365 };
        if remaining_days < year_len {
            break;
        }
        remaining_days -= year_len;
        year += 1;
    }
    let is_leap = is_leap_year(year);
    let month_lengths = [
        31u64,
        if is_leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u64;
    for &len in &month_lengths {
        if remaining_days < len {
            break;
        }
        remaining_days -= len;
        month += 1;
    }
    let day = remaining_days + 1;
    format!("{year:04}-{month:02}-{day:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

#[derive(Serialize)]
pub(crate) struct BudgetReportJson<'a> {
    /// Schema version of the document being emitted; always
    /// [`SCHEMA_VERSION`] for as long as this type is the only writer.
    schema_version: u32,
    /// Timestamp when this report envelope was generated (UTC ISO-8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    generated_at: Option<String>,
    /// Cargo package version of the tool generating the report.
    #[serde(skip_serializing_if = "Option::is_none")]
    generator_version: Option<&'static str>,
    /// The report rows, borrowed rather than owned: rendering only serializes
    /// them, so cloning each row into the envelope would allocate for nothing.
    snapshots: &'a [crate::CostReport],
}

/// Wrap the given report rows in the versioned JSON envelope.
///
/// Returns pretty-printed JSON (two-space indent) because the output is read
/// by a human in CI logs at least as often as it is piped into a parser.
///
/// # Panics
///
/// The `expect` below is unreachable in practice: every field of the envelope
/// is a `u32`, a slice of [`crate::CostReport`], or — inside `CostReport` — a
/// `String`, a `&'static str` or an `Option` of a primitive, none of which can
/// fail to serialize. Returning a `Result` would push an error onto callers
/// that have no way to act on it, so the invariant is documented instead.
pub(crate) fn render_json(reports: &[crate::CostReport]) -> String {
    // The envelope borrows `reports` for the duration of the call; the only
    // allocation here is the output `String` itself.
    let wrapper = BudgetReportJson {
        schema_version: SCHEMA_VERSION,
        generated_at: Some(now_utc_iso8601()),
        generator_version: Some(env!("CARGO_PKG_VERSION")),
        snapshots: reports,
    };
    serde_json::to_string_pretty(&wrapper).expect("report serialization should not fail")
}

#[cfg(test)]
mod tests {
    //! The envelope is the compatibility surface for `--json` consumers, so
    //! these tests pin its shape: which top-level keys exist, how many
    //! snapshots it carries, and which snapshot fields are omitted when unset.
    use super::*;
    use crate::CostReport;

    /// A fully populated row — the shape `--check` emits for a metric that has
    /// both a measured value and a configured limit.
    fn report(package: &str, function: &str) -> CostReport {
        CostReport {
            package: package.to_string(),
            function: function.to_string(),
            metric: "CPU Instructions",
            value: Some(12345),
            limit: Some(20000),
            pass: Some(true),
        }
    }

    #[test]
    fn budget_report_json_contains_schema_version() {
        let reports = vec![CostReport {
            package: "test-pkg".to_string(),
            function: "test_fn".to_string(),
            metric: "CPU Instructions",
            value: Some(12345),
            limit: None,
            pass: None,
        }];

        let json_str = render_json(&reports);
        let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(json["schema_version"], SCHEMA_VERSION);
        assert!(json["snapshots"].is_array());
        assert_eq!(json["snapshots"][0]["package"], "test-pkg");
        assert_eq!(json["snapshots"][0]["value"], 12345);
    }

    #[test]
    fn schema_version_matches_documented_current_version() {
        assert_eq!(SCHEMA_VERSION, 1);
    }

    #[test]
    fn empty_report_list_renders_an_empty_snapshots_array() {
        // A workspace where nothing was measured still has to produce a
        // document a consumer can parse.
        let json_str = render_json(&[]);
        let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(json["schema_version"], SCHEMA_VERSION);
        assert_eq!(json["snapshots"].as_array().map(Vec::len), Some(0));
    }

    #[test]
    fn envelope_carries_schema_version_metadata_and_snapshots() {
        // Adding a top-level field is a schema change, so it should show up as
        // a deliberate edit to this assertion rather than as a side effect.
        let json_str = render_json(&[report("pkg", "fn")]);
        let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        let keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            vec![
                "generated_at",
                "generator_version",
                "schema_version",
                "snapshots"
            ]
        );
        assert_eq!(json["generator_version"], env!("CARGO_PKG_VERSION"));
        let gen_at = json["generated_at"].as_str().unwrap();
        assert!(gen_at.ends_with('Z'));
        assert_eq!(gen_at.len(), 20); // YYYY-MM-DDTHH:MM:SSZ
    }

    #[test]
    fn row_order_is_preserved() {
        let reports = vec![report("pkg-b", "second"), report("pkg-a", "first")];
        let json: serde_json::Value = serde_json::from_str(&render_json(&reports)).unwrap();
        assert_eq!(json["snapshots"][0]["package"], "pkg-b");
        assert_eq!(json["snapshots"][1]["package"], "pkg-a");
    }

    #[test]
    fn unset_option_fields_are_omitted_from_a_snapshot() {
        // Without `--check` there is no limit to report; the key is skipped
        // rather than emitted as `null`.
        let reports = vec![CostReport {
            package: "pkg".to_string(),
            function: "fn".to_string(),
            metric: "Memory Bytes",
            value: Some(7),
            limit: None,
            pass: None,
        }];
        let json: serde_json::Value = serde_json::from_str(&render_json(&reports)).unwrap();
        let snapshot = json["snapshots"][0].as_object().unwrap();
        assert!(!snapshot.contains_key("limit"), "got: {snapshot:?}");
        assert!(!snapshot.contains_key("pass"), "got: {snapshot:?}");
    }

    #[test]
    fn output_is_pretty_printed_multi_line_json() {
        let json_str = render_json(&[report("pkg", "fn")]);
        assert!(
            json_str.contains('\n'),
            "expected a pretty-printed document"
        );
        assert!(json_str.starts_with("{\n"), "got: {json_str}");
    }

    #[test]
    fn validate_report_schema_accepts_version_1() {
        let val = serde_json::json!({
            "schema_version": 1,
            "snapshots": [
                {"package": "pkg", "function": "fn", "metric": "CPU Instructions", "value": 100}
            ]
        });
        let res = validate_report_schema(&val, Path::new("report.json")).unwrap();
        assert!(res.is_some());
        assert_eq!(res.unwrap().len(), 1);
    }

    #[test]
    fn validate_report_schema_accepts_bare_array() {
        let val = serde_json::json!([
            {"package": "pkg", "function": "fn", "metric": "CPU Instructions", "value": 100}
        ]);
        let res = validate_report_schema(&val, Path::new("report.json")).unwrap();
        assert!(res.is_none());
    }

    #[test]
    fn validate_report_schema_rejects_unsupported_version() {
        let val = serde_json::json!({
            "schema_version": 2,
            "snapshots": []
        });
        let err = validate_report_schema(&val, Path::new("test_report.json")).unwrap_err();
        assert_eq!(err.exit_code(), crate::error::EXIT_CONFIG_ERROR);
        let msg = err.to_string();
        assert!(
            msg.contains("unsupported schema_version 2 in test_report.json"),
            "got: {msg}"
        );
        assert!(msg.contains("accepted versions are [1]"), "got: {msg}");
    }

    #[test]
    fn validate_report_schema_rejects_version_zero() {
        let val = serde_json::json!({
            "schema_version": 0,
            "snapshots": []
        });
        let err = validate_report_schema(&val, Path::new("test_report.json")).unwrap_err();
        assert_eq!(err.exit_code(), crate::error::EXIT_CONFIG_ERROR);
        let msg = err.to_string();
        assert!(
            msg.contains("unsupported schema_version 0 in test_report.json"),
            "got: {msg}"
        );
        assert!(msg.contains("accepted versions are [1]"), "got: {msg}");
    }

    #[test]
    fn validate_report_schema_rejects_missing_version() {
        let val = serde_json::json!({
            "snapshots": []
        });
        let err = validate_report_schema(&val, Path::new("test_report.json")).unwrap_err();
        assert_eq!(err.exit_code(), crate::error::EXIT_CONFIG_ERROR);
        let msg = err.to_string();
        assert!(
            msg.contains("missing schema_version in wrapped report test_report.json"),
            "got: {msg}"
        );
        assert!(msg.contains("accepted versions are [1]"), "got: {msg}");
    }

    #[test]
    fn validate_report_schema_rejects_null_version() {
        let val = serde_json::json!({
            "schema_version": null,
            "snapshots": []
        });
        let err = validate_report_schema(&val, Path::new("test_report.json")).unwrap_err();
        assert_eq!(err.exit_code(), crate::error::EXIT_CONFIG_ERROR);
        let msg = err.to_string();
        assert!(
            msg.contains("invalid schema_version: null in test_report.json"),
            "got: {msg}"
        );
        assert!(msg.contains("accepted versions are [1]"), "got: {msg}");
    }

    #[test]
    fn validate_report_schema_rejects_negative_and_non_integer_version() {
        let neg = serde_json::json!({
            "schema_version": -1,
            "snapshots": []
        });
        let err = validate_report_schema(&neg, Path::new("test_report.json")).unwrap_err();
        assert_eq!(err.exit_code(), crate::error::EXIT_CONFIG_ERROR);
        let msg = err.to_string();
        assert!(
            msg.contains("invalid schema_version -1 in test_report.json"),
            "got: {msg}"
        );
        assert!(msg.contains("accepted versions are [1]"), "got: {msg}");

        let str_val = serde_json::json!({
            "schema_version": "1",
            "snapshots": []
        });
        let err2 = validate_report_schema(&str_val, Path::new("test_report.json")).unwrap_err();
        assert_eq!(err2.exit_code(), crate::error::EXIT_CONFIG_ERROR);
        let msg2 = err2.to_string();
        assert!(
            msg2.contains("invalid schema_version \"1\" in test_report.json"),
            "got: {msg2}"
        );
    }

    #[test]
    fn validate_report_schema_rejects_unsupported_version_before_inspecting_snapshots() {
        // Even if `snapshots` is not an array (or is completely invalid),
        // the unsupported version error must be returned first.
        let val = serde_json::json!({
            "schema_version": 99,
            "snapshots": "not-an-array"
        });
        let err = validate_report_schema(&val, Path::new("broken.json")).unwrap_err();
        assert_eq!(err.exit_code(), crate::error::EXIT_CONFIG_ERROR);
        let msg = err.to_string();
        assert!(
            msg.contains("unsupported schema_version 99 in broken.json"),
            "got: {msg}"
        );
        assert!(msg.contains("accepted versions are [1]"), "got: {msg}");
    }
}
