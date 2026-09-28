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
    use stellar_xdr::{Limits, SorobanTransactionData, WriteXdr};

    // ── evaluate_check off-by-one tests ─────────────────────────────────

    #[test]
    fn evaluate_check_value_two_limit_one_fails_off_by_one() {
        // Classic off-by-one: value exceeds inclusive limit by exactly 1.
        let (limit, pass) = evaluate_check(2, Some(1));
        assert_eq!(limit, Some(1));
        assert_eq!(pass, Some(false));
    }

    #[test]
    fn evaluate_check_value_one_limit_one_passes_inclusive() {
        let (limit, pass) = evaluate_check(1, Some(1));
        assert_eq!(limit, Some(1));
        assert_eq!(pass, Some(true));
    }

    #[test]
    fn evaluate_check_value_zero_limit_none_returns_none() {
        let (limit, pass) = evaluate_check(0, None);
        assert_eq!(limit, None);
        assert_eq!(pass, None);
    }

    #[test]
    fn evaluate_check_mid_range_one_below_passes() {
        let (limit, pass) = evaluate_check(49_999, Some(50_000));
        assert_eq!(limit, Some(50_000));
        assert_eq!(pass, Some(true));
    }

    #[test]
    fn evaluate_check_mid_range_exact_passes() {
        let (limit, pass) = evaluate_check(50_000, Some(50_000));
        assert_eq!(limit, Some(50_000));
        assert_eq!(pass, Some(true));
    }

    #[test]
    fn evaluate_check_mid_range_one_above_fails() {
        let (limit, pass) = evaluate_check(50_001, Some(50_000));
        assert_eq!(limit, Some(50_000));
        assert_eq!(pass, Some(false));
    }

    #[test]
    fn evaluate_check_value_u32_max_limit_exactly_u32_max_passes() {
        let (limit, pass) = evaluate_check(u32::MAX, Some(u64::from(u32::MAX)));
        assert_eq!(limit, Some(u64::from(u32::MAX)));
        assert_eq!(pass, Some(true));
    }

    #[test]
    fn evaluate_check_value_u32_max_limit_one_below_fails() {
        let (limit, pass) = evaluate_check(u32::MAX, Some(u64::from(u32::MAX) - 1));
        assert_eq!(limit, Some(u64::from(u32::MAX) - 1));
        assert_eq!(pass, Some(false));
    }

    #[test]
    fn evaluate_check_value_zero_limit_zero_passes() {
        let (limit, pass) = evaluate_check(0, Some(0));
        assert_eq!(limit, Some(0));
        assert_eq!(pass, Some(true));
    }

    #[test]
    fn evaluate_check_value_one_limit_zero_fails() {
        let (limit, pass) = evaluate_check(1, Some(0));
        assert_eq!(limit, Some(0));
        assert_eq!(pass, Some(false));
    }

    // ── limit_for_metric zero-length / boundary input tests ─────────────

    #[test]
    fn limit_for_metric_empty_string_returns_none() {
        let config = FunctionConfig {
            args: vec![],
            cpu_limit: Some(1),
            read_limit: Some(1),
            write_limit: Some(1),
            tolerance: None,
        };
        assert_eq!(limit_for_metric(&config, ""), None);
    }

    #[test]
    fn limit_for_metric_partial_prefix_returns_none() {
        let config = FunctionConfig {
            args: vec![],
            cpu_limit: Some(9),
            read_limit: Some(8),
            write_limit: Some(7),
            tolerance: None,
        };
        // Substring / prefix matches must not succeed — matching is exact.
        assert_eq!(limit_for_metric(&config, "CPU"), None);
        assert_eq!(limit_for_metric(&config, "Read"), None);
        assert_eq!(limit_for_metric(&config, "Write"), None);
        assert_eq!(limit_for_metric(&config, "Bytes"), None);
    }

    #[test]
    fn limit_for_metric_zero_valued_limits_are_returned() {
        let config = FunctionConfig {
            args: vec![],
            cpu_limit: Some(0),
            read_limit: Some(0),
            write_limit: Some(0),
            tolerance: None,
        };
        assert_eq!(limit_for_metric(&config, "CPU Instructions"), Some(0));
        assert_eq!(limit_for_metric(&config, "Read Bytes"), Some(0));
        assert_eq!(limit_for_metric(&config, "Write Bytes"), Some(0));
    }

    #[test]
    fn limit_for_metric_u64_max_limits_are_returned() {
        let config = FunctionConfig {
            args: vec![],
            cpu_limit: Some(u64::MAX),
            read_limit: Some(u64::MAX),
            write_limit: Some(u64::MAX),
            tolerance: None,
        };
        assert_eq!(
            limit_for_metric(&config, "CPU Instructions"),
            Some(u64::MAX)
        );
        assert_eq!(limit_for_metric(&config, "Read Bytes"), Some(u64::MAX));
        assert_eq!(limit_for_metric(&config, "Write Bytes"), Some(u64::MAX));
    }

    // ── format_with_commas_and_units off-by-one tests ───────────────────

    #[test]
    fn formatter_exactly_999_no_comma() {
        assert_eq!(
            format_with_commas_and_units(999, "CPU Instructions"),
            "999 inst."
        );
    }

    #[test]
    fn formatter_exactly_1000_inserts_comma() {
        assert_eq!(format_with_commas_and_units(1_000, "Read Bytes"), "1,000 B");
    }

    #[test]
    fn formatter_exactly_999_999_boundary() {
        assert_eq!(
            format_with_commas_and_units(999_999, "Write Bytes"),
            "999,999 B"
        );
    }

    #[test]
    fn formatter_exactly_1_000_000_boundary() {
        assert_eq!(
            format_with_commas_and_units(1_000_000, "CPU Instructions"),
            "1,000,000 inst."
        );
    }

    #[test]
    fn formatter_one_below_billion() {
        assert_eq!(
            format_with_commas_and_units(999_999_999, "CPU Instructions"),
            "999,999,999 inst."
        );
    }

    #[test]
    fn formatter_exactly_one_billion() {
        assert_eq!(
            format_with_commas_and_units(1_000_000_000, "Read Bytes"),
            "1,000,000,000 B"
        );
    }

    #[test]
    fn formatter_one_above_billion() {
        assert_eq!(
            format_with_commas_and_units(1_000_000_001, "Write Bytes"),
            "1,000,000,001 B"
        );
    }

    #[test]
    fn formatter_empty_metric_name_defaults_to_inst() {
        // Empty metric name does not contain "Bytes", so it gets " inst.".
        assert_eq!(format_with_commas_and_units(42, ""), "42 inst.");
    }

    #[test]
    fn formatter_zero_length_value_is_zero() {
        assert_eq!(format_with_commas_and_units(0, "Write Bytes"), "0 B");
        assert_eq!(
            format_with_commas_and_units(0, "CPU Instructions"),
            "0 inst."
        );
    }

    // ── emit_check_failure_entries edge case tests ─────────────────────

    #[test]
    fn emit_check_failure_entries_zero_length_names() {
        let mut reports = Vec::new();
        let config = FunctionConfig {
            args: vec![],
            cpu_limit: Some(0),
            read_limit: Some(0),
            write_limit: Some(0),
            tolerance: None,
        };
        emit_check_failure_entries(&mut reports, "", "", &config);
        assert_eq!(reports.len(), 3);
        assert_eq!(reports[0].package, "");
        assert_eq!(reports[0].function, "");
        assert_eq!(reports[0].limit, Some(0));
        assert_eq!(reports[0].pass, Some(false));
        assert_eq!(reports[0].value, None);
    }

    #[test]
    fn emit_check_failure_entries_appends_to_existing_reports() {
        let mut reports = vec![CostReport {
            package: "existing".to_string(),
            function: "fn".to_string(),
            metric: "CPU Instructions",
            value: Some(1),
            limit: None,
            pass: None,
        }];
        let config = FunctionConfig::default();
        emit_check_failure_entries(&mut reports, "pkg", "failed_fn", &config);
        // 1 existing + 3 failure stubs
        assert_eq!(reports.len(), 4);
        assert_eq!(reports[0].package, "existing");
        assert_eq!(reports[1].function, "failed_fn");
        assert_eq!(reports[2].function, "failed_fn");
        assert_eq!(reports[3].function, "failed_fn");
    }

    #[test]
    fn emit_check_failure_entries_preserves_metric_order() {
        let mut reports = Vec::new();
        let config = FunctionConfig {
            args: vec![],
            cpu_limit: Some(1),
            read_limit: Some(2),
            write_limit: Some(3),
            tolerance: None,
        };
        emit_check_failure_entries(&mut reports, "pkg", "fn", &config);
        assert_eq!(reports[0].metric, "CPU Instructions");
        assert_eq!(reports[0].limit, Some(1));
        assert_eq!(reports[1].metric, "Read Bytes");
        assert_eq!(reports[1].limit, Some(2));
        assert_eq!(reports[2].metric, "Write Bytes");
        assert_eq!(reports[2].limit, Some(3));
    }

    // ── scaffold_init edge case tests ──────────────────────────────────
    //
    // These tests mutate process CWD, so they must not run concurrently
    // with each other (or with edge_case_tests's scaffold tests).

    fn isolate_temp_dir() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().expect("failed to create temp dir");
        let prev = std::env::current_dir().expect("failed to read current working directory");
        std::env::set_current_dir(tmp.path()).expect("failed to change into temp dir");
        (tmp, prev)
    }

    fn restore_cwd(prev: &std::path::Path) {
        std::env::set_current_dir(prev).expect("failed to restore original working directory");
    }

    #[test]
    fn scaffold_init_edge_cases_are_deterministic() {
        let _guard = crate::TEST_CWD_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        // Creates file when missing (quiet).
        {
            let (_tmp, prev) = isolate_temp_dir();
            let result = scaffold_init(false, true);
            assert!(result.is_ok());
            assert!(
                std::path::Path::new("budget.toml").exists(),
                "scaffold_init should create budget.toml"
            );
            let content = std::fs::read_to_string("budget.toml").unwrap();
            assert!(content.contains("Budget report configuration"));
            restore_cwd(&prev);
        }

        // Errors when exists without --force (quiet).
        {
            let (_tmp, prev) = isolate_temp_dir();
            std::fs::write("budget.toml", "existing").unwrap();
            let result = scaffold_init(false, true);
            assert!(result.is_err());
            let err = format!("{:#}", result.as_ref().unwrap_err());
            assert!(
                err.contains("already exists"),
                "error should mention that budget.toml already exists, got: {}",
                err
            );
            assert_eq!(std::fs::read_to_string("budget.toml").unwrap(), "existing");
            restore_cwd(&prev);
        }

        // --force overwrites (quiet).
        {
            let (_tmp, prev) = isolate_temp_dir();
            std::fs::write("budget.toml", "stale").unwrap();
            let result = scaffold_init(true, true);
            assert!(result.is_ok());
            let content = std::fs::read_to_string("budget.toml").unwrap();
            assert!(content.contains("Budget report configuration"));
            assert!(!content.contains("stale"));
            restore_cwd(&prev);
        }

        // --force overwrites (non-quiet).
        {
            let (_tmp, prev) = isolate_temp_dir();
            std::fs::write("budget.toml", "stale").unwrap();
            let result = scaffold_init(true, false);
            assert!(result.is_ok());
            let content = std::fs::read_to_string("budget.toml").unwrap();
            assert!(content.contains("Budget report configuration"));
            restore_cwd(&prev);
        }
    }

    // ── load_budget_toml edge case tests ───────────────────────────────

    #[test]
    fn load_budget_toml_empty_args_array() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        let content = r#"
