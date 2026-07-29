//! Unit tests targeting additional off-by-one and zero-length edge cases.
//!
//! Complements `module_8` by exercising boundary conditions in functions that
//! were not yet covered there. Focuses on:
//! - Off-by-one errors in `evaluate_check` at type-cast boundaries
//! - Zero-length and exactly-one-length inputs across all helpers
//! - Comma-insertion boundaries in `format_with_commas_and_units`
//! - JSON edge cases in `TransactionData::parse_json`
//! - TOML edge cases in `load_budget_toml`

#[cfg(test)]
mod off_by_one_and_zero_length_tests {
    use crate::*;
    use crate::args::ArgSpec;

    // ── evaluate_check additional boundary tests ───────────────────────

    #[test]
    fn evaluate_check_value_one_limit_one_passes() {
        let (limit, pass) = evaluate_check(1, Some(1));
        assert_eq!(limit, Some(1));
        assert_eq!(pass, Some(true));
    }

    #[test]
    fn evaluate_check_value_u32_max_minus_one_limit_u32_max_passes() {
        let (limit, pass) = evaluate_check(u32::MAX - 1, Some(u64::from(u32::MAX)));
        assert_eq!(limit, Some(u64::from(u32::MAX)));
        assert_eq!(pass, Some(true));
    }

    #[test]
    fn evaluate_check_value_one_limit_none_returns_none() {
        let (limit, pass) = evaluate_check(1, None);
        assert_eq!(limit, None);
        assert_eq!(pass, None);
    }

    #[test]
    fn evaluate_check_value_u32_max_limit_zero_fails() {
        let (limit, pass) = evaluate_check(u32::MAX, Some(0));
        assert_eq!(limit, Some(0));
        assert_eq!(pass, Some(false));
    }

    // ── limit_for_metric additional edge cases ─────────────────────────

    #[test]
    fn limit_for_metric_metric_exactly_bytes() {
        let config = FunctionConfig {
            args: Default::default(),
            cpu_limit: Some(5_000_000),
            read_limit: Some(1_000),
            write_limit: Some(500),
            mem_limit: None,
            cpu_limit_pct: None,
            read_limit_pct: None,
            write_limit_pct: None,
            tolerance: None,
        };
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
        let config = FunctionConfig {
            args: Default::default(),
            cpu_limit: Some(5_000_000),
            read_limit: None,
            write_limit: None,
            mem_limit: None,
            cpu_limit_pct: None,
            read_limit_pct: None,
            write_limit_pct: None,
            tolerance: None,
        };
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
        let args = build_invoke_args("", "", "", "", &[]);
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
        );
        assert_eq!(args[11], "--value");
        assert_eq!(args[12], "!@#$%^&*()");
    }

    #[test]
    fn build_invoke_args_args_with_empty_strings() {
        let args = build_invoke_args("C", "alice", "testnet", "f", &["".into(), "".into()]);
        assert_eq!(args[11], "");
        assert_eq!(args[12], "");
    }

    #[test]
    fn build_invoke_args_function_name_with_special_characters() {
        let args = build_invoke_args("C", "alice", "testnet", "do_work_123!", &[]);
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
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        std::fs::write(tmp.path(), "[lints]\nunused_imports = \"warn\"\n").unwrap();
        let config = load_budget_toml(tmp.path())
            .expect("file with only foreign sections should parse as default");
        assert!(config.network.is_none());
        assert!(config.source.is_none());
        assert!(config.functions.is_empty());
    }

    #[test]
    fn load_budget_toml_no_trailing_newline_parses_correctly() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        std::fs::write(tmp.path(), "network = \"testnet\"").unwrap();
        let config = load_budget_toml(tmp.path())
            .expect("file without trailing newline should parse correctly");
        assert_eq!(config.network.as_deref(), Some("testnet"));
    }

    #[test]
    fn load_budget_toml_extra_top_level_fields_ignored() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        let content = r#"
network = "testnet"
source = "alice"
unknown_top_level = "should be ignored"

[functions.do_work]
cpu_limit = 5000000
"#;
        std::fs::write(tmp.path(), content).unwrap();
        let config =
            load_budget_toml(tmp.path()).expect("extra top-level fields should be tolerated");
        assert_eq!(config.network.as_deref(), Some("testnet"));
        assert_eq!(config.source.as_deref(), Some("alice"));
        assert!(config.functions.contains_key("do_work"));
    }

    #[test]
    fn load_budget_toml_function_config_args_preserved() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        let content = r#"
