//! SARIF 2.1.0 output for `cargo budget-report --check`.
//!
//! When the `--sarif` flag is set, this module produces a valid SARIF document
//! containing one result per budget breach. A run with zero breaches still
//! emits a valid SARIF document with an empty `results` array — GitHub treats
//! a missing file as an upload failure.
//!
//! Each result carries:
//! - A stable `ruleId` per metric (`cpu-limit-exceeded`, `read-limit-exceeded`,
//!   `write-limit-exceeded`) so GitHub can track a finding across runs.
//! - The package, function, metric, measured value, and configured limit in
//!   a human-readable `message.text`.
//! - A `region` with the source location of the function definition when it
//!   can be resolved via `cargo_metadata`; otherwise the result is emitted
//!   without a location.
//!
//! The exit code is unchanged — SARIF is an additional output, not a
//! replacement for the failure signal.

use crate::CostReport;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// SARIF 2.1.0 top-level document.
#[derive(Serialize)]
pub(crate) struct Sarif {
    #[serde(rename = "$schema")]
    pub schema: &'static str,
    pub version: &'static str,
    pub runs: Vec<SarifRun>,
}

/// A single run within a SARIF document.
#[derive(Serialize)]
pub(crate) struct SarifRun {
    pub tool: SarifTool,
    pub results: Vec<SarifResult>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<SarifRule>,
}

/// The tool that produced the results.
#[derive(Serialize)]
pub(crate) struct SarifTool {
    pub driver: SarifDriver,
}

/// Tool driver metadata.
#[derive(Serialize)]
pub(crate) struct SarifDriver {
    pub name: &'static str,
    pub version: &'static str,
    pub semantic_version: &'static str,
    pub rules: Vec<SarifRule>,
}

/// A rule definition referenced by results.
#[derive(Serialize, Clone)]
pub(crate) struct SarifRule {
    pub id: &'static str,
    pub name: &'static str,
    pub short_description: SarifMessage,
    pub full_description: SarifMessage,
    pub help_uri: &'static str,
}

/// A SARIF message object.
#[derive(Serialize, Clone)]
pub(crate) struct SarifMessage {
    pub text: String,
}

/// A single result (finding) in a SARIF run.
#[derive(Serialize)]
pub(crate) struct SarifResult {
    pub rule_id: &'static str,
    pub level: &'static str,
    pub message: SarifMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locations: Option<Vec<SarifLocation>>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, serde_json::Value>,
}

/// A physical location within a SARIF result.
#[derive(Serialize)]
pub(crate) struct SarifLocation {
    pub physical_location: SarifPhysicalLocation,
}

/// Physical location details (file + region).
#[derive(Serialize)]
pub(crate) struct SarifPhysicalLocation {
    pub artifact_location: SarifArtifactLocation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<SarifRegion>,
}

/// Artifact (file) location.
#[derive(Serialize)]
pub(crate) struct SarifArtifactLocation {
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri_base_id: Option<String>,
}

/// Region within a file (line-based).
#[derive(Serialize)]
pub(crate) struct SarifRegion {
    pub start_line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_column: Option<u32>,
}

/// Rule IDs for each metric type.
pub(crate) const RULE_CPU: &str = "cpu-limit-exceeded";
pub(crate) const RULE_READ: &str = "read-limit-exceeded";
pub(crate) const RULE_WRITE: &str = "write-limit-exceeded";

/// The SARIF version and schema URL.
const SARIF_VERSION: &str = "2.1.0";
const SARIF_SCHEMA: &str = "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json";

/// Tool name and version.
const TOOL_NAME: &str = "cargo-budget-report";
const TOOL_VERSION: &str = "0.1.0";

/// Helper to map a metric label to its rule ID.
fn metric_to_rule_id(metric: &str) -> &'static str {
    match metric {
        "CPU Instructions" => RULE_CPU,
        "Read Bytes" => RULE_READ,
        "Write Bytes" => RULE_WRITE,
        _ => "unknown-metric",
    }
}

