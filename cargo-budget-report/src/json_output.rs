use serde::Serialize;
use serde_json::Value;
use std::fmt;
use std::str::FromStr;

/// Supported output formats for budget reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Json,
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self::Human
    }
}

impl FromStr for OutputFormat {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "human" => Ok(Self::Human),
            "json" => Ok(Self::Json),
            _ => Err(format!(
                "unsupported output format `{value}`; expected `human` or `json`"
            )),
        }
    }
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Human => formatter.write_str("human"),
            Self::Json => formatter.write_str("json"),
        }
    }
}

/// The stable top-level JSON representation emitted by `--format json`.
///
/// The snapshot is intentionally kept as a serialized value here. This lets
/// the report command serialize its existing snapshot data structures without
/// changing their ownership or presentation logic, while guaranteeing a
/// stable top-level schema for consumers.
#[derive(Debug, Serialize)]
struct JsonReport<'a> {
    schema_version: u8,
    snapshots: &'a Value,
}

/// Serialize report snapshot data using the public JSON report schema.
///
/// The returned string contains only JSON. No status messages, labels, or
/// progress output are added, which makes it safe to pipe directly into
/// another process.
pub fn render_json<T: Serialize>(snapshots: &T) -> Result<String, serde_json::Error> {
    let snapshots = serde_json::to_value(snapshots)?;
    serde_json::to_string_pretty(&JsonReport {
        schema_version: 1,
        snapshots: &snapshots,
    })
}

/// Render a report according to the selected output format.
pub fn render<T, F>(
    format: OutputFormat,
    snapshots: &T,
    human_renderer: F,
) -> Result<String, serde_json::Error>
where
    T: Serialize,
    F: FnOnce(&T) -> String,
{
    match format {
        OutputFormat::Human => Ok(human_renderer(snapshots)),
        OutputFormat::Json => render_json(snapshots),
    }
}

#[cfg(test)]
mod tests {
    use super::{render_json, OutputFormat};
    use serde_json::json;
    use std::str::FromStr;

    #[test]
    fn json_output_has_stable_schema() {
        let snapshot = json!({
            "contract": "example",
            "cpu": 123,
            "memory": 456
        });

        let output = render_json(&snapshot).expect("snapshot should serialize");
        let parsed: serde_json::Value =
            serde_json::from_str(&output).expect("output must be valid JSON");

        assert_eq!(parsed["schema_version"], 1);
        assert_eq!(parsed["snapshots"]["contract"], "example");
        assert_eq!(parsed["snapshots"]["cpu"], 123);
        assert_eq!(parsed["snapshots"]["memory"], 456);
    }

    #[test]
    fn format_parser_accepts_json_only_as_json() {
        assert_eq!(OutputFormat::from_str("json"), Ok(OutputFormat::Json));
        assert_eq!(OutputFormat::from_str("human"), Ok(OutputFormat::Human));
        assert!(OutputFormat::from_str("yaml").is_err());
    }
}
