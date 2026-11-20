use crate::CostReport;
use std::fmt::Write as _;

const HEADER: &str = "# Workspace Budget Report (Tier A Local Measurements)\n\n\
| Package | Function | Metric | Value |\n\
|---|---|---|---:|\n";

/// Header used when the reports carry `--check` metadata (`limit`/`pass`).
const CHECK_HEADER: &str = "# Workspace Budget Report (Tier A Local Measurements)\n\n\
| Package | Function | Metric | Value | Limit | Status |\n\
|---|---|---|---:|---:|---|\n";

const FOOTER: &str = "\n---\n\
_Simulated resource amounts from local WASM test harness. Network-dependent billing metrics (Read/Write Bytes) require testnet simulation._\n";

/// Cell text for a metric that has no measured value in a report without check
/// metadata: the metric was not produced, typically because it can only be
/// measured against a live network.
const NOT_MEASURED: &str = "N/A (testnet required)";

/// Cell text for a failed-check stub row: the simulation produced no value, so
/// there is nothing to compare against the limit. Rendered distinctly from
/// [`NOT_MEASURED`] so a limit breach is never presented as a missing testnet
/// measurement.
const NOT_MEASURED_CHECK: &str = "N/A (simulation failed)";

/// Cell text for a check column that does not apply to this row (no limit
/// configured, or no pass/fail recorded).
const NOT_APPLICABLE: &str = "-";

/// Escape text for use inside a Markdown table cell.
///
/// * `|` → `\|` — an unescaped pipe would be interpreted as a column
///   delimiter, shifting all subsequent cells and corrupting alignment.
/// * Embedded newlines (`\n`, `\r\n`, `\r`) are collapsed into a single space
///   so that the table row stays on one line.
fn escape_cell(text: &str) -> std::borrow::Cow<'_, str> {
    if text.contains('|') || text.contains('\n') || text.contains('\r') {
        std::borrow::Cow::Owned(
            text.replace('\r', "")
                .replace('\n', " ")
                .replace('|', r"\|"),
        )
    } else {
        std::borrow::Cow::Borrowed(text)
    }
}

/// Typical bytes a row adds beyond its package and function names: the four
/// column separators, a metric name (at most `CPU Instructions`), a value of up
/// to 13 characters, and the newline. Only sizes the up-front allocation, so an
/// underestimate merely costs one regrowth.
const ROW_OVERHEAD: usize = 48;

/// Additional bytes a check row adds over [`ROW_OVERHEAD`]: two column
/// separators, a `u64` limit (at most 20 digits plus separators), and a
/// `PASS`/`FAIL` status cell.
const CHECK_ROW_OVERHEAD: usize = 40;

/// Renders a slice of [`CostReport`] entries into a GitHub-Flavored Markdown table
/// suitable for appending to `$GITHUB_STEP_SUMMARY`.
///
/// The table has four columns — `Package`, `Function`, `Metric`, `Value` — and
/// gains two more (`Limit`, `Status`) whenever the reports carry check
/// metadata, i.e. any entry has a `limit` or `pass` recorded by `--check`.
/// Detecting this from the data (rather than requiring a caller flag) is what
/// lets the `--markdown --from current_report.json` path used for the CI step
/// summary show breach status even when the rendering command does not itself
/// pass `--check`.
///
/// A row whose `value` is `None` renders as `N/A (testnet required)` without
/// check metadata. With check metadata the same row is a failed-simulation
/// stub and renders as `N/A (simulation failed)` in the value column, with the
/// configured limit and `FAIL` in the check columns — so a breach is never
/// shown as a missing network measurement.
///
/// The output is built in a single buffer sized up front: rows are appended with
/// `push_str` and values are formatted in place, so rendering allocates once
/// (plus a regrowth if the estimate is short) instead of once or twice per row.
pub(crate) fn render_markdown(reports: &[CostReport]) -> String {
    // `--check` populates `limit`/`pass`; the plain report leaves both `None`.
    let check = reports
        .iter()
        .any(|r| r.limit.is_some() || r.pass.is_some());

    let row_overhead = if check {
        CHECK_ROW_OVERHEAD
    } else {
        ROW_OVERHEAD
    };
    let rows_len: usize = reports
        .iter()
        .map(|r| r.package.len() + r.function.len() + row_overhead)
        .sum();
    let header = if check { CHECK_HEADER } else { HEADER };
    let mut out = String::with_capacity(header.len() + rows_len + FOOTER.len());
    out.push_str(header);

    for r in reports {
        out.push_str("| ");
        out.push_str(&escape_cell(&r.package));
        out.push_str(" | ");
        out.push_str(&escape_cell(&r.function));
        out.push_str(" | ");
        out.push_str(r.metric);
        out.push_str(" | ");
        match r.value {
            Some(v) => push_with_commas(&mut out, u64::from(v)),
            None if check => out.push_str(NOT_MEASURED_CHECK),
            None => out.push_str(NOT_MEASURED),
        }
        if check {
            out.push_str(" | ");
            match r.limit {
                Some(limit) => push_with_commas(&mut out, limit),
                None => out.push_str(NOT_APPLICABLE),
            }
            out.push_str(" | ");
            out.push_str(match r.pass {
                Some(true) => "PASS",
                Some(false) => "FAIL",
                None => NOT_APPLICABLE,
            });
        }
        out.push_str(" |\n");
    }

    out.push_str(FOOTER);
    out
}