network = "testnet"
source = "alice"

[functions.ping]
args = []
"#;
        std::fs::write(tmp.path(), content).unwrap();

        let config = load_budget_toml(tmp.path()).expect("empty args array should parse");
        let func = config
            .functions
            .get("ping")
            .expect("ping function should be present");
        assert!(func.args.is_empty());
        assert!(func.cpu_limit.is_none());
    }

    #[test]
    fn load_budget_toml_empty_string_network_and_source() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        std::fs::write(tmp.path(), "network = \"\"\nsource = \"\"\n").unwrap();

        let config = load_budget_toml(tmp.path()).expect("empty strings should parse");
        assert_eq!(config.network.as_deref(), Some(""));
        assert_eq!(config.source.as_deref(), Some(""));
        assert!(config.functions.is_empty());
    }

    #[test]
    fn load_budget_toml_single_function_zero_limits() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        let content = r#"
[functions.edge]
cpu_limit = 0
read_limit = 0
write_limit = 0
"#;
        std::fs::write(tmp.path(), content).unwrap();

        let config = load_budget_toml(tmp.path()).expect("zero limits should parse");
        let func = &config.functions["edge"];
        assert_eq!(func.cpu_limit, Some(0));
        assert_eq!(func.read_limit, Some(0));
        assert_eq!(func.write_limit, Some(0));
    }

    #[test]
    fn load_budget_toml_hash_only_comment_returns_default() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        std::fs::write(tmp.path(), "#\n").unwrap();

        let config = load_budget_toml(tmp.path()).expect("hash-only comment should be default");
        assert!(config.network.is_none());
        assert!(config.source.is_none());
        assert!(config.functions.is_empty());
    }

    #[test]
    fn load_budget_toml_function_with_empty_string_arg() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        let content = r#"
