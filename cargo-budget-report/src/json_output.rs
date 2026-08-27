use serde::Serialize;

/// Current JSON schema version for budget report output.
///
/// Increment this value when the JSON structure changes in a way that
/// requires consumers to update their parsing logic (see
/// `docs/src/reference.md` for the full versioning policy).
pub(crate) const SCHEMA_VERSION: u32 = 1;

/// Top-level wrapper emitted by `cargo budget-report --json`.
///
/// Every JSON document produced by the budget report is an object with a
/// `schema_version` integer and a `snapshots` array.  The individual
/// snapshot objects are unchanged from the pre-versioning format.
#[derive(Serialize)]
pub(crate) struct BudgetReportJson<'a> {
    schema_version: u32,
    snapshots: &'a [crate::CostReport],
}

/// Wrap the given report rows in the versioned JSON envelope.
pub(crate) fn render_json(reports: &[crate::CostReport]) -> String {
    let wrapper = BudgetReportJson {
        schema_version: SCHEMA_VERSION,
        snapshots: reports,
    };
    serde_json::to_string_pretty(&wrapper).expect("report serialization should not fail")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CostReport;

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
}
