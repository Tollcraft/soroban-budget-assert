use crate::CostReport;
use std::fmt::Write as _;

const HEADER: &str = "# Workspace Budget Report (Tier A Local Measurements)\n\n\
| Package | Function | Metric | Value |\n\
|---|---|---|---:|\n";

const FOOTER: &str = "\n---\n\
_Simulated resource amounts from local WASM test harness. Network-dependent billing metrics (Read/Write Bytes) require testnet simulation._\n";

/// Cell text for a metric that has no measured value.
const NOT_MEASURED: &str = "N/A (testnet required)";

/// Typical bytes a row adds beyond its package and function names: the four
/// column separators, a metric name (at most `CPU Instructions`), a value of up
/// to 13 characters, and the newline. Only sizes the up-front allocation, so an
/// underestimate merely costs one regrowth.
const ROW_OVERHEAD: usize = 48;

/// Renders a slice of [`CostReport`] entries into a GitHub-Flavored Markdown table
/// suitable for appending to `$GITHUB_STEP_SUMMARY`.
///
/// The output is built in a single buffer sized up front: rows are appended with
/// `push_str` and values are formatted in place, so rendering allocates once
/// (plus a regrowth if the estimate is short) instead of once or twice per row.
pub(crate) fn render_markdown(reports: &[CostReport]) -> String {
    let rows_len: usize = reports
        .iter()
        .map(|r| r.package.len() + r.function.len() + ROW_OVERHEAD)
        .sum();
    let mut out = String::with_capacity(HEADER.len() + rows_len + FOOTER.len());
    out.push_str(HEADER);

    for r in reports {
        out.push_str("| ");
        out.push_str(&r.package);
        out.push_str(" | ");
        out.push_str(&r.function);
        out.push_str(" | ");
        out.push_str(r.metric);
        out.push_str(" | ");
        match r.value {
            Some(v) => push_with_commas(&mut out, v),
            None => out.push_str(NOT_MEASURED),
        }
        out.push_str(" |\n");
    }

    out.push_str(FOOTER);
    out
}

