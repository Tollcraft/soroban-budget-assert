//! Unit tests targeting off-by-one errors and zero-length input edge cases.
//!
//! These tests specifically exercise boundary conditions in the
//! `cargo-budget-report` tool, focusing on:
//! - Off-by-one errors around inclusive/exclusive limit checks
//! - Zero-length and zero-value inputs
//! - Boundary values at type limits (u32::MAX, u64::MAX)

#[cfg(test)]
mod off_by_one_and_zero_length_tests {
    use crate::*;

    // ── evaluate_check off-by-one tests ─────────────────────────────────

    #[test]
    fn evaluate_check_value_zero_limit_zero_passes_inclusive() {
        // Limit is documented as "inclusive upper bound", so value == limit
        // should pass.
        let (limit, pass) = evaluate_check(0, Some(0));
        assert_eq!(limit, Some(0));
        assert_eq!(pass, Some(true));
    }

    #[test]
    fn evaluate_check_value_zero_limit_one_passes() {
        let (limit, pass) = evaluate_check(0, Some(1));
        assert_eq!(limit, Some(1));
        assert_eq!(pass, Some(true));
    }

    #[test]
    fn evaluate_check_value_one_limit_zero_fails_off_by_one() {
        // The smallest possible off-by-one: value is 1, limit is 0.
        let (limit, pass) = evaluate_check(1, Some(0));
        assert_eq!(limit, Some(0));
        assert_eq!(pass, Some(false));
    }

    #[test]
    fn evaluate_check_value_equals_limit_passes_inclusive() {
        let (limit, pass) = evaluate_check(1_000_000, Some(1_000_000));
        assert_eq!(limit, Some(1_000_000));
        assert_eq!(pass, Some(true));
    }

    #[test]
    fn evaluate_check_value_one_below_limit_passes() {
        let (limit, pass) = evaluate_check(999_999, Some(1_000_000));
        assert_eq!(limit, Some(1_000_000));
        assert_eq!(pass, Some(true));
    }

    #[test]
    fn evaluate_check_value_one_above_limit_fails_off_by_one() {
        // Classic off-by-one: value exceeds inclusive limit by exactly 1.
        let (limit, pass) = evaluate_check(1_000_001, Some(1_000_000));
        assert_eq!(limit, Some(1_000_000));
        assert_eq!(pass, Some(false));
    }

    #[test]
    fn evaluate_check_value_u32_max_limit_u32_max_passes_inclusive() {
        let (limit, pass) = evaluate_check(u32::MAX, Some(u64::from(u32::MAX)));
        assert_eq!(limit, Some(u64::from(u32::MAX)));
        assert_eq!(pass, Some(true));
    }

    #[test]
    fn evaluate_check_value_u32_max_limit_below_u32_max_fails() {
        let (limit, pass) = evaluate_check(u32::MAX, Some(u64::from(u32::MAX) - 1));
        assert_eq!(limit, Some(u64::from(u32::MAX) - 1));
        assert_eq!(pass, Some(false));
    }

    #[test]
    fn evaluate_check_value_u32_max_limit_u64_max_passes() {
        // u32::MAX (4,294,967,295) is well within u64::MAX.
        let (limit, pass) = evaluate_check(u32::MAX, Some(u64::MAX));
        assert_eq!(limit, Some(u64::MAX));
        assert_eq!(pass, Some(true));
    }

    #[test]
    fn evaluate_check_no_limit_returns_none_pass() {
        // When no limit is configured, pass is None (metric is reported but
        // not enforced).
        let (limit, pass) = evaluate_check(0, None);
        assert_eq!(limit, None);
        assert_eq!(pass, None);
    }

    #[test]
    fn evaluate_check_no_limit_with_large_value() {
        let (limit, pass) = evaluate_check(u32::MAX, None);
        assert_eq!(limit, None);
        assert_eq!(pass, None);
    }

    #[test]
    fn evaluate_check_value_zero_limit_u64_max_passes() {
        // Verifies the function handles the maximum possible limit correctly
        // for a zero value.
        let (limit, pass) = evaluate_check(0, Some(u64::MAX));
        assert_eq!(limit, Some(u64::MAX));
        assert_eq!(pass, Some(true));
    }

