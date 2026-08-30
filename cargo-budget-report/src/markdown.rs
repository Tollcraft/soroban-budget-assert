use crate::CostReport;

/// Renders a slice of [`CostReport`] entries into a GitHub-Flavored Markdown table
/// suitable for appending to `$GITHUB_STEP_SUMMARY`.
pub(crate) fn render_markdown(reports: &[CostReport]) -> String {
    let mut out = String::new();
    out.push_str("# Workspace Budget Report (Tier A Local Measurements)\n\n");
    out.push_str("| Package | Function | Metric | Value |\n");
    out.push_str("|---|---|---|---:|\n");

    for r in reports {
        let val_str = match r.value {
            Some(v) => format_number(v),
            None => "N/A (testnet required)".to_string(),
        };
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            r.package, r.function, r.metric, val_str
        ));
    }

    out.push_str("\n---\n");
    out.push_str("_Simulated resource amounts from local WASM test harness. Network-dependent billing metrics (Read/Write Bytes) require testnet simulation._\n");
    out
}

#[allow(clippy::manual_is_multiple_of)]
fn format_number(n: u32) -> String {
    let s = n.to_string();
    let mut result = String::new();
    let len = s.len();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result
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
}
