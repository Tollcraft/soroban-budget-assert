//! Integration tests pinning `soroban_budget_assert_core::traits::format_metric`
//! digit grouping across 3-digit / 4-digit boundaries up to `u64::MAX` and metric suffix rules.

use soroban_budget_assert_core::traits::{
    format_metric, METRIC_CPU_INSTRUCTIONS, METRIC_MEMORY_BYTES, METRIC_NAMES, METRIC_READ_BYTES,
    METRIC_WASM_BYTES, METRIC_WRITE_BYTES,
};

fn power_of_ten_boundaries() -> Vec<u64> {
    let mut values = vec![0, 1, 9, 10, u64::from(u32::MAX), u64::MAX - 1, u64::MAX];
    for exp in 1u32..=19 {
        let p = 10u64.pow(exp);
        values.push(p - 1);
        values.push(p);
        if let Some(next) = p.checked_add(1) {
            values.push(next);
        }
    }
    values.sort_unstable();
    values.dedup();
    values
}

#[test]
fn format_metric_pins_digit_grouping_boundaries_up_to_u64_max() {
    let cases: &[(u64, &str)] = &[
        (0, "0"),
        (1, "1"),
        (9, "9"),
        (10, "10"),
        (99, "99"),
        (100, "100"),
        (999, "999"),
        (1_000, "1,000"),
        (1_001, "1,001"),
        (9_999, "9,999"),
        (10_000, "10,000"),
        (99_999, "99,999"),
        (100_000, "100,000"),
        (999_999, "999,999"),
        (1_000_000, "1,000,000"),
        (9_999_999, "9,999,999"),
        (10_000_000, "10,000,000"),
        (999_999_999, "999,999,999"),
        (1_000_000_000, "1,000,000,000"),
        (u64::from(u32::MAX), "4,294,967,295"),
        (999_999_999_999, "999,999,999,999"),
        (1_000_000_000_000, "1,000,000,000,000"),
        (999_999_999_999_999, "999,999,999,999,999"),
        (1_000_000_000_000_000, "1,000,000,000,000,000"),
        (999_999_999_999_999_999, "999,999,999,999,999,999"),
        (1_000_000_000_000_000_000, "1,000,000,000,000,000,000"),
        (9_999_999_999_999_999_999, "9,999,999,999,999,999,999"),
        (10_000_000_000_000_000_000, "10,000,000,000,000,000,000"),
        (u64::MAX, "18,446,744,073,709,551,615"),
    ];

    for &(value, expected_digits) in cases {
        assert_eq!(
            format_metric(value, METRIC_CPU_INSTRUCTIONS),
            format!("{expected_digits} inst."),
            "unexpected inst. formatting for {value}"
        );
        assert_eq!(
            format_metric(value, METRIC_READ_BYTES),
            format!("{expected_digits} B"),
            "unexpected Bytes formatting for {value}"
        );
    }
}

#[test]
fn format_metric_all_powers_of_ten_have_valid_comma_groups() {
    for value in power_of_ten_boundaries() {
        let formatted = format_metric(value, METRIC_CPU_INSTRUCTIONS);
        let digits = formatted
            .strip_suffix(" inst.")
            .expect("CPU Instructions metric must end with \" inst.\"");

        assert!(
            !digits.starts_with(',') && !digits.ends_with(','),
            "leading or trailing comma in {digits:?} for {value}"
        );

        let groups: Vec<&str> = digits.split(',').collect();
        assert!(
            (1..=3).contains(&groups[0].len()),
            "first comma group length must be 1..=3 in {digits:?} for {value}"
        );
        assert!(
            groups[0].chars().all(|c| c.is_ascii_digit()),
            "first group contains non-digit in {digits:?} for {value}"
        );

        for group in &groups[1..] {
            assert_eq!(
                group.len(),
                3,
                "subsequent comma group length must be 3 in {digits:?} for {value}"
            );
            assert!(
                group.chars().all(|c| c.is_ascii_digit()),
                "subsequent group contains non-digit in {digits:?} for {value}"
            );
        }

        let ungrouped_len = value.to_string().len();
        let expected_commas = (ungrouped_len - 1) / 3;
        assert_eq!(
            digits.matches(',').count(),
            expected_commas,
            "comma count mismatch for {value} ({digits:?})"
        );
    }
}

#[test]
fn format_metric_round_trips_after_stripping_commas() {
    let metrics = [
        (METRIC_CPU_INSTRUCTIONS, " inst."),
        (METRIC_READ_BYTES, " B"),
        ("Unknown Metric", " inst."),
    ];

    for value in power_of_ten_boundaries() {
        for &(metric, suffix) in &metrics {
            let formatted = format_metric(value, metric);
            let numeric_part = formatted
                .strip_suffix(suffix)
                .unwrap_or_else(|| panic!("missing suffix {suffix:?} in {formatted:?}"));
            let stripped = numeric_part.replace(',', "");
            let parsed = stripped
                .parse::<u64>()
                .unwrap_or_else(|e| panic!("failed to parse {stripped:?} for {value}: {e}"));
            assert_eq!(
                parsed, value,
                "round-trip mismatch for {value} ({metric:?}): formatted as {formatted:?}"
            );
        }
    }
}

#[test]
fn format_metric_standard_metric_suffixes() {
    assert_eq!(format_metric(1_000, METRIC_CPU_INSTRUCTIONS), "1,000 inst.");
    for bytes_metric in [
        METRIC_READ_BYTES,
        METRIC_WRITE_BYTES,
        METRIC_MEMORY_BYTES,
        METRIC_WASM_BYTES,
    ] {
        assert_eq!(format_metric(0, bytes_metric), "0 B");
        assert_eq!(format_metric(999, bytes_metric), "999 B");
        assert_eq!(format_metric(1_000, bytes_metric), "1,000 B");
        assert_eq!(
            format_metric(u64::MAX, bytes_metric),
            "18,446,744,073,709,551,615 B"
        );
    }

    for &metric in METRIC_NAMES {
        let formatted = format_metric(1_000, metric);
        if metric.contains("Bytes") {
            assert_eq!(formatted, "1,000 B", "metric {metric:?} must use B suffix");
        } else {
            assert_eq!(
                formatted, "1,000 inst.",
                "metric {metric:?} must use inst. suffix"
            );
        }
    }
}

#[test]
fn format_metric_bytes_substring_and_unknown_label_fallback() {
    let boundary_values = [0, 999, 1_000, 999_999, 1_000_000, u64::MAX];

    for label in [
        "Bytes",
        "Custom Bytes Metric",
        "BytesTransferred",
        "TotalBytes",
    ] {
        for value in boundary_values {
            let formatted = format_metric(value, label);
            assert!(
                formatted.ends_with(" B"),
                "label {label:?} containing \"Bytes\" should use B suffix, got {formatted:?}"
            );
        }
    }

    for unknown_label in [
        "",
        "Unknown Metric",
        "bytes",
        "READ BYTES",
        "Byte",
        "Gas",
        "Ledger Entries",
    ] {
        for value in boundary_values {
            let formatted = format_metric(value, unknown_label);
            assert!(
                formatted.ends_with(" inst."),
                "unknown/fallback label {unknown_label:?} should use inst. suffix, got {formatted:?}"
            );
        }
    }
}