    // ── limit_for_metric zero-length / unknown input tests ──────────────

    #[test]
    fn limit_for_metric_cpu_instructions_returns_cpu_limit() {
        let config = FunctionConfig {
            args: vec![],
            cpu_limit: Some(5_000_000),
            read_limit: Some(1_000),
            write_limit: Some(500),
        };
        assert_eq!(
            limit_for_metric(&config, "CPU Instructions"),
            Some(5_000_000)
        );
    }

    #[test]
    fn limit_for_metric_read_bytes_returns_read_limit() {
        let config = FunctionConfig {
            args: vec![],
            cpu_limit: Some(5_000_000),
            read_limit: Some(1_000),
            write_limit: Some(500),
        };
        assert_eq!(limit_for_metric(&config, "Read Bytes"), Some(1_000));
    }

    #[test]
    fn limit_for_metric_write_bytes_returns_write_limit() {
        let config = FunctionConfig {
            args: vec![],
            cpu_limit: Some(5_000_000),
            read_limit: Some(1_000),
            write_limit: Some(500),
        };
        assert_eq!(limit_for_metric(&config, "Write Bytes"), Some(500));
    }

    #[test]
    fn limit_for_metric_zero_length_string_returns_none() {
        let config = FunctionConfig {
            args: vec![],
            cpu_limit: Some(5_000_000),
            read_limit: None,
            write_limit: None,
        };
        // An empty or unknown metric string should return None.
        assert_eq!(limit_for_metric(&config, ""), None);
    }

    #[test]
    fn limit_for_metric_unknown_metric_returns_none() {
        let config = FunctionConfig {
            args: vec![],
            cpu_limit: Some(5_000_000),
            read_limit: None,
            write_limit: None,
        };
        assert_eq!(limit_for_metric(&config, "WASM Bytes"), None);
        assert_eq!(limit_for_metric(&config, "Unknown Metric"), None);
    }

    #[test]
    fn limit_for_metric_none_limits_return_none() {
        let config = FunctionConfig {
            args: vec![],
            cpu_limit: None,
            read_limit: None,
            write_limit: None,
        };
        assert_eq!(limit_for_metric(&config, "CPU Instructions"), None);
        assert_eq!(limit_for_metric(&config, "Read Bytes"), None);
        assert_eq!(limit_for_metric(&config, "Write Bytes"), None);
    }

    #[test]
    fn limit_for_metric_default_config_returns_none() {
        let config = FunctionConfig::default();
        assert_eq!(limit_for_metric(&config, "CPU Instructions"), None);
        assert_eq!(limit_for_metric(&config, "Read Bytes"), None);
        assert_eq!(limit_for_metric(&config, "Write Bytes"), None);
    }

    // ── format_with_commas_and_units off-by-one tests ───────────────────

    #[test]
    fn formatter_zero_value_all_metrics() {
        assert_eq!(
            format_with_commas_and_units(0, "CPU Instructions"),
            "0 inst."
        );
        assert_eq!(format_with_commas_and_units(0, "Read Bytes"), "0 B");
        assert_eq!(format_with_commas_and_units(0, "Write Bytes"), "0 B");
    }

    #[test]
    fn formatter_one_value() {
        // Single digit just below the first comma boundary.
        assert_eq!(
            format_with_commas_and_units(1, "CPU Instructions"),
            "1 inst."
        );
        assert_eq!(format_with_commas_and_units(9, "Read Bytes"), "9 B");
    }

    #[test]
    fn formatter_exactly_ten_no_comma() {
        // Two digits still don't trigger comma insertion.
        assert_eq!(
            format_with_commas_and_units(10, "CPU Instructions"),
            "10 inst."
        );
    }

    #[test]
    fn formatter_exactly_one_hundred_no_comma() {
        assert_eq!(format_with_commas_and_units(100, "Read Bytes"), "100 B");
    }

    #[test]
    fn formatter_exactly_one_thousand_has_comma() {
        // The boundary where commas first appear.
        assert_eq!(
            format_with_commas_and_units(1_000, "CPU Instructions"),
            "1,000 inst."
        );
    }