[functions.weird]
args = [""]
"#;
        std::fs::write(tmp.path(), content).unwrap();

        let config = load_budget_toml(tmp.path()).expect("empty string arg should parse");
        let func = &config.functions["weird"];
        assert_eq!(
            func.args,
            vec![crate::arg_spec::ArgSpec::Raw(String::new())]
        );
    }

    // ── build_invoke_args zero-length / boundary tests ─────────────────

    #[test]
    fn build_invoke_args_all_empty_strings() {
        let args = build_invoke_args("", "", "", "", &[], None);
        assert_eq!(args.len(), 11);
        assert_eq!(args[3], ""); // contract id
        assert_eq!(args[5], ""); // source
        assert_eq!(args[7], ""); // network
        assert_eq!(args[10], ""); // function
    }

    #[test]
    fn build_invoke_args_empty_string_function_arg() {
        let args = build_invoke_args("C", "alice", "testnet", "f", &["".into()], None);
        assert_eq!(args.len(), 12);
        assert_eq!(args.last(), Some(&"".to_string()));
    }

    #[test]
    fn build_invoke_args_whitespace_only_function_arg() {
        let args = build_invoke_args("C", "alice", "testnet", "f", &["   ".into()], None);
        assert_eq!(args.last(), Some(&"   ".to_string()));
    }

    #[test]
    fn build_invoke_args_one_arg_boundary_length() {
        let args = build_invoke_args("CID", "src", "net", "fn", &["x".into()], None);
        // 11 base args + 1 function arg
        assert_eq!(args.len(), 12);
        assert_eq!(args[10], "fn");
        assert_eq!(args[11], "x");
    }

    // ── build_rpc_payload zero-length / boundary tests ─────────────────

    #[test]
    fn build_rpc_payload_empty_string() {
        let payload = build_rpc_payload("");
        assert_eq!(payload["jsonrpc"], "2.0");
        assert_eq!(payload["id"], 1);
        assert_eq!(payload["method"], "simulateTransaction");
        assert_eq!(payload["params"]["transaction"], "");
    }

    #[test]
    fn build_rpc_payload_single_character() {
        let payload = build_rpc_payload("Z");
        assert_eq!(payload["params"]["transaction"], "Z");
    }

    #[test]
    fn build_rpc_payload_whitespace_xdr() {
        let payload = build_rpc_payload(" \t\n");
        assert_eq!(payload["params"]["transaction"], " \t\n");
    }

    // ── TransactionData parse_json zero / boundary tests ───────────────

    #[test]
    fn transaction_data_parse_all_zeros() {
        let json_str =
            r#"{"resources": {"instructions": 0, "disk_read_bytes": 0, "write_bytes": 0}}"#;
        let tx_data = TransactionData::parse_json(json_str).expect("zeros should parse");
        assert_eq!(tx_data.resources.instructions, 0);
        assert_eq!(tx_data.resources.disk_read_bytes, 0);
        assert_eq!(tx_data.resources.write_bytes, 0);
    }

    #[test]
    fn transaction_data_parse_one_above_zero() {
        let json_str =
            r#"{"resources": {"instructions": 1, "disk_read_bytes": 1, "write_bytes": 1}}"#;
        let tx_data = TransactionData::parse_json(json_str).expect("ones should parse");
        assert_eq!(tx_data.resources.instructions, 1);
        assert_eq!(tx_data.resources.disk_read_bytes, 1);
        assert_eq!(tx_data.resources.write_bytes, 1);
    }

    #[test]
    fn transaction_data_parse_empty_string_fails() {
        let result = TransactionData::parse_json("");
        assert!(result.is_err(), "empty JSON string should fail");
    }

    #[test]
    fn transaction_data_parse_zero_length_object_fails() {
        let result = TransactionData::parse_json("{}");
        assert!(result.is_err(), "empty object should fail");
    }

    #[test]
    fn transaction_data_parse_u64_max_minus_one() {
        let almost = u64::MAX - 1;
        let json_str = format!(
            r#"{{"resources": {{"instructions": {}, "disk_read_bytes": {}, "write_bytes": {}}}}}"#,
            almost, almost, almost
        );
        let tx_data = TransactionData::parse_json(&json_str).expect("u64::MAX-1 should parse");
        assert_eq!(tx_data.resources.instructions, almost);
        assert_eq!(tx_data.resources.disk_read_bytes, almost);
        assert_eq!(tx_data.resources.write_bytes, almost);
    }

    // ── extract_metrics zero-value / boundary tests ────────────────────

    fn make_tx_data(instructions: u32, read_bytes: u32, write_bytes: u32) -> String {
        use stellar_xdr::{LedgerFootprint, SorobanTransactionDataExt, VecM};
        let tx_data = SorobanTransactionData {
            ext: SorobanTransactionDataExt::V0,
            resources: stellar_xdr::SorobanResources {
                footprint: LedgerFootprint {
                    read_only: VecM::default(),
                    read_write: VecM::default(),
                },
                instructions,
                disk_read_bytes: read_bytes,
                write_bytes,
            },
            resource_fee: 0,
        };
        tx_data
            .to_xdr_base64(Limits::none())
            .expect("failed to encode fixture SorobanTransactionData")
    }

    #[test]
    fn extract_metrics_all_zeros() {
        let b64 = make_tx_data(0, 0, 0);
        let rpc_json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": { "transactionData": b64 }
        });
        let (instructions, read_bytes, write_bytes) =
            extract_metrics(&rpc_json).expect("zero metrics should extract");
        assert_eq!(instructions, 0);
        assert_eq!(read_bytes, 0);
        assert_eq!(write_bytes, 0);
    }

    #[test]
    fn extract_metrics_one_above_zero() {
        let b64 = make_tx_data(1, 1, 1);
        let rpc_json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": { "transactionData": b64 }
        });
        let (instructions, read_bytes, write_bytes) =
            extract_metrics(&rpc_json).expect("unit metrics should extract");
        assert_eq!(instructions, 1);
        assert_eq!(read_bytes, 1);
        assert_eq!(write_bytes, 1);
    }

    #[test]
    fn extract_metrics_empty_transaction_data_string_fails() {
        let rpc_json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": { "transactionData": "" }
        });
        let result = extract_metrics(&rpc_json);
        assert!(
            result.is_err(),
            "empty transactionData string should fail to decode"
        );
    }

    #[test]
    fn extract_metrics_missing_result_object_fails() {
        let rpc_json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1
        });
        let result = extract_metrics(&rpc_json);
        assert!(result.is_err());
    }

    // ── CSV output zero-length / boundary tests ────────────────────────

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
    fn csv_output_empty_package_and_function_names() {
        let reports = vec![CostReport {
            package: "".to_string(),
            function: "".to_string(),
            metric: "CPU Instructions",
            value: Some(0),
            limit: None,
            pass: None,
        }];
        let csv = reports_to_csv(&reports, false);
        assert!(csv.contains(",,CPU Instructions,0\n") || csv.contains(",,CPU Instructions,0\r\n"));
    }

    #[test]
    fn csv_output_check_mode_zero_value_zero_limit_pass() {
        let reports = vec![CostReport {
            package: "pkg".to_string(),
            function: "fn".to_string(),
            metric: "Read Bytes",
            value: Some(0),
            limit: Some(0),
            pass: Some(true),
        }];
        let csv = reports_to_csv(&reports, true);
        assert!(csv.contains("pkg,fn,Read Bytes,0,0,true"));
    }

    #[test]
    fn csv_output_check_mode_value_one_above_zero_limit_fails() {
        let reports = vec![CostReport {
            package: "pkg".to_string(),
            function: "fn".to_string(),
            metric: "Write Bytes",
            value: Some(1),
            limit: Some(0),
            pass: Some(false),
        }];
        let csv = reports_to_csv(&reports, true);
        assert!(csv.contains("pkg,fn,Write Bytes,1,0,false"));
    }

    #[test]
    fn csv_output_without_check_skips_none_values_only_header() {
        let reports = vec![CostReport {
            package: "pkg".to_string(),
            function: "fn".to_string(),
            metric: "CPU Instructions",
            value: None,
            limit: Some(1),
            pass: Some(false),
        }];
        let csv = reports_to_csv(&reports, false);
        assert_eq!(csv, "package,function,metric,value\n");
    }