/// Appends `n` in decimal with a comma between each group of three digits
/// (`2654615` becomes `2,654,615`), without allocating.
///
/// The number is split into base-1000 groups, least significant first. A `u32`
/// has at most ten digits, so four groups always suffice. The leading group is
/// written as-is; every later group is zero-padded to three digits so that
/// `1_000_001` renders as `1,000,001` and not `1,0,1`.
fn push_with_commas(out: &mut String, n: u32) {
    let mut groups = [0u16; 4];
    let mut count = 0;
    let mut rest = n;
    loop {
        groups[count] = (rest % 1000) as u16;
        count += 1;
        rest /= 1000;
        if rest == 0 {
            break;
        }
    }

    // Writing into a `String` cannot fail.
    let _ = write!(out, "{}", groups[count - 1]);
    for group in groups[..count - 1].iter().rev() {
        let _ = write!(out, ",{group:03}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_markdown_table() {
        let reports = vec![
            CostReport {
                package: "amm-pool-contract".to_string(),
                function: "do_expensive_work".to_string(),
                metric: "CPU Instructions",
                value: Some(2654615),
                limit: None,
                pass: None,
            },
            CostReport {
                package: "amm-pool-contract".to_string(),
                function: "do_expensive_work".to_string(),
                metric: "Read Bytes",
                value: None,
                limit: None,
                pass: None,
            },
        ];

        let md = render_markdown(&reports);
        assert!(md.contains("# Workspace Budget Report"));
        assert!(
            md.contains("| amm-pool-contract | do_expensive_work | CPU Instructions | 2,654,615 |")
        );
        assert!(md.contains(
            "| amm-pool-contract | do_expensive_work | Read Bytes | N/A (testnet required) |"
        ));
    }

    fn report(
        package: &str,
        function: &str,
        metric: &'static str,
        value: Option<u32>,
    ) -> CostReport {
        CostReport {
            package: package.to_string(),
            function: function.to_string(),
            metric,
            value,
            limit: None,
            pass: None,
        }
    }

    /// The pre-optimization implementation, kept verbatim as the oracle the
    /// allocation-free version must match byte for byte.
    fn reference_format_number(n: u32) -> String {
        let s = n.to_string();
        let mut result = String::new();
        let len = s.len();
        for (i, c) in s.chars().enumerate() {
            if i > 0 && (len - i).is_multiple_of(3) {
                result.push(',');
            }
            result.push(c);
        }
        result
    }

    fn commas(n: u32) -> String {
        let mut out = String::new();
        push_with_commas(&mut out, n);
        out
    }

    #[test]
    fn push_with_commas_group_boundaries() {
        for (n, expected) in [
            (0, "0"),
            (9, "9"),
            (999, "999"),
            (1_000, "1,000"),
            (1_001, "1,001"),
            (10_000, "10,000"),
            (999_999, "999,999"),
            (1_000_000, "1,000,000"),
            // Interior groups that are all or partly zero must keep their padding.
            (1_000_001, "1,000,001"),
            (1_010_010, "1,010,010"),
            (999_999_999, "999,999,999"),
            (1_000_000_000, "1,000,000,000"),
            (u32::MAX, "4,294,967,295"),
        ] {
            assert_eq!(commas(n), expected, "n = {n}");
        }
    }

    #[test]
    fn push_with_commas_matches_reference_implementation() {
        // Every value within a few of each power-of-1000 boundary, plus a
        // stride through the whole u32 range.
        let mut values: Vec<u32> = (0..5_000).collect();
        for boundary in [1_000u32, 1_000_000, 1_000_000_000] {
            values.extend((boundary - 5)..(boundary + 5));
        }
        values.extend((0..u32::MAX).step_by(7_919_003));
        values.push(u32::MAX);
        for n in values {
            assert_eq!(commas(n), reference_format_number(n), "n = {n}");
        }
    }

    #[test]
    fn push_with_commas_appends_without_disturbing_existing_text() {
        let mut out = String::from("value: ");
        push_with_commas(&mut out, 12_345);
        assert_eq!(out, "value: 12,345");
    }

    #[test]
    fn render_markdown_empty_input_is_header_and_footer_only() {
        assert_eq!(render_markdown(&[]), format!("{HEADER}{FOOTER}"));
    }

    #[test]
    fn render_markdown_full_output_is_exact() {
        let md = render_markdown(&[
            report("pkg", "fn_a", "CPU Instructions", Some(1_234_567)),
            report("pkg", "fn_a", "Write Bytes", None),
            report("", "", "Read Bytes", Some(0)),
        ]);
        let expected = "# Workspace Budget Report (Tier A Local Measurements)\n\n\
| Package | Function | Metric | Value |\n\
|---|---|---|---:|\n\
| pkg | fn_a | CPU Instructions | 1,234,567 |\n\
| pkg | fn_a | Write Bytes | N/A (testnet required) |\n\
|  |  | Read Bytes | 0 |\n\
\n---\n\
_Simulated resource amounts from local WASM test harness. Network-dependent billing metrics (Read/Write Bytes) require testnet simulation._\n";
        assert_eq!(md, expected);
    }

    #[test]
    fn render_markdown_preserves_report_order_and_emits_one_row_each() {
        let reports: Vec<CostReport> = (0..50)
            .map(|i| report("p", &format!("f{i}"), "CPU Instructions", Some(i * 1_000)))
            .collect();
        let md = render_markdown(&reports);
        let rows: Vec<&str> = md.lines().filter(|l| l.starts_with("| p |")).collect();
        assert_eq!(rows.len(), 50);
        assert!(rows[0].contains("| f0 |") && rows[49].contains("| f49 |"));
    }

    #[test]
    fn render_markdown_does_not_escape_or_alter_cell_text() {
        // Behaviour preserved from the original: names are emitted verbatim.
        let md = render_markdown(&[report("a|b", "c d", "CPU Instructions", Some(1))]);
        assert!(md.contains("| a|b | c d | CPU Instructions | 1 |\n"));
    }
}
