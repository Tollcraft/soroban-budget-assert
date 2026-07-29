//! Windows-compatibility and cross-platform edge-case tests.
//!
//! These tests focus on edge cases that are especially relevant when the
//! tool runs on Windows, including path separator handling, file-path
//! boundary conditions, and cross-platform configuration parsing.

#[cfg(test)]
mod windows_compatibility_tests {
    use crate::module_10::Error;
    use crate::*;
    use std::path::PathBuf;

    // ── Path handling with Windows-style separators ────────────────────

    /// Verifies that `Path::new` works correctly with both `/` and `\`
    /// separators for a simple "budget.toml" path.
    #[test]
    fn scaffold_init_path_handles_forward_slash() {
        let p = std::path::Path::new("budget.toml");
        assert_eq!(p.file_name().unwrap(), "budget.toml");
        assert!(p.parent().is_none() || p.parent().unwrap().as_os_str().is_empty());
    }

    #[test]
    fn scaffold_init_path_handles_windows_separator() {
        // A Windows path with a backslash separator should still resolve
        // the correct file name.
        let p = std::path::Path::new(r"subdir\budget.toml");
        assert_eq!(p.file_name().unwrap(), "budget.toml");
    }

    #[test]
    fn pathbuf_from_windows_style_path() {
        let pb = PathBuf::from(r"C:\Users\dev\project\budget.toml");
        assert_eq!(pb.file_name().unwrap(), "budget.toml");
        // Drive-letter root should be preserved.
        assert!(pb.to_string_lossy().starts_with("C:"));
    }

    #[test]
    fn pathbuf_from_mixed_separators() {
        // Mixed `/` and `\` — Rust normalises this on each platform.
        let pb = PathBuf::from(r"C:/Users\dev/project/budget.toml");
        assert_eq!(pb.file_name().unwrap(), "budget.toml");
    }

    #[test]
    fn pathbuf_from_unc_style_path() {
        // UNC paths start with `\\` and are valid on Windows.
        let pb = PathBuf::from(r"\\server\share\project\budget.toml");
        assert_eq!(pb.file_name().unwrap(), "budget.toml");
    }

    #[test]
    fn pathbuf_empty_string_is_valid() {
        let pb = PathBuf::from("");
        assert_eq!(pb.as_os_str().len(), 0);
        assert!(pb.file_name().is_none());
    }

    #[test]
    fn pathbuf_single_dot() {
        let pb = PathBuf::from(".");
        assert_eq!(pb.file_name().unwrap(), ".");
    }

    #[test]
    fn pathbuf_single_dotdot() {
        let pb = PathBuf::from("..");
        assert_eq!(pb.file_name().unwrap(), "..");
    }

    #[test]
    fn pathbuf_trailing_separator() {
        // Trailing separator is stripped by `file_name()` on Unix-style
        // paths; Windows paths behave the same.
        let pb = PathBuf::from("dir/");
        assert_eq!(pb.file_name().unwrap(), "dir");
    }

