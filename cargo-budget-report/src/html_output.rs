//! Self-contained HTML report output for `cargo budget-report --html`.
//!
//! The rendered page is a single file with no external CSS, scripts, or
//! fonts — it renders correctly from a `file://` URL and from a downloaded
//! CI artifact. Every value that originates from the workspace (package and
//! function names) is HTML-escaped before being placed in the page; nothing
//! is interpolated raw.

use crate::CostReport;

/// Render the full report as a single self-contained HTML page.
///
/// * One row per measured metric, mirroring the JSON output for the same run
///   (same values, same metric names).
/// * In `--check` mode each row additionally shows its limit and pass/fail
///   status. Pass/fail is conveyed by the words `PASS`/`FAIL` and a ✓/✗
///   glyph, not by colour alone.
/// * Values are displayed with thousands separators; the raw number is kept
///   in a `data-value` attribute so it can be copied or scripted over.
/// * An empty report renders a valid page with an explicit empty state.
pub fn render_html(reports: &[CostReport], check: bool) -> String {
    let mut html = String::new();
    html.push_str("<!doctype html>\n");
    html.push_str("<html lang=\"en\">\n");
    html.push_str("<head>\n");
    html.push_str("<meta charset=\"utf-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    html.push_str("<title>Workspace Budget Report</title>\n");
    html.push_str("<style>\n");
    html.push_str(
        "body{font-family:system-ui,-apple-system,'Segoe UI',Roboto,sans-serif;margin:2rem auto;max-width:960px;padding:0 1rem;color:#1a202c}\n",
    );
    html.push_str("h1{font-size:1.4rem;margin-bottom:0.25rem}\n");
    html.push_str("p.sub{color:#4a5568;margin-top:0}\n");
    html.push_str("table{border-collapse:collapse;width:100%;margin-top:1rem}\n");
    html.push_str("th,td{border:1px solid #cbd5e0;padding:0.4rem 0.6rem;text-align:left;vertical-align:top}\n");
    html.push_str("th{background:#f7fafc}\n");
    html.push_str("td.num{text-align:right;font-variant-numeric:tabular-nums}\n");
    html.push_str(".pass{color:#16a34a}\n");
    html.push_str(".fail{color:#991b1b;font-weight:600}\n");
    html.push_str(".empty{color:#4a5568;font-style:italic}\n");
    html.push_str("</style>\n");
    html.push_str("</head>\n");
    html.push_str("<body>\n");
    html.push_str("<h1>Workspace Budget Report</h1>\n");
    if check {
        html.push_str("<p class=\"sub\">Check mode: measured values are compared against the limits declared in <code>budget.toml</code>.</p>\n");
    } else {
        html.push_str("<p class=\"sub\">Simulated resource amounts from <code>cargo budget-report</code> — not fees.</p>\n");
    }

    if reports.is_empty() {
        html.push_str("<p class=\"empty\">No measurements were recorded for this run. Nothing failed to simulate, but there is also nothing to report yet.</p>\n");
    } else {
        html.push_str("<table>\n");
        html.push_str("<thead><tr><th>Package</th><th>Function</th><th>Metric</th><th>Value</th>");
        if check {
            html.push_str("<th>Limit</th><th>Status</th>");
        }
        html.push_str("</tr></thead>\n");
        html.push_str("<tbody>\n");
        for report in reports {
            html.push_str("<tr>");
            html.push_str(&format!(
                "<td>{}</td><td>{}</td><td>{}</td>",
                escape_html(&report.package),
                escape_html(&report.function),
                escape_html(report.metric),
            ));
            match report.value {
                Some(value) => {
                    html.push_str(&format!(
                        "<td class=\"num\" data-value=\"{value}\">{}</td>",
                        format_thousands(u64::from(value)),
                    ));
                }
                None => {
                    html.push_str("<td class=\"num\">&mdash;</td>");
                }
            }
            if check {
                match report.limit {
                    Some(limit) => html.push_str(&format!(
                        "<td class=\"num\" data-value=\"{limit}\">{}</td>",
                        format_thousands(limit),
                    )),
                    None => html.push_str("<td class=\"num\">&mdash;</td>"),
                }
                match report.pass {
                    Some(true) => html.push_str("<td class=\"pass\">&#10003; PASS</td>"),
                    Some(false) => html.push_str("<td class=\"fail\">&#10007; FAIL</td>"),
                    None => html.push_str("<td>&mdash;</td>"),
                }
            }
            html.push_str("</tr>\n");
        }
        html.push_str("</tbody>\n");
        html.push_str("</table>\n");
    }

    html.push_str("</body>\n");
    html.push_str("</html>\n");
    html
}

/// Escape a value from the workspace so it can never inject markup.
///
/// Package and function names end up in the page; both are treated as
/// untrusted text.
fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

/// Format a number with thousands separators for display.
fn format_thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            formatted.push(',');
        }
        formatted.push(ch);
    }
    formatted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_report() -> Vec<CostReport> {
        vec![
            CostReport {
                package: "amm-pool-contract".to_string(),
                function: "do_expensive_work".to_string(),
                metric: "CPU Instructions",
                value: Some(1_234_567),
                limit: Some(5_000_000),
                pass: Some(true),
            },
            CostReport {
                package: "amm-pool-contract".to_string(),
                function: "do_expensive_work".to_string(),
                metric: "Read Bytes",
                value: Some(2_048),
                limit: Some(5_000),
                pass: Some(true),
            },
            CostReport {
                package: "amm-pool-contract".to_string(),
                function: "do_expensive_work".to_string(),
                metric: "Write Bytes",
                value: Some(4_096),
                limit: Some(1_000),
                pass: Some(false),
            },
            CostReport {
                package: "amm-pool-contract".to_string(),
                function: "broken_sim".to_string(),
                metric: "CPU Instructions",
                value: None,
                limit: Some(5_000_000),
                pass: Some(false),
            },
        ]
    }

    /// Golden-file test: the rendered page for a fixed report is pinned to
    /// `tests/fixtures/html_report_golden.html`. Bump the fixture file when
    /// the output intentionally changes.
    #[test]
    fn golden_html_output_is_pinned() {
        let html = render_html(&sample_report(), true);
        let golden = include_str!("../tests/fixtures/html_report_golden.html");
        assert_eq!(html, golden);
    }

    #[test]
    fn empty_report_renders_valid_page_with_empty_state() {
        let html = render_html(&[], false);
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("No measurements were recorded"));
        assert!(!html.contains("<table>"));
    }

    #[test]
    fn workspace_values_are_escaped() {
        let reports = vec![CostReport {
            package: "<script>alert('x')</script>".to_string(),
            function: "fn\"&<>".to_string(),
            metric: "CPU Instructions",
            value: Some(1),
            limit: None,
            pass: None,
        }];
        let html = render_html(&reports, false);
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("&quot;"));
        assert!(html.contains("&amp;"));
    }

    #[test]
    fn values_show_thousands_separators_and_raw_data_attribute() {
        let html = render_html(&sample_report(), true);
        assert!(html.contains(">1,234,567</td>"));
        assert!(html.contains("data-value=\"1234567\""));
        assert!(html.contains("data-value=\"5000000\""));
        assert!(html.contains(">5,000,000</td>"));
    }

    #[test]
    fn check_mode_distinguishes_pass_and_fail_without_colour() {
        let html = render_html(&sample_report(), true);
        assert!(html.contains("&#10003; PASS"));
        assert!(html.contains("&#10007; FAIL"));
        assert!(html.contains("<td class=\"pass\">"));
        assert!(html.contains("<td class=\"fail\">"));
    }

    #[test]
    fn non_check_mode_omits_limit_and_status_columns() {
        let html = render_html(&sample_report(), false);
        assert!(!html.contains(">Limit</th>"));
        assert!(!html.contains(">Status</th>"));
        assert!(!html.contains("PASS"));
        assert!(!html.contains("FAIL"));
    }
}
