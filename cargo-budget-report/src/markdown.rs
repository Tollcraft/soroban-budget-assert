//! Markdown output formatting for budget reports.

/// A single contract or test budget result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownReportRow {
    /// Contract or test name displayed in the first column.
    pub name: String,
    /// CPU budget cost displayed in the second column.
    pub cpu_cost: u64,
    /// Memory budget cost displayed in the third column.
    pub memory_cost: u64,
    /// Whether the budget assertion passed.
    pub passed: bool,
}

impl MarkdownReportRow {
    /// Creates a report row.
    pub fn new(name: impl Into<String>, cpu_cost: u64, memory_cost: u64, passed: bool) -> Self {
        Self {
            name: name.into(),
            cpu_cost,
            memory_cost,
            passed,
        }
    }
}

/// Formats budget report rows as a Markdown table.
///
/// The returned string does not include a trailing newline, allowing callers
/// to decide how the table should be separated from other CLI output.
pub fn format_markdown_table(rows: &[MarkdownReportRow]) -> String {
    let mut table = String::from(
        "| Contract/Test Name | CPU Cost | Memory Cost | Status |\n| --- | ---: | ---: | --- |",
    );

    for row in rows {
        let status = if row.passed { "Pass" } else { "Fail" };
        table.push('\n');
        table.push_str("| ");
        table.push_str(&escape_markdown_cell(&row.name));
        table.push_str(" | ");
        table.push_str(&row.cpu_cost.to_string());
        table.push_str(" | ");
        table.push_str(&row.memory_cost.to_string());
        table.push_str(" | ");
        table.push_str(status);
        table.push_str(" |");
    }

    table
}

fn escape_markdown_cell(value: &str) -> String {
    value
        .replace('|', "\\|")
        .replace('\n', " ")
        .replace('\r', " ")
}

#[cfg(test)]
mod tests {
    use super::{format_markdown_table, MarkdownReportRow};

    #[test]
    fn formats_passing_and_failing_rows() {
        let rows = vec![
            MarkdownReportRow::new("token_contract", 1_234, 567, true),
            MarkdownReportRow::new("swap_test", 9_876, 1_024, false),
        ];

        assert_eq!(
            format_markdown_table(&rows),
            "| Contract/Test Name | CPU Cost | Memory Cost | Status |\n\
             | --- | ---: | ---: | --- |\n\
             | token_contract | 1234 | 567 | Pass |\n\
             | swap_test | 9876 | 1024 | Fail |"
        );
    }

    #[test]
    fn formats_empty_reports_with_headers_only() {
        assert_eq!(
            format_markdown_table(&[]),
            "| Contract/Test Name | CPU Cost | Memory Cost | Status |\n\
             | --- | ---: | ---: | --- |"
        );
    }

    #[test]
    fn escapes_markdown_cells() {
        let rows = [MarkdownReportRow::new(
            "contract|with\nseparator",
            10,
            20,
            true,
        )];

        assert_eq!(
            format_markdown_table(&rows),
            "| Contract/Test Name | CPU Cost | Memory Cost | Status |\n\
             | --- | ---: | ---: | --- |\n\
             | contract\\|with separator | 10 | 20 | Pass |"
        );
    }
}