// ── resolve_retry_config edge case tests ──────────────────────

#[test]
fn resolve_retry_config_defaults_when_no_args_or_toml() {
    let config = crate::resolve_retry_config(None, None, None).unwrap();
    assert_eq!(config.max_attempts, crate::MAX_DEPLOY_ATTEMPTS);
    assert_eq!(config.initial_backoff.as_secs(), crate::INITIAL_RETRY_DELAY_SECS);
}

#[test]
fn resolve_retry_config_cli_overrides_toml() {
    let config = crate::resolve_retry_config(
        Some(2),
        Some(10),
        None,
    ).unwrap();
    assert_eq!(config.max_attempts, 2);
    assert_eq!(config.initial_backoff.as_secs(), 10);
}

#[test]
fn resolve_retry_config_toml_overrides_defaults() {
    use crate::RetryToml;
    let toml = RetryToml {
        max_attempts: Some(5),
        initial_backoff_secs: Some(20),
    };
    let config = crate::resolve_retry_config(None, None, Some(toml)).unwrap();
    assert_eq!(config.max_attempts, 5);
    assert_eq!(config.initial_backoff.as_secs(), 20);
}

#[test]
fn resolve_retry_config_zero_attempts_is_error() {
    let result = crate::resolve_retry_config(Some(0), None, None);
    assert!(result.is_err());
    let err = format!("{:#}", result.unwrap_err());
    assert!(err.contains("max_attempts must be at least 1"), "got: {err}");
}

