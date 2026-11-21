pub struct CostReport {
    pub package: String,
    pub function: String,
    pub metric: &'static str,
    pub value: Option<u32>,
    pub limit: Option<u64>,
    pub pass: Option<bool>,
}

#[path = "../src/html_output.rs"]
mod html_output;

fn formatted_limit(value: u64) -> String {
    let html = html_output::render_html(
        &[CostReport {
            package: "pkg".to_string(),
            function: "fn".to_string(),
            metric: "CPU Instructions",
            value: None,
            limit: Some(value),
            pass: None,
        }],
        true,
    );

    let marker = format!("data-value=\"{value}\">");
    let start = html
        .find(&marker)
        .unwrap_or_else(|| panic!("no cell for limit {value}: {html}"))
        + marker.len();
    let end = start + html[start..].find("</td>").expect("closing cell");
    html[start..end].to_string()
}

#[test]
fn format_thousands_at_boundary_values() {
    for (value, expected) in [
        (0, "0"),
        (1, "1"),
        (999, "999"),
        (1_000, "1,000"),
        (999_999, "999,999"),
        (1_000_000, "1,000,000"),
        (1_000_001, "1,000,001"),
        (999_999_999, "999,999,999"),
        (1_000_000_000, "1,000,000,000"),
        (u64::MAX, "18,446,744,073,709,551,615"),
    ] {
        assert_eq!(formatted_limit(value), expected, "value {value}");
    }
}

#[test]
fn formatted_numbers_round_trip_when_commas_are_stripped() {
    for value in [0, 999, 1_000, 999_999, 1_000_000, u64::MAX] {
        let formatted = formatted_limit(value);
        let stripped: String = formatted.chars().filter(|ch| *ch != ',').collect();
        assert_eq!(stripped.parse::<u64>().unwrap(), value, "value {value}");
    }
}

#[test]
fn commas_only_appear_at_thousand_separators() {
    for value in [0, 999, 1_000, 999_999, 1_000_000, u64::MAX] {
        let formatted = formatted_limit(value);
        let digits = value.to_string().len();
        let expected_commas = digits.saturating_sub(1) / 3;
        assert_eq!(
            formatted.matches(',').count(),
            expected_commas,
            "value {value}, formatted {formatted}"
        );
    }
}