    #[test]
    fn formatter_exactly_ten_thousand_has_comma() {
        assert_eq!(
            format_with_commas_and_units(10_000, "Write Bytes"),
            "10,000 B"
        );
    }

    #[test]
    fn formatter_exactly_one_hundred_thousand_has_comma() {
        assert_eq!(
            format_with_commas_and_units(100_000, "CPU Instructions"),
            "100,000 inst."
        );
    }

    #[test]
    fn formatter_one_below_million() {
        // 999,999 is just below the second comma insertion.
        assert_eq!(
            format_with_commas_and_units(999_999, "Read Bytes"),
            "999,999 B"
        );
    }

    #[test]
    fn formatter_one_above_million() {
        // 1,000,001 is just above the second comma insertion.
        assert_eq!(
            format_with_commas_and_units(1_000_001, "Write Bytes"),
            "1,000,001 B"
        );
    }

    #[test]
    fn formatter_exactly_one_billion_has_two_commas() {
        assert_eq!(
            format_with_commas_and_units(1_000_000_000, "CPU Instructions"),
            "1,000,000,000 inst."
        );
    }

    #[test]
    fn formatter_u64_max() {
        // u64::MAX = 18,446,744,073,709,551,615
        assert_eq!(
            format_with_commas_and_units(u64::MAX, "CPU Instructions"),
            "18,446,744,073,709,551,615 inst."
        );
    }

    #[test]
    fn formatter_unknown_metric_gets_inst_suffix() {
        // Any metric that doesn't contain "Bytes" gets " inst." suffix.
        assert_eq!(
            format_with_commas_and_units(500, "Some Unknown Metric"),
            "500 inst."
        );
    }

    // ── emit_check_failure_entries edge case tests ─────────────────────

    #[test]
    fn emit_check_failure_entries_emits_three_entries() {
        let mut reports = Vec::new();
        let config = FunctionConfig {
            args: vec![],
            cpu_limit: Some(5_000_000),
            read_limit: Some(1_000),
            write_limit: Some(500),
        };
        emit_check_failure_entries(&mut reports, "my-pkg", "do_work", &config);
        assert_eq!(reports.len(), 3);
    }

    #[test]
    fn emit_check_failure_entries_all_have_value_none_and_pass_false() {
        let mut reports = Vec::new();
        let config = FunctionConfig {
            args: vec![],
            cpu_limit: Some(5_000_000),
            read_limit: None,
            write_limit: Some(500),
        };
        emit_check_failure_entries(&mut reports, "my-pkg", "do_work", &config);
        for r in &reports {
            assert_eq!(r.package, "my-pkg");
            assert_eq!(r.function, "do_work");
            assert_eq!(r.value, None);
            assert_eq!(r.pass, Some(false));
        }
    }

    #[test]
    fn emit_check_failure_entries_handles_empty_function_name() {
        let mut reports = Vec::new();
        let config = FunctionConfig::default();
        emit_check_failure_entries(&mut reports, "pkg", "", &config);
        assert_eq!(reports.len(), 3);
        assert_eq!(reports[0].function, "");
    }

    #[test]
    fn emit_check_failure_entries_handles_empty_package_name() {
        let mut reports = Vec::new();
        let config = FunctionConfig::default();
        emit_check_failure_entries(&mut reports, "", "do_work", &config);
        assert_eq!(reports.len(), 3);
        assert_eq!(reports[0].package, "");
    }

    // ── CSV output zero-value and edge case tests ──────────────────────

    /// Helper that serializes reports via the same CSV logic used in `main`.
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
    fn csv_output_with_zero_value_indicates_zero_resource_usage() {
        let reports = vec![CostReport {
            package: "my-contract".to_string(),
            function: "do_work".to_string(),
            metric: "CPU Instructions",
            value: Some(0),
            limit: None,
            pass: None,
        }];
        let csv = reports_to_csv(&reports, false);
        assert!(csv.contains(",0\n") || csv.contains(",0\r\n"));
    }

