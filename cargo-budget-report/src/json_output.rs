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

use serde::Serialize;

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
#[derive(Serialize)]
pub(crate) struct BudgetReportJson<'a> {
    /// Schema version of the document being emitted; always
    /// [`SCHEMA_VERSION`] for as long as this type is the only writer.
    schema_version: u32,
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
    fn envelope_carries_only_the_schema_version_and_snapshots() {
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
        assert_eq!(keys, vec!["schema_version", "snapshots"]);
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
        assert!(json_str.contains('\n'), "expected a pretty-printed document");
        assert!(json_str.starts_with("{\n"), "got: {json_str}");
    }
}