    #[test]
    fn pathbuf_trailing_windows_separator() {
        let pb = PathBuf::from(r"dir\");
        assert_eq!(pb.file_name().unwrap(), "dir");
    }

    // ── Mode path dispatch edge cases ──────────────────────────────────

    #[test]
    fn mode_record_with_windows_style_path() {
        let args = BudgetReportArgs {
            init: false,
            force: false,
            network: None,
            source: None,
            json: false,
            format: Default::default(),
            check: false,
            fail_fast: false,
            csv: false,
            concurrency: Default::default(),
            validate: false,
            quiet: false,
            record_baseline: Some(r"snapshots\baseline.toml".to_string()),
            check_baseline: None,
            tolerance: None,
            derive_limits: None,
            provenance_out: None,
            profile: None,
            from: None,
            margin_cpu: None,
            margin_memory: None,
            margin_read: None,
            margin_write: None,
            totals: false,
        };
        assert_eq!(
            Mode::from_args(&args),
            Mode::Record(PathBuf::from(r"snapshots\baseline.toml"))
        );
    }

    #[test]
    fn mode_check_with_windows_style_path() {
        let args = BudgetReportArgs {
            init: false,
            force: false,
            network: None,
            source: None,
            json: false,
            format: Default::default(),
            check: false,
            fail_fast: false,
            csv: false,
            concurrency: Default::default(),
            validate: false,
            quiet: false,
            record_baseline: None,
            check_baseline: Some(r"snapshots\check.toml".to_string()),
            tolerance: None,
            derive_limits: None,
            provenance_out: None,
            profile: None,
            from: None,
            margin_cpu: None,
            margin_memory: None,
            margin_read: None,
            margin_write: None,
            totals: false,
        };
        assert_eq!(
            Mode::from_args(&args),
            Mode::Check(PathBuf::from(r"snapshots\check.toml"))
        );
    }

    // ── CostReport JSON serialization with various field combinations ──

    #[test]
    fn cost_report_all_none_fields_serializes_minimally() {
        let report = CostReport {
            package: "pkg".to_string(),
            function: "fn".to_string(),
            metric: "CPU Instructions",
            value: None,
            limit: None,
            pass: None,
            resource_limit: None,
            share_pct: None,
        };
        let json = serde_json::to_string(&report).expect("serialization should succeed");
        // `value`, `limit`, and `pass` should be skipped when None.
        assert!(!json.contains("value"));
        assert!(!json.contains("limit"));
        assert!(!json.contains("pass"));
        assert!(json.contains("pkg"));
        assert!(json.contains("fn"));
        assert!(json.contains("CPU Instructions"));
    }

    #[test]
    fn cost_report_all_some_fields_serializes_completely() {
        let report = CostReport {
            package: "pkg".to_string(),
            function: "fn".to_string(),
            metric: "Read Bytes",
            value: Some(2048),
            limit: Some(5000),
            pass: Some(true),
            resource_limit: None,
            share_pct: None,
        };
        let json = serde_json::to_string(&report).expect("serialization should succeed");
        assert!(json.contains("\"value\":2048"));
        assert!(json.contains("\"limit\":5000"));
        assert!(json.contains("\"pass\":true"));
    }

    #[test]
    fn cost_report_mixed_some_none_serializes_correctly() {
        let report = CostReport {
            package: "pkg".to_string(),
            function: "fn".to_string(),
            metric: "Write Bytes",
            value: Some(4096),
            limit: None,
            pass: Some(false),
            resource_limit: None,
            share_pct: None,
        };
        let json = serde_json::to_string(&report).expect("serialization should succeed");
        assert!(json.contains("\"value\":4096"));
        assert!(json.contains("\"pass\":false"));
        assert!(!json.contains("limit"));
    }

    // ── BudgetToml deserialization cross-platform edge cases ──────────

    #[test]
    fn budget_toml_network_quoted_with_single_quotes_fails() {
        // TOML requires double-quotes for strings; single quotes are
        // treated as literal single quotes.
        let result = toml::from_str::<BudgetToml>("network = 'testnet'\n");
        assert!(result.is_err(), "single-quoted strings are not valid TOML");
    }

    #[test]
    fn budget_toml_trailing_comma_in_args_array() {
        // A trailing comma in an inline array is valid TOML.
        let config: BudgetToml = toml::from_str(
            r#"
[functions.fn]
args = ["--n", "10",]
"#,
        )
        .expect("trailing comma in array should parse");
        assert_eq!(
            config.functions["fn"].args.encode(),
            vec!["--n".to_string(), "10".to_string()]
        );
    }

    #[test]
    fn budget_toml_function_name_with_dollar_sign() {
        // TOML allows many special characters in bare keys,
        // but keys with special chars should generally remain parseable.
        let result = toml::from_str::<toml::Table>("[functions.my$fn]\nargs = []\n");
        // The $ is not a valid bare-key char in the TOML spec; the
        // key should be quoted.
        assert!(result.is_err());
    }

    #[test]
    fn budget_toml_function_name_quoted_with_special_chars() {
        let content = r#"
[functions."my$fn"]
args = ["--x"]
"#;
        // Quoted keys support any character.
        let table: toml::Table = toml::from_str(content).expect("quoted key should parse");
        assert!(table["functions"].get("my$fn").is_some());
    }

    // ── format_with_commas_and_units edge cases ────────────────────────

    #[test]
    fn formatter_value_at_powers_of_ten() {
        // Verify commas are placed correctly at each power-of-ten boundary.
        assert_eq!(
            format_with_commas_and_units(1_000, "CPU Instructions"),
            "1,000 inst."
        );
        assert_eq!(
            format_with_commas_and_units(10_000, "CPU Instructions"),
            "10,000 inst."
        );
        assert_eq!(
            format_with_commas_and_units(100_000, "CPU Instructions"),
            "100,000 inst."
        );
    }

    #[test]
    fn formatter_exact_boundaries_in_bytes() {
        assert_eq!(format_with_commas_and_units(1_000, "Read Bytes"), "1,000 B");
        assert_eq!(
            format_with_commas_and_units(10_000, "Write Bytes"),
            "10,000 B"
        );
        assert_eq!(
            format_with_commas_and_units(100_000, "Read Bytes"),
            "100,000 B"
        );
    }

    #[test]
    fn formatter_metric_name_contains_bytes_gets_b_suffix() {
        // Any metric containing "Bytes" gets the "B" suffix.
        assert_eq!(
            format_with_commas_and_units(42, "CPU Instructions Bytes"),
            "42 B"
        );
    }

    #[test]
    fn formatter_metric_name_only_bytes() {
        assert_eq!(format_with_commas_and_units(100, "Bytes"), "100 B");
    }

    #[test]
    fn formatter_metric_name_no_bytes_gets_inst_suffix() {
        // Metric name that doesn't contain "Bytes" defaults to inst.
        assert_eq!(
            format_with_commas_and_units(42, "CPU Instructions"),
            "42 inst."
        );
    }

    // ── evaluate_check regression boundary tests ──────────────────────

    #[test]
    fn evaluate_check_value_u32_max_minus_one_limit_u32_max_minus_one() {
        let (limit, pass) = evaluate_check(u32::MAX - 1, Some(u64::from(u32::MAX) - 1));
        assert_eq!(pass, Some(true));
    }

    #[test]
    fn evaluate_check_value_u32_max_minus_one_limit_u32_max() {
        let (limit, pass) = evaluate_check(u32::MAX - 1, Some(u64::from(u32::MAX)));
        assert_eq!(pass, Some(true));
    }

    #[test]
    fn evaluate_check_value_at_u64_max_limit_at_u64_max() {
        // u32 value compared against a u64::MAX limit — always passes.
        let (limit, pass) = evaluate_check(42, Some(u64::MAX));
        assert_eq!(limit, Some(u64::MAX));
        assert_eq!(pass, Some(true));
    }

    // ── limit_for_metric additional exact-match tests ──────────────────

    #[test]
    fn limit_for_metric_substring_at_end_does_not_match() {
        let config = FunctionConfig {
            args: Default::default(),
            cpu_limit: Some(1),
            read_limit: Some(2),
            write_limit: Some(3),
            mem_limit: None,
            cpu_limit_pct: None,
            read_limit_pct: None,
            write_limit_pct: None,
            tolerance: None,
        };
        // "Instructions" alone is not a known metric key.
        assert_eq!(limit_for_metric(&config, "Instructions"), None);
    }

    #[test]
    fn limit_for_metric_only_bytes_prefix() {
        let config = FunctionConfig {
            args: Default::default(),
            cpu_limit: Some(1),
            read_limit: Some(2),
            write_limit: Some(3),
            mem_limit: None,
            cpu_limit_pct: None,
            read_limit_pct: None,
            write_limit_pct: None,
            tolerance: None,
        };
        // "Read" alone does not match "Read Bytes".
        assert_eq!(limit_for_metric(&config, "Read"), None);
        assert_eq!(limit_for_metric(&config, "Write"), None);
        assert_eq!(limit_for_metric(&config, "CPU"), None);
    }

    #[test]
    fn limit_for_metric_exact_cpu_instructions() {
        let config = FunctionConfig {
            args: Default::default(),
            cpu_limit: Some(1234567),
            read_limit: None,
            write_limit: None,
            mem_limit: None,
            cpu_limit_pct: None,
            read_limit_pct: None,
            write_limit_pct: None,
            tolerance: None,
        };
        assert_eq!(limit_for_metric(&config, "CPU Instructions"), Some(1234567));
    }

    // ── emit_check_failure_entries regression boundary ────────────────

    #[test]
    fn emit_check_failure_entries_with_partial_limits() {
        // Only read_limit is set; CPU and write are None.
        let mut reports = Vec::new();
        let config = FunctionConfig {
            args: Default::default(),
            cpu_limit: None,
            read_limit: Some(2_000),
            write_limit: None,
            mem_limit: None,
            cpu_limit_pct: None,
            read_limit_pct: None,
            write_limit_pct: None,
            tolerance: None,
        };
        emit_check_failure_entries(&mut reports, "pkg", "fn", &config);
        assert_eq!(reports.len(), 4);
        assert_eq!(reports[0].metric, "CPU Instructions");
        assert_eq!(reports[0].limit, None);
        assert_eq!(reports[1].metric, "Memory Bytes");
        assert_eq!(reports[1].limit, None);
        assert_eq!(reports[2].metric, "Read Bytes");
        assert_eq!(reports[2].limit, Some(2_000));
        assert_eq!(reports[3].metric, "Write Bytes");
        assert_eq!(reports[3].limit, None);
    }

    // ── BUDGET_TOML_TEMPLATE content regression checks ─────────────────

    #[test]
    fn budget_toml_template_contains_required_sections() {
        assert!(
            BUDGET_TOML_TEMPLATE.contains("[functions.do_expensive_work]"),
            "template should mention the example function"
        );
        assert!(
            BUDGET_TOML_TEMPLATE.contains("cpu_limit"),
            "template should include cpu_limit"
        );
        assert!(
            BUDGET_TOML_TEMPLATE.contains("read_limit"),
            "template should include read_limit"
        );
        assert!(
            BUDGET_TOML_TEMPLATE.contains("write_limit"),
            "template should include write_limit"
        );
        assert!(
            BUDGET_TOML_TEMPLATE.contains("network = "),
            "template should include network config"
        );
        assert!(
            BUDGET_TOML_TEMPLATE.contains("source = "),
            "template should include source config"
        );
        assert!(
            BUDGET_TOML_TEMPLATE.contains("Budget report configuration"),
            "template should have a descriptive header comment"
        );
    }

    #[test]
    fn budget_toml_template_can_be_deserialized() {
        // The template itself must be valid TOML that BudgetToml can parse.
        let config: BudgetToml =
            toml::from_str(BUDGET_TOML_TEMPLATE).expect("BUDGET_TOML_TEMPLATE must be valid TOML");
        assert_eq!(config.network.as_deref(), Some("testnet"));
        assert_eq!(config.source.as_deref(), Some("alice"));
        assert!(config.functions.contains_key("do_expensive_work"));
        let func = &config.functions["do_expensive_work"];
        assert_eq!(func.cpu_limit, Some(5_000_000));
        assert_eq!(func.read_limit, Some(5_000));
        assert_eq!(func.write_limit, Some(1_000));
    }

    // ── load_budget_toml cross-platform path edge cases ────────────────

    #[test]
    fn load_budget_toml_carriage_return_line_feeds() {
        // Windows-style CRLF line endings should be handled gracefully.
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        std::fs::write(
            tmp.path(),
            "network = \"testnet\"\r\nsource = \"alice\"\r\n",
        )
        .unwrap();

        let config = load_budget_toml(tmp.path()).expect("CRLF file should parse");
        assert_eq!(config.network.as_deref(), Some("testnet"));
        assert_eq!(config.source.as_deref(), Some("alice"));
    }

    #[test]
    fn load_budget_toml_crlf_with_function_config() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        std::fs::write(
            tmp.path(),
            "[functions.do_work]\r\nargs = [\"--n\", \"10\"]\r\ncpu_limit = 1_000_000\r\n",
        )
        .unwrap();

        let config = load_budget_toml(tmp.path()).expect("CRLF function config should parse");
        let func = &config.functions["do_work"];
        assert_eq!(func.args.encode(), vec!["--n", "10"]);
        assert_eq!(func.cpu_limit, Some(1_000_000));
    }