[functions.do_work]
args = ["--n", "10000", "--flag"]
cpu_limit = 5000000
"#;
        std::fs::write(tmp.path(), content).unwrap();
        let config = load_budget_toml(tmp.path()).expect("should parse args correctly");
        let func = config.functions.get("do_work").unwrap();
        assert_eq!(func.args.encode(), vec!["--n".to_string(), "10000".to_string(), "--flag".to_string()]);
        assert_eq!(func.cpu_limit, Some(5_000_000));
        assert!(func.read_limit.is_none());
        assert!(func.write_limit.is_none());
    }

    // ── CSV output additional edge cases ───────────────────────────────

    fn reports_to_csv(reports: &[CostReport], check: bool) -> String {
        let mut wtr = csv::Writer::from_writer(vec![]);
        if check {
            wtr.write_record(["package", "function", "metric", "value", "limit", "pass"])
                .unwrap();
            for r in reports {
                let value_str = r.value.map(|v| v.to_string()).unwrap_or_default();
                let limit_str = r.limit.map(|l| l.to_string()).unwrap_or_default();
                let pass_str = r.pass.map(|p| p.to_string()).unwrap_or_default();
                wtr.write_record([
                    r.package.as_str(),
                    r.function.as_str(),
                    r.metric,
                    value_str.as_str(),
                    limit_str.as_str(),
                    pass_str.as_str(),
                ])
                .unwrap();
            }
        } else {
            wtr.write_record(["package", "function", "metric", "value"])
                .unwrap();
            for r in reports {
                if r.value.is_some() {
                    let value_str = r.value.map(|v| v.to_string()).unwrap_or_default();
                    wtr.write_record([
                        r.package.as_str(),
                        r.function.as_str(),
                        r.metric,
                        value_str.as_str(),
                    ])
                    .unwrap();
                }
            }
        }
        wtr.flush().unwrap();
        String::from_utf8(wtr.into_inner().unwrap()).unwrap()
    }

    #[test]
    fn csv_output_with_check_limit_none_pass_false() {
        let reports = vec![CostReport {
            package: "p".to_string(),
            function: "f".to_string(),
            metric: "CPU Instructions",
            value: None,
            limit: None,
            pass: Some(false),
            resource_limit: None,
            share_pct: None,
        }];
        let csv = reports_to_csv(&reports, true);
        assert!(csv.contains("p,f,CPU Instructions,,,false"));
    }

    #[test]
    fn csv_output_with_check_limit_some_pass_none() {
        let reports = vec![CostReport {
            package: "p".to_string(),
            function: "f".to_string(),
            metric: "CPU Instructions",
            value: Some(100),
            limit: Some(200),
            pass: None,
            resource_limit: None,
            share_pct: None,
        }];
        let csv = reports_to_csv(&reports, true);
        assert!(csv.contains("p,f,CPU Instructions,100,200,"));
    }

    #[test]
    fn csv_output_without_check_empty_package_name() {
        let reports = vec![CostReport {
            package: "".to_string(),
            function: "f".to_string(),
            metric: "CPU Instructions",
            value: Some(100),
            limit: None,
            pass: None,
            resource_limit: None,
            share_pct: None,
        }];
        let csv = reports_to_csv(&reports, false);
        assert!(csv.contains(",f,CPU Instructions,100"));
    }

    #[test]
    fn csv_output_without_check_empty_function_name() {
        let reports = vec![CostReport {
            package: "p".to_string(),
            function: "".to_string(),
            metric: "CPU Instructions",
            value: Some(100),
            limit: None,
            pass: None,
            resource_limit: None,
            share_pct: None,
        }];
        let csv = reports_to_csv(&reports, false);
        assert!(csv.contains("p,,CPU Instructions,100"));
    }

    #[test]
    fn csv_output_value_u32_max() {
        let reports = vec![CostReport {
            package: "p".to_string(),
            function: "f".to_string(),
            metric: "CPU Instructions",
            value: Some(u32::MAX),
            limit: None,
            pass: None,
            resource_limit: None,
            share_pct: None,
        }];
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