/// Format a value with commas and a unit suffix, matching the main report's
/// `format_with_commas_and_units` function.
fn format_value(value: u64, metric: &str) -> String {
    let value_str = value.to_string();
    let mut result = String::new();
    let mut digit_count = 0;
    for ch in value_str.chars().rev() {
        if digit_count == 3 {
            result.push(',');
            digit_count = 0;
        }
        result.push(ch);
        digit_count += 1;
    }
    let formatted = result.chars().rev().collect::<String>();

    if metric.contains("Bytes") {
        format!("{} B", formatted)
    } else {
        format!("{} inst.", formatted)
    }
}

/// Format a limit value with commas and a unit suffix.
fn format_limit(value: u64, metric: &str) -> String {
    format_value(value, metric)
}

/// Try to find the source file for a contract function using `cargo_metadata`.
///
/// This searches the workspace packages for a Rust source file containing
/// the function definition. When it cannot find the file (e.g. the function
/// is generated by a macro), it returns `None` — the result is emitted
/// without a location rather than inventing one.
fn find_source_location(
    package_name: &str,
    function_name: &str,
    metadata: &cargo_metadata::Metadata,
) -> Option<PathBuf> {
    // Find the package in the workspace metadata.
    let pkg = metadata.packages.iter().find(|p| p.name == package_name)?;

    // Look in the package's source directory for a .rs file containing
    // the function name as a public function definition.
    let pkg_root = pkg.manifest_path.parent()?;
    let src_dir = pkg_root.join("src");
    if !src_dir.exists() {
        return None;
    }

    // Search for the function in .rs files.
    search_for_function(src_dir.as_std_path(), function_name)
        .ok()
        .flatten()
}

/// Recursively search a directory for a .rs file containing the function.
fn search_for_function(dir: &Path, function_name: &str) -> std::io::Result<Option<PathBuf>> {
    if !dir.is_dir() {
        return Ok(None);
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = search_for_function(&path, function_name)? {
                return Ok(Some(found));
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            if let Ok(contents) = std::fs::read_to_string(&path) {
                // Look for a function definition pattern:
                // `pub fn function_name`, `fn function_name`, or
                // `pub(crate) fn function_name`.
                let pattern = format!("fn {function_name}");
                if contents.contains(&pattern) {
                    return Ok(Some(path));
                }
            }
        }
    }
    Ok(None)
}

/// Find the line number of a function in a source file.
fn find_line_number(file_path: &Path, function_name: &str) -> Option<u32> {
    let contents = std::fs::read_to_string(file_path).ok()?;
    let pattern = format!("fn {function_name}");
    for (idx, line) in contents.lines().enumerate() {
        if line.contains(&pattern) {
            return Some(idx as u32 + 1); // 1-indexed
        }
    }
    None
}

/// Build the SARIF rules that define the budget check rules.
fn build_rules() -> Vec<SarifRule> {
    vec![
        SarifRule {
            id: RULE_CPU,
            name: "cpu-limit-exceeded",
            short_description: SarifMessage {
                text: "CPU instruction budget exceeded".to_string(),
            },
            full_description: SarifMessage {
                text: "The measured CPU instructions exceeded the configured limit for this function.".to_string(),
            },
            help_uri: "https://github.com/Tollcraft/soroban-budget-assert/blob/main/docs/src/user_guide.md",
        },
        SarifRule {
            id: RULE_READ,
            name: "read-limit-exceeded",
            short_description: SarifMessage {
                text: "Read bytes budget exceeded".to_string(),
            },
            full_description: SarifMessage {
                text: "The measured read bytes exceeded the configured limit for this function.".to_string(),
            },
            help_uri: "https://github.com/Tollcraft/soroban-budget-assert/blob/main/docs/src/user_guide.md",
        },
        SarifRule {
            id: RULE_WRITE,
            name: "write-limit-exceeded",
            short_description: SarifMessage {
                text: "Write bytes budget exceeded".to_string(),
            },
            full_description: SarifMessage {
                text: "The measured write bytes exceeded the configured limit for this function.".to_string(),
            },
            help_uri: "https://github.com/Tollcraft/soroban-budget-assert/blob/main/docs/src/user_guide.md",
        },
    ]
}