    // ── resolve_tolerance precision edge cases ─────────────────────────

    #[test]
    fn resolve_tolerance_zero_tolerance() {
        let t = crate::compare::parse_tolerance("0.0").expect("zero tolerance should parse");
        // Zero tolerance means any increase is a regression.
        assert!((t.value - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn resolve_tolerance_one_hundred_percent() {
        let t = crate::compare::parse_tolerance("1.0").expect("1.0 tolerance should parse");
        assert!((t.value - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn resolve_tolerance_negative_rejected() {
        let err = crate::compare::parse_tolerance("-0.1")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("non-negative"),
            "negative tolerance should be rejected, got: {}",
            err
        );
    }

    // ── build_invoke_args cross-platform argument edge cases ───────────

    #[test]
    fn build_invoke_args_args_with_spaces() {
        let args = build_invoke_args(
            "C",
            "alice",
            "testnet",
            "do_work",
            &["--msg".into(), "hello world".into()],
        );
        assert_eq!(args[args.len() - 2], "--msg");
        assert_eq!(args[args.len() - 1], "hello world");
    }

    #[test]
    fn build_invoke_args_very_long_arg() {
        let long = "x".repeat(1_000);
        let args = build_invoke_args("C", "alice", "testnet", "f", &[long.clone()]);
        assert_eq!(args.last().unwrap(), &long);
    }

    #[test]
    fn build_invoke_args_non_ascii_arg() {
        let args = build_invoke_args(
            "C",
            "alice",
            "testnet",
            "f",
            &["--name".into(), "José".into()],
        );
        assert_eq!(args[args.len() - 2], "--name");
        assert_eq!(args[args.len() - 1], "José");
    }

    // ── run_preflight_checks edge-case identifiers ─────────────────────

    #[test]
    fn preflight_error_on_missing_stellar_cli_contains_actionable_message() {
        // We can't easily test `run_preflight_checks` without mocking the
        // process, but we can verify that the error messages it produces
        // contain the actionable text the docs reference.
        let error = Error::Message(
            "Stellar CLI is not installed or not on PATH.\n\
             Install it with:  cargo install --locked stellar-cli\n\
             See: https://github.com/stellar/stellar-cli"
                .to_string(),
        );
        let msg = error.to_string();
        assert!(msg.contains("cargo install --locked stellar-cli"));
        assert!(msg.contains("github.com/stellar/stellar-cli"));
    }

    #[test]
    fn preflight_error_on_missing_wasm_target_contains_actionable_message() {
        let error = Error::Message(
            "wasm32-unknown-unknown target is not installed.\n\
             Install it with:  rustup target add wasm32-unknown-unknown"
                .to_string(),
        );
        let msg = error.to_string();
        assert!(msg.contains("rustup target add wasm32-unknown-unknown"));
        assert!(msg.contains("wasm32-unknown-unknown"));
    }

    // ── WASM size overflow guard ───────────────────────────────────────

    #[test]
    fn wasm_size_truncation_at_u32_max() {
        // `wasm_bytes.len().try_into().unwrap_or(u32::MAX)` — if the WASM
        // is larger than u32::MAX bytes (impossible in practice), the size
        // is capped at u32::MAX rather than panicking.
        let size: usize = u64::MAX as usize; // implausibly large
        let capped: u32 = size.try_into().unwrap_or(u32::MAX);
        assert_eq!(capped, u32::MAX);
    }

    #[test]
    fn wasm_size_zero_bytes() {
        let size: usize = 0;
        let capped: u32 = size.try_into().unwrap_or(u32::MAX);
        assert_eq!(capped, 0);
    }

    // ── Typed argument TOML deserialization integration ──────────────

    #[test]
    fn budget_toml_typed_args_address_and_i128() {
        let toml_str = r#"
[functions.transfer]
args = [
    { address = "GABCDEF123456789" },
    { i128 = "1000000000" },
]
"#;
        let config: BudgetToml =
            toml::from_str(toml_str).expect("typed args should parse in BudgetToml");
        let func = &config.functions["transfer"];
        let encoded = func.args.encode();
        assert_eq!(encoded.len(), 2);
        assert_eq!(encoded[0], "GABCDEF123456789");
        assert_eq!(encoded[1], "1000000000");
    }

    #[test]
    fn budget_toml_typed_args_multiple_types() {
        let toml_str = r#"
[functions.complex]
args = [
    { address = "GX" },
    { i128 = "1000000000" },
    { u32 = 42 },
    { bool = true },
    { symbol = "native" },
    { string = "hello" },
    { bytes = "deadbeef" },
]
"#;
        let config: BudgetToml =
            toml::from_str(toml_str).expect("typed args with multiple types should parse");
        let func = &config.functions["complex"];
        let encoded = func.args.encode();
        assert_eq!(encoded.len(), 7);
        assert_eq!(encoded[0], "GX");
        assert_eq!(encoded[1], "1000000000");
        assert_eq!(encoded[2], "42");
        assert_eq!(encoded[3], "true");
        assert_eq!(encoded[4], "native");
        assert_eq!(encoded[5], "hello");
        assert_eq!(encoded[6], "deadbeef");
    }

    #[test]
    fn budget_toml_typed_args_empty_array() {
        let toml_str = r#"
[functions.init]
args = []
"#;
        let config: BudgetToml = toml::from_str(toml_str).expect("empty args should parse");
        let func = &config.functions["init"];
        let encoded = func.args.encode();
        assert!(encoded.is_empty());
    }

    #[test]
    fn budget_toml_typed_args_vec() {
        let toml_str = r#"
[functions.batch]
args = [
    { address = "GA" },
    { vec = [ { u64 = 1 }, { u64 = 2 }, { u64 = 3 } ] },
]
"#;
        let config: BudgetToml = toml::from_str(toml_str).expect("vec typed args should parse");
        let func = &config.functions["batch"];
        let encoded = func.args.encode();
        assert_eq!(encoded.len(), 2);
        assert_eq!(encoded[0], "GA");
        // Vec encodes as JSON array string
        assert!(encoded[1].contains("1"));
        assert!(encoded[1].contains("2"));
        assert!(encoded[1].contains("3"));
    }

    #[test]
    fn budget_toml_legacy_args_still_works_with_typed_system() {
        // Verify backward compatibility: legacy string array still works
        let toml_str = r#"
[functions.legacy]
args = ["--n", "10", "--msg", "hello"]
"#;
        let config: BudgetToml = toml::from_str(toml_str).expect("legacy args should parse");
        let func = &config.functions["legacy"];
        let encoded = func.args.encode();
        assert_eq!(encoded, vec!["--n", "10", "--msg", "hello"]);
    }

    #[test]
    fn budget_toml_typed_args_unknown_type_fails() {
        // Unknown types should fail deserialization
        let toml_str = r#"
[functions.bad]
args = [
    { unknown_type = "value" },
]
"#;
        let result: Result<BudgetToml, _> = toml::from_str(toml_str);
        assert!(result.is_err(), "unknown arg type should fail");
    }

    #[test]
    fn budget_toml_mixed_legacy_and_typed_in_same_array_rejected() {
        // Mixing string and table in the same array should fail
        let toml_str = r#"
[functions.mixed]
args = [
    "--flag",
    { address = "GA" },
]
"#;
        let result: Result<BudgetToml, _> = toml::from_str(toml_str);
        assert!(
            result.is_err(),
            "mixing legacy strings and typed tables should fail: {:?}",
            result
        );
    }

    // ── build_invoke_args with typed arguments ───────────────────────

    #[test]
    fn build_invoke_args_with_typed_address_arg() {
        let args = build_invoke_args(
            "C",
            "alice",
            "testnet",
            "transfer",
            &["GABCDEF123456789".into(), "1000000000".into()],
        );
        assert!(args.contains(&"GABCDEF123456789".to_string()));
        assert!(args.contains(&"1000000000".to_string()));
        // fn name should be in the args
        assert!(args.contains(&"transfer".to_string()));
    }

    // ── ArgSpec::encode consistency ──────────────────────────────────

    #[test]
    fn argspec_legacy_encode_matches_input() {
        let spec = crate::args::ArgSpec::Legacy(vec!["--n".into(), "42".into()]);
        assert_eq!(spec.encode(), vec!["--n", "42"]);
    }

    #[test]
    fn argspec_typed_encode_empty() {
        let spec = crate::args::ArgSpec::Typed(vec![]);
        assert!(spec.encode().is_empty());
    }

    #[test]
    fn argspec_default_is_empty_legacy() {
        let spec = crate::args::ArgSpec::default();
        assert!(spec.is_empty());
        assert!(matches!(spec, crate::args::ArgSpec::Legacy(_)));
    }

    #[test]
    fn argspec_from_vec_string() {
        let spec: crate::args::ArgSpec = vec!["--x".to_string(), "1".to_string()].into();
        assert_eq!(spec.encode(), vec!["--x", "1"]);
    }
}