    #[test]
    fn csv_output_with_check_zero_value_passes_zero_limit() {
        let reports = vec![CostReport {
            package: "my-contract".to_string(),
            function: "do_work".to_string(),
            metric: "CPU Instructions",
            value: Some(0),
            limit: Some(0),
            pass: Some(true),
        }];
        let csv = reports_to_csv(&reports, true);
        assert!(csv.contains(",0,0,true"));
    }

    #[test]
    fn csv_output_zero_reports_produces_header_only() {
        let csv = reports_to_csv(&[], false);
        assert_eq!(csv, "package,function,metric,value\n");
    }

    #[test]
    fn csv_output_check_zero_reports_produces_header_only() {
        let csv = reports_to_csv(&[], true);
        assert_eq!(csv, "package,function,metric,value,limit,pass\n");
    }

    // ── scaffold_init edge case tests ──────────────────────────────────
    //
    // These tests create a temporary working directory and change the
    // process CWD into it so that `scaffold_init`'s hard-coded
    // `Path::new("budget.toml")` does not clobber the real project file.

    /// Change into a newly-created temp directory and return the old CWD
    /// so the caller can restore it with [`restore_cwd`].
    fn isolate_temp_dir() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().expect("failed to create temp dir");
        let prev = std::env::current_dir().expect("failed to read current working directory");
        std::env::set_current_dir(tmp.path()).expect("failed to change into temp dir");
        (tmp, prev)
    }

    /// Restore the original CWD. The temp dir can then be dropped cleanly.
    fn restore_cwd(prev: &std::path::Path) {
        std::env::set_current_dir(prev).expect("failed to restore original working directory");
    }

    #[test]
    fn scaffold_init_creates_file_when_not_exists() {
        let (_tmp, prev) = isolate_temp_dir();

        let result = scaffold_init(false);
        assert!(result.is_ok());
        assert!(
            std::path::Path::new("budget.toml").exists(),
            "scaffold_init should create budget.toml"
        );

        // Verify the written content matches the template.
        let content = std::fs::read_to_string("budget.toml").unwrap();
        assert!(
            content.contains("Budget report configuration"),
            "written file should contain the template header"
        );

        restore_cwd(&prev);
    }

    #[test]
    fn scaffold_init_errors_when_exists_without_force() {
        let (_tmp, prev) = isolate_temp_dir();
        std::fs::write("budget.toml", "existing data").unwrap();

        let result = scaffold_init(false);
        assert!(result.is_err());
        let err = format!("{:#}", result.as_ref().unwrap_err());
        assert!(
            err.contains("already exists"),
            "error should mention that budget.toml already exists, got: {}",
            err
        );

        // File content must be untouched.
        assert_eq!(
            std::fs::read_to_string("budget.toml").unwrap(),
            "existing data"
        );

        restore_cwd(&prev);
    }

    #[test]
    fn scaffold_init_overwrites_with_force_flag() {
        let (_tmp, prev) = isolate_temp_dir();
        std::fs::write("budget.toml", "existing data").unwrap();

        let result = scaffold_init(true);
        assert!(result.is_ok());

        let content = std::fs::read_to_string("budget.toml").unwrap();
        assert!(
            content.contains("Budget report configuration"),
            "with --force, existing content should be overwritten with template"
        );

        restore_cwd(&prev);
    }

    // ── load_budget_toml edge case tests ───────────────────────────────

    #[test]
    fn load_budget_toml_empty_file_returns_default() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        // Write empty content — just create the file as empty.
        std::fs::write(tmp.path(), "").unwrap();

        let config =
            load_budget_toml(tmp.path()).expect("empty file should parse as default BudgetToml");
        assert!(config.network.is_none());
        assert!(config.source.is_none());
        assert!(config.functions.is_empty());
    }

    #[test]
    fn load_budget_toml_comments_only_returns_default() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        std::fs::write(tmp.path(), "# This is a comment\n# Another comment\n").unwrap();

        let config = load_budget_toml(tmp.path())
            .expect("comments-only file should parse as default BudgetToml");
        assert!(config.network.is_none());
        assert!(config.source.is_none());
        assert!(config.functions.is_empty());
    }

    #[test]
    fn load_budget_toml_all_limits_at_zero() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        let content = r#"
