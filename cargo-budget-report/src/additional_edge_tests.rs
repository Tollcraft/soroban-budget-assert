//! Unit tests targeting additional off-by-one and zero-length edge cases.
//!
//! Complements `edge_case_tests` by exercising boundary conditions in functions that
//! were not yet covered there. Focuses on:
//! - Off-by-one errors in `evaluate_check` at type-cast boundaries
//! - Zero-length and exactly-one-length inputs across all helpers
//! - Comma-insertion boundaries in `format_with_commas_and_units`
//! - JSON edge cases in `TransactionData::parse_json`
//! - TOML edge cases in `load_budget_toml`
//!
//! Shared builders and the CSV renderer live in the `helpers` module so the
//! test bodies only carry inputs and expectations.

/// Small builders and CSV renderers shared by the tests below, kept apart so
/// each test body only states its inputs and expectations.
/// 
/// This module encapsulates test fixtures such as dummy function configuration builders,
/// resource report generators, and a mirror of the CSV reporting pipeline.
#[cfg(test)]
mod helpers {
    use crate::*;

    /// Constructs a [`FunctionConfig`] populated with the specified resource limits
    /// and empty argument vectors and no custom tolerance overrides. Useful for testing
    /// threshold enforcement boundaries.
    pub(super) fn config_with_limits(
        cpu: Option<u64>,
        read: Option<u64>,
        write: Option<u64>,
    ) -> FunctionConfig {
        FunctionConfig {
            args: vec![],
            cpu_limit: cpu,
            read_limit: read,
            write_limit: write,
            tolerance: None,
        }
    }

    /// Constructs a sample `CPU Instructions` [`CostReport`] entry for mock reporting validation.
    pub(super) fn cpu_report(
        package: &str,
        function: &str,
        value: Option<u32>,
        limit: Option<u64>,
        pass: Option<bool>,
    ) -> CostReport {
        CostReport {
            package: package.to_string(),
            function: function.to_string(),
            metric: "CPU Instructions",
            value,
            limit,
            pass,
        }
    }

    /// Write `contents` to a temp file and run [`load_budget_toml`] on it.
    pub(super) fn load_toml_str(contents: &str) -> crate::error::Result<BudgetToml> {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        std::fs::write(tmp.path(), contents).unwrap();
        load_budget_toml(tmp.path())
    }

    /// Render an optional value as a CSV cell: the value, or empty.
    fn cell<T: ToString>(v: Option<T>) -> String {
        v.map(|v| v.to_string()).unwrap_or_default()
    }

    /// `--check` layout: every row, six columns.
    fn write_check_rows(wtr: &mut csv::Writer<Vec<u8>>, reports: &[CostReport]) {
        wtr.write_record(["package", "function", "metric", "value", "limit", "pass"])
            .unwrap();
        for r in reports {
            let (value, limit, pass) = (cell(r.value), cell(r.limit), cell(r.pass));
            wtr.write_record([
                r.package.as_str(),
                r.function.as_str(),
                r.metric,
                value.as_str(),
                limit.as_str(),
                pass.as_str(),
            ])
            .unwrap();
        }
    }

    /// Plain layout: four columns, rows without a measured value skipped.
    fn write_value_rows(wtr: &mut csv::Writer<Vec<u8>>, reports: &[CostReport]) {
        wtr.write_record(["package", "function", "metric", "value"])
            .unwrap();
        for r in reports.iter().filter(|r| r.value.is_some()) {
            let value = cell(r.value);
            wtr.write_record([
                r.package.as_str(),
                r.function.as_str(),
                r.metric,
                value.as_str(),
            ])
            .unwrap();
        }
    }

    /// Serializes a slice of [`CostReport`] entries into a CSV formatted string,
    /// mirroring the CLI's `--csv` output structure for both plain value reports
    /// and check-mode assertion reports.
    pub(super) fn reports_to_csv(reports: &[CostReport], check: bool) -> String {
        let mut wtr = csv::Writer::from_writer(vec![]);
        if check {
            write_check_rows(&mut wtr, reports);
        } else {
            write_value_rows(&mut wtr, reports);
        }
        wtr.flush().unwrap();
        String::from_utf8(wtr.into_inner().unwrap()).unwrap()
    }

    #[test]
    fn cell_renders_none_as_empty() {
        assert_eq!(cell::<u32>(None), "");
        assert_eq!(cell(Some(0u32)), "0");
        assert_eq!(cell(Some(false)), "false");
    }

    #[test]
    fn csv_with_no_reports_is_header_only() {
        assert_eq!(
            reports_to_csv(&[], false),
            "package,function,metric,value\n"
        );
        assert_eq!(
            reports_to_csv(&[], true),
            "package,function,metric,value,limit,pass\n"
        );
    }