/// Appends `n` in decimal with a comma between each group of three digits
/// (`2654615` becomes `2,654,615`), without allocating.
///
/// The number is split into base-1000 groups, least significant first. A `u64`
/// has at most twenty digits, so seven groups always suffice. The leading group
/// is written as-is; every later group is zero-padded to three digits so that
/// `1_000_001` renders as `1,000,001` and not `1,0,1`.
fn push_with_commas(out: &mut String, n: u64) {
    let mut groups = [0u16; 7];
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

    fn checked_report(
        package: &str,
        function: &str,
        metric: &'static str,
        value: Option<u32>,
        limit: Option<u64>,
        pass: Option<bool>,
    ) -> CostReport {
        CostReport {
            package: package.to_string(),
            function: function.to_string(),
            metric,
            value,
            limit,
            pass,
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
        push_with_commas(&mut out, u64::from(n));
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
    fn push_with_commas_handles_full_u64_range() {
        let mut out = String::new();
        push_with_commas(&mut out, u64::MAX);
        assert_eq!(out, "18,446,744,073,709,551,615");
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
    fn render_markdown_escapes_pipes_in_cell_text() {
        // Pipes in package or function names must be escaped so the Markdown
        // table structure is preserved.
        let md = render_markdown(&[report("a|b", "c d", "CPU Instructions", Some(1))]);
        assert!(md.contains(r"| a\|b | c d | CPU Instructions | 1 |"));
    }

    #[test]
    fn render_markdown_omits_check_columns_without_metadata() {
        let md = render_markdown(&[report("pkg", "fn_a", "CPU Instructions", Some(1))]);
        assert!(
            md.contains("| Package | Function | Metric | Value |"),
            "{md}"
        );
        assert!(
            !md.contains("| Limit |"),
            "no check columns without metadata"
        );
        assert!(!md.contains("PASS") && !md.contains("FAIL"), "{md}");
    }

    #[test]
    fn render_markdown_check_metadata_adds_limit_and_status_columns() {
        let md = render_markdown(&[checked_report(
            "pkg",
            "fn_a",
            "CPU Instructions",
            Some(1_234_567),
            Some(5_000_000),
            Some(true),
        )]);
        assert!(
            md.contains("| Package | Function | Metric | Value | Limit | Status |"),
            "{md}"
        );
        assert!(
            md.contains("| pkg | fn_a | CPU Instructions | 1,234,567 | 5,000,000 | PASS |"),
            "{md}"
        );
    }

    #[test]
    fn render_markdown_pass_false_differs_from_pass_none() {
        let failing = render_markdown(&[checked_report(
            "pkg",
            "fn_a",
            "Write Bytes",
            Some(4_096),
            Some(1_000),
            Some(false),
        )]);
        let unmeasured = render_markdown(&[checked_report(
            "pkg",
            "fn_a",
            "Write Bytes",
            Some(4_096),
            Some(1_000),
            None,
        )]);

        assert!(failing.contains("| FAIL |"), "{failing}");
        assert!(!unmeasured.contains("| FAIL |"), "{unmeasured}");
        assert!(
            failing.contains("| pkg | fn_a | Write Bytes | 4,096 | 1,000 | FAIL |"),
            "{failing}"
        );
        assert!(
            unmeasured.contains("| pkg | fn_a | Write Bytes | 4,096 | 1,000 | - |"),
            "{unmeasured}"
        );
        assert_ne!(failing, unmeasured);
    }

    #[test]
    fn render_markdown_stub_row_is_not_reported_as_a_testnet_gap() {
        // A stub emitted by `emit_check_failure_entries`: no value, a breached
        // limit, `pass: false`. It must not read as "N/A (testnet required)".
        let md = render_markdown(&[checked_report(
            "pkg",
            "broken_sim",
            "CPU Instructions",
            None,
            Some(5_000_000),
            Some(false),
        )]);
        assert!(md.contains("N/A (simulation failed)"), "{md}");
        assert!(
            !md.contains("N/A (testnet required)"),
            "a failed-check stub must not read as a missing testnet measurement: {md}"
        );
        assert!(
            md.contains("| pkg | broken_sim | CPU Instructions | N/A (simulation failed) | 5,000,000 | FAIL |"),
            "{md}"
        );
    }

    #[test]
    fn render_markdown_stub_without_limit_still_shows_failure() {
        // `emit_check_failure_entries` also stubs metrics with no configured
        // limit (`limit: None`); the row must still be visible as failed.
        let md = render_markdown(&[checked_report(
            "pkg",
            "broken_sim",
            "Read Bytes",
            None,
            None,
            Some(false),
        )]);
        assert!(
            md.contains("| pkg | broken_sim | Read Bytes | N/A (simulation failed) | - | FAIL |"),
            "{md}"
        );
    }
}