#[test]
fn resolve_retry_config_one_attempt_disables_retry() {
    let config = crate::resolve_retry_config(Some(1), None, None).unwrap();
    assert_eq!(config.max_attempts, 1);
    assert!(config.disabled());
}

// ── resolve_tolerance edge case tests ─────────────────────────

#[test]
fn resolve_tolerance_uses_cli_override() {
    use crate::BudgetToml;
    let config = BudgetToml::default();
    let tolerance = crate::resolve_tolerance(Some("0.15"), &config).unwrap();
    assert_eq!(tolerance.value, 0.15);
}

#[test]
fn resolve_tolerance_uses_toml_when_no_cli() {
    use crate::BudgetToml;
    let mut config = BudgetToml::default();
    config.tolerance = Some(0.10);
    let tolerance = crate::resolve_tolerance(None, &config).unwrap();
    assert_eq!(tolerance.value, 0.10);
}

#[test]
fn resolve_tolerance_defaults_when_no_cli_or_toml() {
    use crate::BudgetToml;
    let config = BudgetToml::default();
    let tolerance = crate::resolve_tolerance(None, &config).unwrap();
    assert_eq!(tolerance.value, 0.10);
}

// ── is_transient_error edge case tests ────────────────────────

#[test]
fn is_transient_error_recognizes_rate_limit() {
    assert!(crate::is_transient_error("rate limit exceeded"));
    assert!(crate::is_transient_error("429 Too Many Requests"));
}