    #[test]
    fn csv_without_check_skips_rows_without_value() {
        let reports = [
            cpu_report("p", "measured", Some(1), None, None),
            cpu_report("p", "failed", None, Some(5), Some(false)),
        ];
        let csv = reports_to_csv(&reports, false);
        assert!(csv.contains("measured"));
        assert!(!csv.contains("failed"));
    }

    #[test]
    fn load_toml_str_surfaces_parse_errors() {
        assert!(load_toml_str("network = ").is_err());
    }
}

/// Unit tests covering numerical precision boundaries, type-cast thresholds,
/// and zero-length inputs across core parser and formatter functions.
#[cfg(test)]
mod off_by_one_and_zero_length_tests {
    use super::helpers::{config_with_limits, cpu_report, load_toml_str, reports_to_csv};
    use crate::*;

    // ── evaluate_check additional boundary tests ───────────────────────

    #[test]
    fn evaluate_check_value_one_limit_one_passes() {
        assert_eq!(evaluate_check(1, Some(1)), (Some(1), Some(true)));
    }

    #[test]
    fn evaluate_check_value_u32_max_minus_one_limit_u32_max_passes() {
        let max = u64::from(u32::MAX);
        assert_eq!(
            evaluate_check(u32::MAX - 1, Some(max)),
            (Some(max), Some(true))
        );
    }

    #[test]
    fn evaluate_check_value_one_limit_none_returns_none() {
        assert_eq!(evaluate_check(1, None), (None, None));
    }

    #[test]
    fn evaluate_check_value_u32_max_limit_zero_fails() {
        assert_eq!(evaluate_check(u32::MAX, Some(0)), (Some(0), Some(false)));
    }

    // ── limit_for_metric additional edge cases ─────────────────────────

    #[test]
    fn limit_for_metric_metric_exactly_bytes() {
        let config = config_with_limits(Some(5_000_000), Some(1_000), Some(500));
        // "Bytes" (exact match, no prefix) does not match any known metric.
        assert_eq!(limit_for_metric(&config, "Bytes"), None);
    }

    #[test]
    fn limit_for_metric_whitespace_only_string() {
        let config = FunctionConfig::default();
        assert_eq!(limit_for_metric(&config, "   "), None);
        assert_eq!(limit_for_metric(&config, "\t"), None);
    }

    #[test]
    fn limit_for_metric_newline_terminated_metric() {
        let config = config_with_limits(Some(5_000_000), None, None);
        assert_eq!(limit_for_metric(&config, "CPU Instructions\n"), None);
    }

    #[test]
    fn limit_for_metric_null_byte_in_metric_name() {
        let config = FunctionConfig::default();
        assert_eq!(limit_for_metric(&config, "CPU\0 Instructions"), None);
    }

    // ── format_with_commas_and_units comma-boundary tests ──────────────

    #[test]
    fn formatter_ten_thousand_no_comma() {
        assert_eq!(
            format_with_commas_and_units(10_000, "CPU Instructions"),
            "10,000 inst."
        );
    }

    #[test]
    fn formatter_one_hundred_thousand() {
        assert_eq!(
            format_with_commas_and_units(100_000, "CPU Instructions"),
            "100,000 inst."
        );
    }

    #[test]
    fn formatter_nine_hundred_ninety_nine_thousand_nine_hundred_ninety_nine() {
        assert_eq!(
            format_with_commas_and_units(999_999, "CPU Instructions"),
            "999,999 inst."
        );
    }

    #[test]
    fn formatter_one_million_exactly() {
        assert_eq!(
            format_with_commas_and_units(1_000_000, "CPU Instructions"),
            "1,000,000 inst."
        );
    }

    #[test]
    fn formatter_multi_comma_boundary_ten_million() {
        assert_eq!(
            format_with_commas_and_units(10_000_000, "Read Bytes"),
            "10,000,000 B"
        );
    }

    #[test]
    fn formatter_multi_comma_boundary_one_billion_minus_one() {
        assert_eq!(
            format_with_commas_and_units(999_999_999, "CPU Instructions"),
            "999,999,999 inst."
        );
    }

    #[test]
    fn formatter_metric_contains_bytes_prefix_not_suffix() {
        // "Bytes" appears in the metric name but not as a separate word.
        // The function uses `contains("Bytes")`, so this should still
        // get the byte suffix.
        assert_eq!(format_with_commas_and_units(100, "MyBytesMetric"), "100 B");
    }

    #[test]
    fn formatter_metric_contains_bytes_as_suffix() {
        assert_eq!(format_with_commas_and_units(256, "Some Bytes"), "256 B");
    }