/// Build SARIF results from the budget check reports.
///
/// Only entries with `pass == Some(false)` (budget breaches) are included.
/// A valid SARIF document with an empty results array is always returned
/// when there are no breaches.
pub(crate) fn build_sarif(
    reports: &[CostReport],
    metadata: Option<&cargo_metadata::Metadata>,
) -> Sarif {
    let rules = build_rules();

    let results: Vec<SarifResult> = reports
        .iter()
        .filter(|r| r.pass == Some(false) && r.value.is_some())
        .filter_map(|r| {
            let rule_id = metric_to_rule_id(r.metric);
            let measured = r.value?;
            let limit = r.limit?;
            let pct_over = ((measured as f64 - limit as f64) / limit as f64 * 100.0).round();

            let message_text = format!(
                "{}::{} [{}]: {} (measured) exceeds {} (limit) by {:.0}%",
                r.package,
                r.function,
                r.metric,
                format_value(measured as u64, r.metric),
                format_limit(limit, r.metric),
                pct_over,
            );

            // Attempt to resolve the source location.
            let location = metadata.and_then(|meta| {
                let file_path = find_source_location(&r.package, &r.function, meta)?;
                let line = find_line_number(&file_path, &r.function);
                let uri = file_path.to_string_lossy().to_string();
                Some(SarifLocation {
                    physical_location: SarifPhysicalLocation {
                        artifact_location: SarifArtifactLocation {
                            uri,
                            uri_base_id: None,
                        },
                        region: line.map(|l| SarifRegion {
                            start_line: l,
                            start_column: None,
                        }),
                    },
                })
            });

            let mut properties = BTreeMap::new();
            properties.insert(
                "package".to_string(),
                serde_json::Value::String(r.package.clone()),
            );
            properties.insert(
                "function".to_string(),
                serde_json::Value::String(r.function.clone()),
            );
            properties.insert(
                "metric".to_string(),
                serde_json::Value::String(r.metric.to_string()),
            );
            properties.insert("measured".to_string(), serde_json::json!(measured));
            properties.insert("limit".to_string(), serde_json::json!(limit));
            properties.insert("percent_over".to_string(), serde_json::json!(pct_over));

            Some(SarifResult {
                rule_id,
                level: "error",
                message: SarifMessage { text: message_text },
                locations: location.map(|l| vec![l]),
                properties,
            })
        })
        .collect();

    let tool = SarifTool {
        driver: SarifDriver {
            name: TOOL_NAME,
            version: TOOL_VERSION,
            semantic_version: TOOL_VERSION,
            rules: rules.clone(),
        },
    };

    Sarif {
        schema: SARIF_SCHEMA,
        version: SARIF_VERSION,
        runs: vec![SarifRun {
            tool,
            results,
            rules,
        }],
    }
}