#[test]
fn is_transient_error_recognizes_connection_errors() {
    assert!(crate::is_transient_error("connection refused"));
    assert!(crate::is_transient_error("timed out"));
}

#[test]
fn is_transient_error_recognizes_server_errors() {
    assert!(crate::is_transient_error("503 Service Unavailable"));
    assert!(crate::is_transient_error("502 Bad Gateway"));
}

#[test]
fn is_transient_error_rejects_permanent_errors() {
    assert!(!crate::is_transient_error("contract not found"));
    assert!(!crate::is_transient_error("malformed XDR"));
}

#[test]
fn is_transient_error_case_insensitive() {
    assert!(crate::is_transient_error("RATE-LIMIT"));
    assert!(crate::is_transient_error("TIMEOUT"));
}

// ── classify_outcome edge case tests ──────────────────────────

#[test]
fn classify_outcome_returns_success_when_all_pass() {
    let code = crate::classify_outcome(false, false, false);
    assert_eq!(code, crate::EXIT_SUCCESS);
}

#[test]
fn classify_outcome_returns_regression_when_present() {
    let code = crate::classify_outcome(true, false, false);
    assert_eq!(code, crate::EXIT_REGRESSION);
}

#[test]
fn classify_outcome_returns_budget_exceeded_when_present() {
    let code = crate::classify_outcome(false, true, false);
    assert_eq!(code, crate::EXIT_BUDGET_EXCEEDED);
}