    #[test]
    fn formatter_metric_case_sensitive_no_match() {
        // "bytes" (lowercase) should NOT match due to case-sensitive contains.
        assert_eq!(format_with_commas_and_units(100, "read bytes"), "100 inst.");
    }

    // ── emit_check_failure_entries additional edge cases ───────────────

    #[test]
    fn emit_check_failure_entries_long_package_and_function_names() {
        let mut reports = Vec::new();
        let long_pkg = "a".repeat(256);
        let long_fn = "b".repeat(256);
        let config = FunctionConfig::default();
        emit_check_failure_entries(&mut reports, &long_pkg, &long_fn, &config);
        assert_eq!(reports.len(), 3);
        for r in &reports {
            assert_eq!(r.package.len(), 256);
            assert_eq!(r.function.len(), 256);
        }
    }

    #[test]
    fn emit_check_failure_entries_unicode_package_name() {
        let mut reports = Vec::new();
        let config = FunctionConfig::default();
        emit_check_failure_entries(&mut reports, "päckäge", "fünctiön", &config);
        assert_eq!(reports.len(), 3);
        for r in &reports {
            assert_eq!(r.package, "päckäge");
            assert_eq!(r.function, "fünctiön");
        }
    }

    // ── build_invoke_args additional zero-length edge cases ────────────

    #[test]
    fn build_invoke_args_all_empty_strings() {
        let args = build_invoke_args("", "", "", "", &[], None);
        assert_eq!(args.len(), 11);
        assert_eq!(args[3], ""); // contract id
        assert_eq!(args[5], ""); // source
        assert_eq!(args[7], ""); // network
        assert_eq!(args[10], ""); // function name
    }

    #[test]
    fn build_invoke_args_special_character_in_arg() {
        let args = build_invoke_args(
            "C",
            "alice",
            "testnet",
            "transfer",
            &["--value".into(), "!@#$%^&*()".into()],
            None,
        );
        assert_eq!(args[11], "--value");
        assert_eq!(args[12], "!@#$%^&*()");
    }

    #[test]
    fn build_invoke_args_args_with_empty_strings() {
        let args = build_invoke_args("C", "alice", "testnet", "f", &["".into(), "".into()], None);
        assert_eq!(args[11], "");
        assert_eq!(args[12], "");
    }

    #[test]
    fn build_invoke_args_function_name_with_special_characters() {
        let args = build_invoke_args("C", "alice", "testnet", "do_work_123!", &[], None);
        assert_eq!(args.last(), Some(&"do_work_123!".to_string()));
    }

    // ── build_rpc_payload additional edge cases ────────────────────────

    #[test]
    fn build_rpc_payload_unicode_xdr() {
        let payload = build_rpc_payload("héllo_wörld");
        assert_eq!(payload["params"]["transaction"], "héllo_wörld");
        assert_eq!(payload["jsonrpc"], "2.0");
    }

    // ── TransactionData::parse_json additional edge cases ──────────────