/// Serialize a SARIF document to a pretty-printed JSON string.
pub(crate) fn to_json(sarif: &Sarif) -> String {
    serde_json::to_string_pretty(sarif).expect("SARIF serialization is infallible")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CostReport;

    fn breach_report(metric: &'static str, value: u32, limit: u64) -> CostReport {
        CostReport {
            package: "test-contract".to_string(),
            function: "do_work".to_string(),
            metric,
            value: Some(value),
            limit: Some(limit),
            pass: Some(false),
        }
    }

    fn pass_report(metric: &'static str, value: u32, limit: u64) -> CostReport {
        CostReport {
            package: "test-contract".to_string(),
            function: "do_work".to_string(),
            metric,
            value: Some(value),
            limit: Some(limit),
            pass: Some(true),
        }
    }

    fn no_limit_report(metric: &'static str, value: u32) -> CostReport {
        CostReport {
            package: "test-contract".to_string(),
            function: "do_work".to_string(),
            metric,
            value: Some(value),
            limit: None,
            pass: None,
        }
    }

    #[test]
    fn sarif_empty_when_no_breaches() {
        let reports = vec![
            pass_report("CPU Instructions", 1_000_000, 5_000_000),
            pass_report("Read Bytes", 2_048, 5_000),
        ];
        let sarif = build_sarif(&reports, None);
        assert_eq!(sarif.version, "2.1.0");
        assert_eq!(sarif.runs.len(), 1);
        assert!(sarif.runs[0].results.is_empty());
    }

    #[test]
    fn sarif_empty_when_no_reports() {
        let sarif = build_sarif(&[], None);
        assert_eq!(sarif.runs[0].results.len(), 0);
    }

    #[test]
    fn sarif_includes_only_breaches() {
        let reports = vec![
            pass_report("CPU Instructions", 1_000_000, 5_000_000),
            breach_report("Read Bytes", 10_000, 5_000),
            no_limit_report("Write Bytes", 4_096),
        ];
        let sarif = build_sarif(&reports, None);
        assert_eq!(sarif.runs[0].results.len(), 1);
        let result = &sarif.runs[0].results[0];
        assert_eq!(result.rule_id, RULE_READ);
        assert_eq!(result.level, "error");
        assert!(result.message.text.contains("Read Bytes"));
        assert!(result.message.text.contains("10,000"));
        assert!(result.message.text.contains("5,000"));
    }

    #[test]
    fn sarif_result_has_properties() {
        let reports = vec![breach_report("CPU Instructions", 6_000_000, 5_000_000)];
        let sarif = build_sarif(&reports, None);
        let result = &sarif.runs[0].results[0];
        assert_eq!(
            result.properties.get("package").unwrap(),
            &serde_json::json!("test-contract")
        );
        assert_eq!(
            result.properties.get("function").unwrap(),
            &serde_json::json!("do_work")
        );
        assert_eq!(
            result.properties.get("metric").unwrap(),
            &serde_json::json!("CPU Instructions")
        );
        assert_eq!(
            result.properties.get("measured").unwrap(),
            &serde_json::json!(6_000_000)
        );
        assert_eq!(
            result.properties.get("limit").unwrap(),
            &serde_json::json!(5_000_000)
        );
    }

    #[test]
    fn sarif_rules_present_in_driver() {
        let sarif = build_sarif(&[], None);
        let driver = &sarif.runs[0].tool.driver;
        assert_eq!(driver.rules.len(), 3);
        let ids: Vec<&str> = driver.rules.iter().map(|r| r.id).collect();
        assert!(ids.contains(&RULE_CPU));
        assert!(ids.contains(&RULE_READ));
        assert!(ids.contains(&RULE_WRITE));
    }

    #[test]
    fn sarif_multiple_breaches() {
        let reports = vec![
            breach_report("CPU Instructions", 10_000_000, 5_000_000),
            breach_report("Read Bytes", 10_000, 5_000),
            breach_report("Write Bytes", 2_000, 1_000),
        ];
        let sarif = build_sarif(&reports, None);
        assert_eq!(sarif.runs[0].results.len(), 3);
    }

    #[test]
    fn sarif_valid_json() {
        let reports = vec![breach_report("CPU Instructions", 6_000_000, 5_000_000)];
        let sarif = build_sarif(&reports, None);
        let json = to_json(&sarif);
        // Verify it's valid JSON.
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["$schema"], SARIF_SCHEMA);
        assert_eq!(parsed["version"], "2.1.0");
    }

    #[test]
    fn sarif_percent_over_calculated_correctly() {
        let reports = vec![breach_report("CPU Instructions", 1_100_000, 1_000_000)];
        let sarif = build_sarif(&reports, None);
        let result = &sarif.runs[0].results[0];
        assert_eq!(
            result.properties.get("percent_over").unwrap(),
            &serde_json::json!(10.0)
        );
    }

    #[test]
    fn sarif_no_location_when_no_metadata() {
        let reports = vec![breach_report("CPU Instructions", 6_000_000, 5_000_000)];
        let sarif = build_sarif(&reports, None);
        let result = &sarif.runs[0].results[0];
        assert!(result.locations.is_none());
    }

    #[test]
    fn sarif_simulation_failure_excluded() {
        // A report with value: None and pass: Some(false) should be excluded
        // since it has no measured value to compare.
        let reports = vec![CostReport {
            package: "test-contract".to_string(),
            function: "broken".to_string(),
            metric: "CPU Instructions",
            value: None,
            limit: Some(5_000_000),
            pass: Some(false),
        }];
        let sarif = build_sarif(&reports, None);
        assert!(sarif.runs[0].results.is_empty());
    }
}