#[test]
fn classify_outcome_returns_network_failure_when_present() {
    let code = crate::classify_outcome(false, false, true);
    assert_eq!(code, crate::EXIT_NETWORK_FAILURE);
}

// ── resolve_toml_config edge case tests ───────────────────────

#[test]
fn load_budget_toml_missing_file_returns_error() {
    let result = crate::load_budget_toml("nonexistent_file.toml");
    assert!(result.is_err());
}

#[test]
fn load_budget_toml_with_retry_section_overrides_defaults() {
    let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
    let content = r#"
[retry]
max_attempts = 5
initial_backoff_secs = 10
"#;
    std::fs::write(tmp.path(), content).unwrap();
    let config = crate::load_budget_toml(tmp.path()).expect("retry section should parse");
    assert_eq!(config.retry.as_ref().unwrap().max_attempts, Some(5));
    assert_eq!(config.retry.as_ref().unwrap().initial_backoff_secs, Some(10));
}

// ── extract_metrics u64 boundary tests ────────────────────────

#[test]
fn extract_metrics_u64_max_values() {
    use stellar_xdr::{LedgerFootprint, SorobanTransactionDataExt, VecM};
    let tx_data = stellar_xdr::SorobanTransactionData {
        ext: stellar_xdr::SorobanTransactionDataExt::V0,
        resources: stellar_xdr::SorobanResources {
            footprint: LedgerFootprint {
                read_only: VecM::default(),
                read_write: VecM::default(),
            },
            instructions: u32::MAX,
            disk_read_bytes: u32::MAX,
            write_bytes: u32::MAX,
        },
        resource_fee: 0,
    };
    let b64 = tx_data.to_xdr_base64(Limits::none()).unwrap();
    let rpc_json = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": { "transactionData": b64 }
    });
    let (instructions, read_bytes, write_bytes) =
        crate::extract_metrics(&rpc_json).expect("u32::MAX metrics should extract");
    assert_eq!(instructions, u32::MAX);
    assert_eq!(read_bytes, u32::MAX);
    assert_eq!(write_bytes, u32::MAX);
}

// ── format_with_commas_and_units large value tests ────────────

#[test]
fn formatter_very_large_values() {
    assert_eq!(
        format_with_commas_and_units(u64::MAX, "CPU Instructions"),
        format!("{},018,446,744,073,709,551,615 inst.", u64::MAX / 1_000_000_000 * 1_000_000_000)
    );
}

// ── build_invoke_args edge case tests ────────────────────────

#[test]
fn build_invoke_args_empty_contract_id() {
    let args = crate::build_invoke_args("", "src", "testnet", "fn", &[], None);
    assert_eq!(args[3], ""); // contract id is empty
}

#[test]
fn build_invoke_args_with_rpc_override_has_extra_args() {
    let args = crate::build_invoke_args("C", "src", "testnet", "fn", &[], Some(("http://rpc", "pass")));
    // 7 base + 4 override = 11 args, plus function and args
    assert!(args.len() >= 11);
    assert_eq!(args[7], "--rpc-url");
    assert_eq!(args[8], "http://rpc");
    assert_eq!(args[9], "--network-passphrase");
    assert_eq!(args[10], "pass");
}
}