    #[test]
    fn transaction_data_parse_extra_unknown_fields() {
        let json_str = r#"{
            "resources": {
                "instructions": 100,
                "disk_read_bytes": 200,
                "write_bytes": 300
            },
            "extra_field": "ignored"
        }"#;
        let tx_data =
            TransactionData::parse_json(json_str).expect("extra fields should be ignored");
        assert_eq!(tx_data.resources.instructions, 100);
        assert_eq!(tx_data.resources.disk_read_bytes, 200);
        assert_eq!(tx_data.resources.write_bytes, 300);
    }

    #[test]
    fn transaction_data_parse_large_number_below_u64_max() {
        let json_str = r#"{"resources": {"instructions": 9223372036854775807, "disk_read_bytes": 0, "write_bytes": 0}}"#;
        let tx_data = TransactionData::parse_json(json_str)
            .expect("should parse large i64-compatible numbers");
        assert_eq!(tx_data.resources.instructions, 9_223_372_036_854_775_807);
    }

    #[test]
    fn transaction_data_parse_json_with_extra_resources_fields() {
        let json_str = r#"{
            "resources": {
                "instructions": 50,
                "disk_read_bytes": 100,
                "write_bytes": 150,
                "footprint": "some_value"
            }
        }"#;
        let tx_data = TransactionData::parse_json(json_str)
            .expect("extra resources fields should be ignored");
        assert_eq!(tx_data.resources.instructions, 50);
        assert_eq!(tx_data.resources.disk_read_bytes, 100);
        assert_eq!(tx_data.resources.write_bytes, 150);
    }

    #[test]
    fn transaction_data_parse_floating_point_value_fails() {
        let json_str =
            r#"{"resources": {"instructions": 100.5, "disk_read_bytes": 0, "write_bytes": 0}}"#;
        let result = TransactionData::parse_json(json_str);
        assert!(result.is_err(), "floating point should fail for u64 field");
    }

    #[test]
    fn transaction_data_parse_empty_string_fails() {
        let result = TransactionData::parse_json("");
        assert!(result.is_err());
    }

    #[test]
    fn transaction_data_parse_null_resources_fails() {
        let json_str = r#"{"resources": null}"#;
        let result = TransactionData::parse_json(json_str);
        assert!(result.is_err());
    }

    // ── load_budget_toml additional edge cases ─────────────────────────

    #[test]
    fn load_budget_toml_only_foreign_section_returns_default() {
        let config = load_toml_str("[lints]\nunused_imports = \"warn\"\n")
            .expect("file with only foreign sections should parse as default");
        assert!(config.network.is_none());
        assert!(config.source.is_none());
        assert!(config.functions.is_empty());
    }

    #[test]
    fn load_budget_toml_no_trailing_newline_parses_correctly() {
        let config = load_toml_str("network = \"testnet\"")
            .expect("file without trailing newline should parse correctly");
        assert_eq!(config.network.as_deref(), Some("testnet"));
    }

    #[test]
    fn load_budget_toml_extra_top_level_fields_ignored() {
        let content = r#"
network = "testnet"
source = "alice"
unknown_top_level = "should be ignored"

[functions.do_work]
cpu_limit = 5000000
"#;
        let config = load_toml_str(content).expect("extra top-level fields should be tolerated");
        assert_eq!(config.network.as_deref(), Some("testnet"));
        assert_eq!(config.source.as_deref(), Some("alice"));
        assert!(config.functions.contains_key("do_work"));
    }

    #[test]
    fn load_budget_toml_function_config_args_preserved() {
        let content = r#"
[functions.do_work]
args = ["--n", "10000", "--flag"]
cpu_limit = 5000000
"#;
        let config = load_toml_str(content).expect("should parse args correctly");
        let func = config.functions.get("do_work").unwrap();
        assert_eq!(
            func.args,
            ["--n", "10000", "--flag"]
                .into_iter()
                .map(|s| crate::arg_spec::ArgSpec::Raw(s.to_string()))
                .collect::<Vec<_>>()
        );
        assert_eq!(func.cpu_limit, Some(5_000_000));
        assert!(func.read_limit.is_none());
        assert!(func.write_limit.is_none());
    }

    // ── CSV output additional edge cases ───────────────────────────────

    #[test]
    fn csv_output_with_check_limit_none_pass_false() {
        let reports = [cpu_report("p", "f", None, None, Some(false))];
        let csv = reports_to_csv(&reports, true);
        assert!(csv.contains("p,f,CPU Instructions,,,false"));
    }

    #[test]
    fn csv_output_with_check_limit_some_pass_none() {
        let reports = [cpu_report("p", "f", Some(100), Some(200), None)];
        let csv = reports_to_csv(&reports, true);
        assert!(csv.contains("p,f,CPU Instructions,100,200,"));
    }

    #[test]
    fn csv_output_without_check_empty_package_name() {
        let reports = [cpu_report("", "f", Some(100), None, None)];
        let csv = reports_to_csv(&reports, false);
        assert!(csv.contains(",f,CPU Instructions,100"));
    }

    #[test]
    fn csv_output_without_check_empty_function_name() {
        let reports = [cpu_report("p", "", Some(100), None, None)];
        let csv = reports_to_csv(&reports, false);
        assert!(csv.contains("p,,CPU Instructions,100"));
    }

    #[test]
    fn csv_output_value_u32_max() {
        let reports = [cpu_report("p", "f", Some(u32::MAX), None, None)];
        let csv = reports_to_csv(&reports, false);
        assert!(csv.contains(&u32::MAX.to_string()));
    }

    // ── BudgetToml / FunctionConfig default additional tests ───────────

    #[test]
    fn budget_toml_default_network_is_none() {
        let config = BudgetToml::default();
        assert!(config.network.is_none());
    }

    #[test]
    fn budget_toml_default_source_is_none() {
        let config = BudgetToml::default();
        assert!(config.source.is_none());
    }

    #[test]
    fn function_config_default_args_empty() {
        let config = FunctionConfig::default();
        assert!(config.args.is_empty());
    }

    #[test]
    fn function_config_default_all_limits_none() {
        let config = FunctionConfig::default();
        assert!(config.cpu_limit.is_none());
        assert!(config.read_limit.is_none());
        assert!(config.write_limit.is_none());
    }
}