network = "testnet"
source = "alice"

[functions.do_work]
cpu_limit = 0
read_limit = 0
write_limit = 0
"#;
        std::fs::write(tmp.path(), content).unwrap();

        let config =
            load_budget_toml(tmp.path()).expect("should parse zero-valued limits successfully");
        assert_eq!(config.network.as_deref(), Some("testnet"));
        assert_eq!(config.source.as_deref(), Some("alice"));

        let func = config
            .functions
            .get("do_work")
            .expect("do_work function should be present");
        assert_eq!(
            func.cpu_limit,
            Some(0),
            "cpu_limit should parse 0 correctly"
        );
        assert_eq!(
            func.read_limit,
            Some(0),
            "read_limit should parse 0 correctly"
        );
        assert_eq!(
            func.write_limit,
            Some(0),
            "write_limit should parse 0 correctly"
        );
    }

    #[test]
    fn load_budget_toml_whitespace_only_returns_default() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        std::fs::write(tmp.path(), "   \n\t\n  \n").unwrap();

        let config = load_budget_toml(tmp.path())
            .expect("whitespace-only file should parse as default BudgetToml");
        assert!(config.network.is_none());
        assert!(config.source.is_none());
    }

    #[test]
    fn load_budget_toml_network_only_uses_default_for_rest() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        std::fs::write(tmp.path(), "network = \"futurenet\"\n").unwrap();

        let config =
            load_budget_toml(tmp.path()).expect("partial config should parse successfully");
        assert_eq!(config.network.as_deref(), Some("futurenet"));
        assert!(config.source.is_none());
        assert!(config.functions.is_empty());
    }

    // ── More evaluate_check boundary tests ────────────────────────────

    #[test]
    fn evaluate_check_value_u32_max_limit_u64_max_minus_one_passes() {
        // u32::MAX (4,294,967,295) is still well below u64::MAX - 1.
        let (limit, pass) = evaluate_check(u32::MAX, Some(u64::MAX - 1));
        assert_eq!(limit, Some(u64::MAX - 1));
        assert_eq!(pass, Some(true));
    }

    // ── More limit_for_metric edge case tests ──────────────────────────

    #[test]
    fn limit_for_metric_leading_whitespace_returns_none() {
        let config = FunctionConfig {
            args: vec![],
            cpu_limit: Some(5_000_000),
            read_limit: None,
            write_limit: None,
        };
        assert_eq!(limit_for_metric(&config, " CPU Instructions"), None);
        assert_eq!(limit_for_metric(&config, "  Read Bytes"), None);
    }

    #[test]
    fn limit_for_metric_trailing_whitespace_returns_none() {
        let config = FunctionConfig {
            args: vec![],
            cpu_limit: Some(5_000_000),
            read_limit: None,
            write_limit: None,
        };
        assert_eq!(limit_for_metric(&config, "CPU Instructions "), None);
        assert_eq!(limit_for_metric(&config, "Write Bytes\t"), None);
    }

    #[test]
    fn limit_for_metric_case_sensitive_check() {
        let config = FunctionConfig {
            args: vec![],
            cpu_limit: Some(5_000_000),
            read_limit: Some(1_000),
            write_limit: Some(500),
        };
        // The function should do exact case-sensitive matching.
        assert_eq!(limit_for_metric(&config, "cpu instructions"), None);
        assert_eq!(limit_for_metric(&config, "read bytes"), None);
        assert_eq!(limit_for_metric(&config, "write bytes"), None);
    }

    // ── More TransactionData::parse_json edge case tests ───────────────

    #[test]
    fn transaction_data_parse_negative_value_fails() {
        // The `Resources` struct uses `u64`, so negative values should fail.
        let json_str =
            r#"{"resources": {"instructions": -1, "disk_read_bytes": 0, "write_bytes": 0}}"#;
        let result = TransactionData::parse_json(json_str);
        assert!(result.is_err(), "negative value should fail to parse");
    }

    #[test]
    fn transaction_data_parse_null_value_fails() {
        let json_str =
            r#"{"resources": {"instructions": null, "disk_read_bytes": 0, "write_bytes": 0}}"#;
        let result = TransactionData::parse_json(json_str);
        assert!(
            result.is_err(),
            "null value for non-optional field should fail"
        );
    }

    // ── build_invoke_args additional edge cases ────────────────────────

    #[test]
    fn build_invoke_args_multiple_function_arguments() {
        let args = build_invoke_args(
            "C",
            "alice",
            "testnet",
            "transfer",
            &["--to".into(), "GBP".into(), "--amount".into(), "100".into()],
        );
        // Expected structure: [<11 standard args>, "--to", "GBP", "--amount", "100"]
        assert_eq!(args.len(), 15);
        assert_eq!(args[10], "transfer");
        assert_eq!(args[11], "--to");
        assert_eq!(args[12], "GBP");
        assert_eq!(args[13], "--amount");
        assert_eq!(args[14], "100");
    }

    #[test]
    fn build_invoke_args_empty_source_string() {
        let args = build_invoke_args("C", "", "testnet", "f", &[]);
        assert_eq!(args[5], ""); // source
    }

    #[test]
    fn build_invoke_args_empty_network_string() {
        let args = build_invoke_args("C", "alice", "", "f", &[]);
        assert_eq!(args[7], ""); // network
    }

    #[test]
    fn build_invoke_args_function_name_with_hyphens() {
        let args = build_invoke_args("C", "alice", "testnet", "do-something", &[]);
        assert_eq!(args.last(), Some(&"do-something".to_string()));
    }

    // ── build_rpc_payload additional edge cases ────────────────────────

    #[test]
    fn build_rpc_payload_json_structure_is_consistent() {
        // Verify the JSON structure regardless of XDR content.
        let payload = build_rpc_payload("test");
        assert_eq!(payload["id"], 1);
        assert_eq!(payload["jsonrpc"], "2.0");
        assert_eq!(payload["method"], "simulateTransaction");
        assert!(payload["params"].is_object());
        assert_eq!(payload["params"]["transaction"], "test");
    }

    #[test]
    fn build_rpc_payload_very_long_xdr() {
        // Simulate a realistic-length base64 XDR string.
        let long_xdr = "A".repeat(10_000);
        let payload = build_rpc_payload(&long_xdr);
        assert_eq!(
            payload["params"]["transaction"].as_str().unwrap().len(),
            10_000
        );
    }

    // ── emit_check_failure_entries additional edge cases ───────────────

    #[test]
    fn emit_check_failure_entries_mixed_limits_preserves_metric_order() {
        let mut reports = Vec::new();
        let config = FunctionConfig {
            args: vec![],
            cpu_limit: Some(5_000_000),
            read_limit: None,
            write_limit: Some(500),
        };
        emit_check_failure_entries(&mut reports, "pkg", "fn", &config);
        assert_eq!(reports.len(), 3);
        // Must be emitted in order: CPU, Read, Write.
        assert_eq!(reports[0].metric, "CPU Instructions");
        assert_eq!(reports[0].limit, Some(5_000_000));
        assert_eq!(reports[1].metric, "Read Bytes");
        assert_eq!(reports[1].limit, None);
        assert_eq!(reports[2].metric, "Write Bytes");
        assert_eq!(reports[2].limit, Some(500));
        // All should have pass = false.
        for r in &reports {
            assert_eq!(r.pass, Some(false));
        }
    }

    #[test]
    fn emit_check_failure_entries_all_limits_none() {
        let mut reports = Vec::new();
        let config = FunctionConfig::default();
        emit_check_failure_entries(&mut reports, "pkg", "fn", &config);
        assert_eq!(reports.len(), 3);
        for r in &reports {
            assert_eq!(r.limit, None);
            assert_eq!(r.pass, Some(false));
        }
    }

    // ── build_invoke_args zero-length edge cases ────────────────────────

    #[test]
    fn build_invoke_args_zero_arguments_empty_slice() {
        // Function with no arguments — already tested in main.rs tests, but
        // included here for the focused edge-case module.
        let args = build_invoke_args("CCONTRACT", "alice", "testnet", "ping", &[]);
        assert_eq!(args.len(), 11);
        assert_eq!(args.last(), Some(&"ping".to_string()));
    }

    #[test]
    fn build_invoke_args_single_argument_boundary() {
        let args = build_invoke_args("CCONTRACT", "alice", "testnet", "do_work", &["--n".into()]);
        // After "--", we expect: ["do_work", "--n"]
        assert_eq!(args[args.len() - 2], "do_work");
        assert_eq!(args[args.len() - 1], "--n");
    }

    #[test]
    fn build_invoke_args_zero_length_contract_id() {
        let args = build_invoke_args("", "alice", "testnet", "f", &[]);
        assert_eq!(args[3], ""); // contract ID
    }

    #[test]
    fn build_invoke_args_zero_length_function_name() {
        let args = build_invoke_args("C", "alice", "testnet", "", &[]);
        assert_eq!(args.last(), Some(&"".to_string()));
    }

    // ── build_rpc_payload zero-length edge cases ────────────────────────

    #[test]
    fn build_rpc_payload_zero_length_xdr() {
        let payload = build_rpc_payload("");
        assert_eq!(payload["jsonrpc"], "2.0");
        assert_eq!(payload["method"], "simulateTransaction");
        assert_eq!(payload["params"]["transaction"], "");
    }

    #[test]
    fn build_rpc_payload_single_byte_xdr() {
        let payload = build_rpc_payload("A");
        assert_eq!(payload["params"]["transaction"], "A");
    }

    // ── TransactionData parse_json zero-value edge cases ────────────────

    #[test]
    fn transaction_data_parse_all_zeros() {
        let json_str =
            r#"{"resources": {"instructions": 0, "disk_read_bytes": 0, "write_bytes": 0}}"#;
        let tx_data = TransactionData::parse_json(json_str).expect("Parsing should succeed");
        assert_eq!(tx_data.resources.instructions, 0);
        assert_eq!(tx_data.resources.disk_read_bytes, 0);
        assert_eq!(tx_data.resources.write_bytes, 0);
    }

    #[test]
    fn transaction_data_parse_zero_instructions_only() {
        let json_str =
            r#"{"resources": {"instructions": 0, "disk_read_bytes": 2048, "write_bytes": 4096}}"#;
        let tx_data = TransactionData::parse_json(json_str).expect("Parsing should succeed");
        assert_eq!(tx_data.resources.instructions, 0);
        assert_eq!(tx_data.resources.disk_read_bytes, 2048);
        assert_eq!(tx_data.resources.write_bytes, 4096);
    }

    #[test]
    fn transaction_data_parse_u64_max_values() {
        let max = u64::MAX;
        let json_str = format!(
            r#"{{"resources": {{"instructions": {}, "disk_read_bytes": {}, "write_bytes": {}}}}}"#,
            max, max, max
        );
        let tx_data =
            TransactionData::parse_json(&json_str).expect("Parsing should succeed at u64::MAX");
        assert_eq!(tx_data.resources.instructions, max);
        assert_eq!(tx_data.resources.disk_read_bytes, max);
        assert_eq!(tx_data.resources.write_bytes, max);
    }

    #[test]
    fn transaction_data_parse_empty_json_fails() {
        let result = TransactionData::parse_json("{}");
        assert!(result.is_err());
    }

    #[test]
    fn transaction_data_parse_empty_resources_fails() {
        let result = TransactionData::parse_json(r#"{"resources": {}}"#);
        assert!(result.is_err());
    }

    // ── BudgetToml default / empty edge cases ──────────────────────────

    #[test]
    fn budget_toml_default_is_empty() {
        let config = BudgetToml::default();
        assert!(config.network.is_none());
        assert!(config.source.is_none());
        assert!(config.functions.is_empty());
    }

    #[test]
    fn function_config_default_is_empty() {
        let config = FunctionConfig::default();
        assert!(config.args.is_empty());
        assert!(config.cpu_limit.is_none());
        assert!(config.read_limit.is_none());
        assert!(config.write_limit.is_none());
    }
}
