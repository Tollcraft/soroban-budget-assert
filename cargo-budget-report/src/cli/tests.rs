/// Comprehensive CLI argument parsing tests.
///
/// These tests validate:
/// 1. Each argument parses to the expected value
/// 2. All documented default values
/// 3. Precedence between CLI flags and budget.toml (verified at integration level)
/// 4. Invalid combinations are rejected with useful messages
///
/// Tests target specific argument fields rather than whole-struct assertions
/// to remain stable as the CLI evolves.

#[cfg(test)]
mod tests {
    use crate::cli::{BudgetReportArgs, CargoCli};
    use clap::Parser;

    /// Helper to parse BudgetReportArgs from a vector of strings.
    /// Prepends "cargo budget-report" to simulate the cargo subcommand structure.
    fn parse_args(args: &[&str]) -> Result<BudgetReportArgs, clap::Error> {
        let mut full_args = vec!["cargo", "budget-report"];
        full_args.extend_from_slice(args);
        
        match CargoCli::try_parse_from(full_args) {
            Ok(CargoCli::BudgetReport(args)) => Ok(args),
            Err(e) => Err(e),
        }
    }

    // ========================================================================
    // SECTION 1: Individual argument parsing
    // ========================================================================

    #[test]
    fn test_init_flag_parses() {
        let args = parse_args(&["--init"]).unwrap();
        assert!(args.init, "expected --init to set init=true");
    }

    #[test]
    fn test_force_flag_parses() {
        let args = parse_args(&["--force"]).unwrap();
        assert!(args.force, "expected --force to set force=true");
    }

    #[test]
    fn test_network_flag_parses() {
        let args = parse_args(&["--network", "testnet"]).unwrap();
        assert_eq!(
            args.network,
            Some("testnet".to_string()),
            "expected --network testnet"
        );
    }

    #[test]
    fn test_source_flag_parses() {
        let args = parse_args(&["--source", "alice"]).unwrap();
        assert_eq!(
            args.source,
            Some("alice".to_string()),
            "expected --source alice"
        );
    }

    #[test]
    fn test_json_flag_parses() {
        let args = parse_args(&["--json"]).unwrap();
        assert!(args.json, "expected --json to set json=true");
    }

    #[test]
    fn test_check_flag_parses() {
        let args = parse_args(&["--check"]).unwrap();
        assert!(args.check, "expected --check to set check=true");
    }

    #[test]
    fn test_csv_flag_parses() {
        let args = parse_args(&["--csv"]).unwrap();
        assert!(args.csv, "expected --csv to set csv=true");
    }

    #[test]
    fn test_quiet_flag_parses() {
        let args = parse_args(&["--quiet"]).unwrap();
        assert!(args.quiet, "expected --quiet to set quiet=true");
    }

    #[test]
    fn test_validate_flag_parses() {
        let args = parse_args(&["--validate"]).unwrap();
        assert!(args.validate, "expected --validate to set validate=true");
    }

    #[test]
    fn test_record_baseline_parses() {
        let args = parse_args(&["--record-baseline", "baseline.json"]).unwrap();
        assert_eq!(
            args.record_baseline,
            Some("baseline.json".to_string()),
            "expected --record-baseline baseline.json"
        );
    }

    #[test]
    fn test_check_baseline_parses() {
        let args = parse_args(&["--check-baseline", "baseline.json"]).unwrap();
        assert_eq!(
            args.check_baseline,
            Some("baseline.json".to_string()),
            "expected --check-baseline baseline.json"
        );
    }

    #[test]
    fn test_tolerance_parses() {
        let args = parse_args(&["--tolerance", "0.15"]).unwrap();
        assert_eq!(
            args.tolerance,
            Some("0.15".to_string()),
            "expected --tolerance 0.15"
        );
    }

    #[test]
    fn test_profile_parses() {
        let args = parse_args(&["--profile", "release-opt"]).unwrap();
        assert_eq!(
            args.profile,
            Some("release-opt".to_string()),
            "expected --profile release-opt"
        );
    }

    #[test]
    fn test_derive_limits_parses() {
        let args = parse_args(&["--derive-limits", "tier-a.env"]).unwrap();
        assert_eq!(
            args.derive_limits,
            Some("tier-a.env".to_string()),
            "expected --derive-limits tier-a.env"
        );
    }

    #[test]
    fn test_from_parses() {
        let args = parse_args(&["--from", "report.json"]).unwrap();
        assert_eq!(
            args.from,
            Some("report.json".to_string()),
            "expected --from report.json"
        );
    }

    #[test]
    fn test_from_stdin_parses() {
        let args = parse_args(&["--from", "-"]).unwrap();
        assert_eq!(
            args.from,
            Some("-".to_string()),
            "expected --from - for stdin"
        );
    }

    #[test]
    fn test_margin_cpu_parses() {
        let args = parse_args(&["--margin-cpu", "1.5"]).unwrap();
        assert_eq!(
            args.margin_cpu,
            Some("1.5".to_string()),
            "expected --margin-cpu 1.5"
        );
    }

    #[test]
    fn test_margin_memory_parses() {
        let args = parse_args(&["--margin-memory", "1.2"]).unwrap();
        assert_eq!(
            args.margin_memory,
            Some("1.2".to_string()),
            "expected --margin-memory 1.2"
        );
    }

    #[test]
    fn test_margin_read_parses() {
        let args = parse_args(&["--margin-read", "1.3"]).unwrap();
        assert_eq!(
            args.margin_read,
            Some("1.3".to_string()),
            "expected --margin-read 1.3"
        );
    }

    #[test]
    fn test_margin_write_parses() {
        let args = parse_args(&["--margin-write", "1.4"]).unwrap();
        assert_eq!(
            args.margin_write,
            Some("1.4".to_string()),
            "expected --margin-write 1.4"
        );
    }

    #[test]
    fn test_provenance_out_parses() {
        let args = parse_args(&["--provenance-out", "provenance.md"]).unwrap();
        assert_eq!(
            args.provenance_out,
            Some("provenance.md".to_string()),
            "expected --provenance-out provenance.md"
        );
    }

    #[test]
    fn test_max_retry_attempts_parses() {
        let args = parse_args(&["--max-retry-attempts", "3"]).unwrap();
        assert_eq!(
            args.max_retry_attempts,
            Some(3),
            "expected --max-retry-attempts 3"
        );
    }

    #[test]
    fn test_retry_backoff_secs_parses() {
        let args = parse_args(&["--retry-backoff-secs", "5"]).unwrap();
        assert_eq!(
            args.retry_backoff_secs,
            Some(5),
            "expected --retry-backoff-secs 5"
        );
    }

    // ========================================================================
    // SECTION 2: Default values
    // ========================================================================

    #[test]
    fn test_default_init_is_false() {
        let args = parse_args(&[]).unwrap();
        assert!(!args.init, "init should default to false");
    }

    #[test]
    fn test_default_force_is_false() {
        let args = parse_args(&[]).unwrap();
        assert!(!args.force, "force should default to false");
    }

    #[test]
    fn test_default_network_is_none() {
        let args = parse_args(&[]).unwrap();
        assert_eq!(args.network, None, "network should default to None");
    }

    #[test]
    fn test_default_source_is_none() {
        let args = parse_args(&[]).unwrap();
        assert_eq!(args.source, None, "source should default to None");
    }

    #[test]
    fn test_default_json_is_false() {
        // According to cli.rs, json has default_value_t = false
        let args = parse_args(&[]).unwrap();
        assert!(!args.json, "json should default to false");
    }

    #[test]
    fn test_default_check_is_false() {
        // According to cli.rs, check has default_value_t = false
        let args = parse_args(&[]).unwrap();
        assert!(!args.check, "check should default to false");
    }

    #[test]
    fn test_default_csv_is_false() {
        // According to cli.rs, csv has default_value_t = false
        let args = parse_args(&[]).unwrap();
        assert!(!args.csv, "csv should default to false");
    }

    #[test]
    fn test_default_quiet_is_false() {
        // According to cli.rs, quiet has default_value_t = false
        let args = parse_args(&[]).unwrap();
        assert!(!args.quiet, "quiet should default to false");
    }

    #[test]
    fn test_default_validate_is_false() {
        // According to cli.rs, validate has default_value_t = false
        let args = parse_args(&[]).unwrap();
        assert!(!args.validate, "validate should default to false");
    }

    #[test]
    fn test_default_record_baseline_is_none() {
        let args = parse_args(&[]).unwrap();
        assert_eq!(
            args.record_baseline, None,
            "record_baseline should default to None"
        );
    }

    #[test]
    fn test_default_check_baseline_is_none() {
        let args = parse_args(&[]).unwrap();
        assert_eq!(
            args.check_baseline, None,
            "check_baseline should default to None"
        );
    }

    #[test]
    fn test_default_tolerance_is_none() {
        let args = parse_args(&[]).unwrap();
        assert_eq!(args.tolerance, None, "tolerance should default to None");
    }

    #[test]
    fn test_default_profile_is_none() {
        // According to reference.md, profile defaults to "release" when not provided,
        // but that's resolved at runtime, not in the CLI struct.
        let args = parse_args(&[]).unwrap();
        assert_eq!(args.profile, None, "profile should default to None in CLI struct");
    }

    #[test]
    fn test_default_derive_limits_is_none() {
        let args = parse_args(&[]).unwrap();
        assert_eq!(
            args.derive_limits, None,
            "derive_limits should default to None"
        );
    }

    #[test]
    fn test_default_from_is_none() {
        let args = parse_args(&[]).unwrap();
        assert_eq!(args.from, None, "from should default to None");
    }

    #[test]
    fn test_default_margin_cpu_is_none() {
        let args = parse_args(&[]).unwrap();
        assert_eq!(args.margin_cpu, None, "margin_cpu should default to None");
    }

    #[test]
    fn test_default_margin_memory_is_none() {
        let args = parse_args(&[]).unwrap();
        assert_eq!(
            args.margin_memory, None,
            "margin_memory should default to None"
        );
    }

    #[test]
    fn test_default_margin_read_is_none() {
        let args = parse_args(&[]).unwrap();
        assert_eq!(args.margin_read, None, "margin_read should default to None");
    }

    #[test]
    fn test_default_margin_write_is_none() {
        let args = parse_args(&[]).unwrap();
        assert_eq!(
            args.margin_write, None,
            "margin_write should default to None"
        );
    }

    #[test]
    fn test_default_provenance_out_is_none() {
        let args = parse_args(&[]).unwrap();
        assert_eq!(
            args.provenance_out, None,
            "provenance_out should default to None"
        );
    }

    #[test]
    fn test_default_max_retry_attempts_is_none() {
        // According to reference.md, defaults to 4 at runtime, but CLI struct has None
        let args = parse_args(&[]).unwrap();
        assert_eq!(
            args.max_retry_attempts, None,
            "max_retry_attempts should default to None in CLI struct"
        );
    }

    #[test]
    fn test_default_retry_backoff_secs_is_none() {
        // According to reference.md, defaults to 2 at runtime, but CLI struct has None
        let args = parse_args(&[]).unwrap();
        assert_eq!(
            args.retry_backoff_secs, None,
            "retry_backoff_secs should default to None in CLI struct"
        );
    }

    // ========================================================================
    // SECTION 3: Multiple flags and combinations
    // ========================================================================

    #[test]
    fn test_multiple_flags_parse_together() {
        let args = parse_args(&[
            "--network",
            "testnet",
            "--source",
            "alice",
            "--json",
            "--check",
            "--quiet",
        ])
        .unwrap();
        assert_eq!(args.network, Some("testnet".to_string()));
        assert_eq!(args.source, Some("alice".to_string()));
        assert!(args.json);
        assert!(args.check);
        assert!(args.quiet);
    }

    #[test]
    fn test_json_and_csv_together() {
        // Both json and csv can be set; runtime logic chooses precedence
        let args = parse_args(&["--json", "--csv"]).unwrap();
        assert!(args.json, "expected json=true");
        assert!(args.csv, "expected csv=true");
    }

    #[test]
    fn test_check_baseline_with_tolerance() {
        let args = parse_args(&["--check-baseline", "baseline.json", "--tolerance", "0.10"])
            .unwrap();
        assert_eq!(args.check_baseline, Some("baseline.json".to_string()));
        assert_eq!(args.tolerance, Some("0.10".to_string()));
    }

    #[test]
    fn test_derive_limits_with_from() {
        let args = parse_args(&["--derive-limits", "out.env", "--from", "report.json"]).unwrap();
        assert_eq!(args.derive_limits, Some("out.env".to_string()));
        assert_eq!(args.from, Some("report.json".to_string()));
    }

    #[test]
    fn test_all_margin_flags_together() {
        let args = parse_args(&[
            "--margin-cpu",
            "1.5",
            "--margin-memory",
            "1.2",
            "--margin-read",
            "1.3",
            "--margin-write",
            "1.4",
        ])
        .unwrap();
        assert_eq!(args.margin_cpu, Some("1.5".to_string()));
        assert_eq!(args.margin_memory, Some("1.2".to_string()));
        assert_eq!(args.margin_read, Some("1.3".to_string()));
        assert_eq!(args.margin_write, Some("1.4".to_string()));
    }

    #[test]
    fn test_retry_flags_together() {
        let args = parse_args(&["--max-retry-attempts", "3", "--retry-backoff-secs", "5"])
            .unwrap();
        assert_eq!(args.max_retry_attempts, Some(3));
        assert_eq!(args.retry_backoff_secs, Some(5));
    }

    #[test]
    fn test_init_with_force() {
        let args = parse_args(&["--init", "--force"]).unwrap();
        assert!(args.init);
        assert!(args.force);
    }

    #[test]
    fn test_record_and_check_baseline_separate() {
        // Having both record and check baseline is allowed at parse time
        // (runtime logic may reject it)
        let args = parse_args(&[
            "--record-baseline",
            "new.json",
            "--check-baseline",
            "old.json",
        ])
        .unwrap();
        assert_eq!(args.record_baseline, Some("new.json".to_string()));
        assert_eq!(args.check_baseline, Some("old.json".to_string()));
    }

    // ========================================================================
    // SECTION 4: Precedence testing (CLI vs budget.toml)
    // ========================================================================
    // 
    // Note: CLI precedence over budget.toml is tested at integration level
    // because it requires loading and merging configuration files. The CLI
    // parsing itself just captures the values; main.rs does the precedence
    // resolution.
    //
    // According to reference.md:
    // - network: CLI flag overrides budget.toml
    // - source: CLI flag overrides budget.toml  
    // - tolerance: CLI flag overrides budget.toml
    // - margin_*: CLI flag overrides budget.toml
    // - max_retry_attempts: CLI flag overrides budget.toml
    // - retry_backoff_secs: CLI flag overrides budget.toml
    //
    // These are verified in cargo-budget-report/tests/integration.rs

    // ========================================================================
    // SECTION 5: Invalid combinations and error cases
    // ========================================================================

    #[test]
    fn test_unknown_flag_rejected() {
        let result = parse_args(&["--unknown-flag"]);
        assert!(
            result.is_err(),
            "unknown flags should be rejected by clap"
        );
        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(
            err_str.contains("unexpected argument") || err_str.contains("unrecognized"),
            "error should mention unexpected/unrecognized argument, got: {}",
            err_str
        );
    }

    #[test]
    fn test_network_requires_value() {
        let result = parse_args(&["--network"]);
        assert!(result.is_err(), "--network requires a value");
        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(
            err_str.contains("requires a value") || err_str.contains("argument"),
            "error should mention missing value, got: {}",
            err_str
        );
    }

    #[test]
    fn test_source_requires_value() {
        let result = parse_args(&["--source"]);
        assert!(result.is_err(), "--source requires a value");
    }

    #[test]
    fn test_tolerance_requires_value() {
        let result = parse_args(&["--tolerance"]);
        assert!(result.is_err(), "--tolerance requires a value");
    }

    #[test]
    fn test_profile_requires_value() {
        let result = parse_args(&["--profile"]);
        assert!(result.is_err(), "--profile requires a value");
    }

    #[test]
    fn test_derive_limits_requires_value() {
        let result = parse_args(&["--derive-limits"]);
        assert!(result.is_err(), "--derive-limits requires a value");
    }

    #[test]
    fn test_from_requires_value() {
        let result = parse_args(&["--from"]);
        assert!(result.is_err(), "--from requires a value");
    }

    #[test]
    fn test_max_retry_attempts_requires_numeric_value() {
        let result = parse_args(&["--max-retry-attempts", "not-a-number"]);
        assert!(
            result.is_err(),
            "--max-retry-attempts should reject non-numeric values"
        );
        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(
            err_str.contains("invalid value") || err_str.contains("parse"),
            "error should mention invalid value or parse error, got: {}",
            err_str
        );
    }

    #[test]
    fn test_retry_backoff_secs_requires_numeric_value() {
        let result = parse_args(&["--retry-backoff-secs", "not-a-number"]);
        assert!(
            result.is_err(),
            "--retry-backoff-secs should reject non-numeric values"
        );
    }

    #[test]
    fn test_max_retry_attempts_zero() {
        // Zero is valid at parse time but may be rejected at runtime
        let args = parse_args(&["--max-retry-attempts", "0"]).unwrap();
        assert_eq!(args.max_retry_attempts, Some(0));
    }

    #[test]
    fn test_max_retry_attempts_one() {
        // According to reference.md, 1 disables retry
        let args = parse_args(&["--max-retry-attempts", "1"]).unwrap();
        assert_eq!(args.max_retry_attempts, Some(1));
    }

    #[test]
    fn test_record_baseline_empty_string() {
        // Empty string is allowed at parse time
        let args = parse_args(&["--record-baseline", ""]).unwrap();
        assert_eq!(args.record_baseline, Some("".to_string()));
    }

    // ========================================================================
    // SECTION 6: Edge cases and special values
    // ========================================================================

    #[test]
    fn test_tolerance_as_fraction() {
        let args = parse_args(&["--tolerance", "0.10"]).unwrap();
        assert_eq!(args.tolerance, Some("0.10".to_string()));
    }

    #[test]
    fn test_tolerance_as_percentage() {
        // According to reference.md, accepts "10%" format
        let args = parse_args(&["--tolerance", "10%"]).unwrap();
        assert_eq!(args.tolerance, Some("10%".to_string()));
    }

    #[test]
    fn test_network_with_spaces() {
        // Network names shouldn't have spaces, but CLI accepts it
        let args = parse_args(&["--network", "test net"]).unwrap();
        assert_eq!(args.network, Some("test net".to_string()));
    }

    #[test]
    fn test_source_with_special_characters() {
        let args = parse_args(&["--source", "alice-test_123"]).unwrap();
        assert_eq!(args.source, Some("alice-test_123".to_string()));
    }

    #[test]
    fn test_profile_release() {
        // According to reference.md, "release" is the default profile name
        let args = parse_args(&["--profile", "release"]).unwrap();
        assert_eq!(args.profile, Some("release".to_string()));
    }

    #[test]
    fn test_profile_custom() {
        // Custom profiles like "release-opt" should parse
        let args = parse_args(&["--profile", "release-opt"]).unwrap();
        assert_eq!(args.profile, Some("release-opt".to_string()));
    }

    #[test]
    fn test_margin_cpu_large_value() {
        let args = parse_args(&["--margin-cpu", "100.0"]).unwrap();
        assert_eq!(args.margin_cpu, Some("100.0".to_string()));
    }

    #[test]
    fn test_margin_cpu_small_value() {
        let args = parse_args(&["--margin-cpu", "0.01"]).unwrap();
        assert_eq!(args.margin_cpu, Some("0.01".to_string()));
    }

    #[test]
    fn test_max_retry_attempts_large_value() {
        let args = parse_args(&["--max-retry-attempts", "1000"]).unwrap();
        assert_eq!(args.max_retry_attempts, Some(1000));
    }

    #[test]
    fn test_retry_backoff_secs_zero() {
        // Zero backoff is valid (no delay between retries)
        let args = parse_args(&["--retry-backoff-secs", "0"]).unwrap();
        assert_eq!(args.retry_backoff_secs, Some(0));
    }

    #[test]
    fn test_from_stdin_dash() {
        // Special case: "-" means stdin for --from
        let args = parse_args(&["--from", "-"]).unwrap();
        assert_eq!(args.from, Some("-".to_string()));
    }

    #[test]
    fn test_path_with_spaces() {
        let args = parse_args(&["--record-baseline", "path with spaces.json"]).unwrap();
        assert_eq!(
            args.record_baseline,
            Some("path with spaces.json".to_string())
        );
    }

    #[test]
    fn test_windows_path() {
        let args = parse_args(&["--check-baseline", r"C:\Users\test\baseline.json"]).unwrap();
        assert_eq!(
            args.check_baseline,
            Some(r"C:\Users\test\baseline.json".to_string())
        );
    }

    #[test]
    fn test_unix_path() {
        let args = parse_args(&["--check-baseline", "/home/test/baseline.json"]).unwrap();
        assert_eq!(
            args.check_baseline,
            Some("/home/test/baseline.json".to_string())
        );
    }

    #[test]
    fn test_relative_path() {
        let args = parse_args(&["--check-baseline", "../baseline.json"]).unwrap();
        assert_eq!(args.check_baseline, Some("../baseline.json".to_string()));
    }

    // ========================================================================
    // SECTION 7: Flag ordering independence
    // ========================================================================

    #[test]
    fn test_flag_order_independence_1() {
        let args1 = parse_args(&["--network", "testnet", "--source", "alice"]).unwrap();
        let args2 = parse_args(&["--source", "alice", "--network", "testnet"]).unwrap();
        assert_eq!(args1.network, args2.network);
        assert_eq!(args1.source, args2.source);
    }

    #[test]
    fn test_flag_order_independence_2() {
        let args1 = parse_args(&["--json", "--check", "--quiet"]).unwrap();
        let args2 = parse_args(&["--quiet", "--json", "--check"]).unwrap();
        assert_eq!(args1.json, args2.json);
        assert_eq!(args1.check, args2.check);
        assert_eq!(args1.quiet, args2.quiet);
    }

    #[test]
    fn test_flag_order_independence_margin() {
        let args1 = parse_args(&[
            "--margin-cpu",
            "1.5",
            "--margin-memory",
            "1.2",
            "--margin-read",
            "1.3",
            "--margin-write",
            "1.4",
        ])
        .unwrap();
        let args2 = parse_args(&[
            "--margin-write",
            "1.4",
            "--margin-read",
            "1.3",
            "--margin-cpu",
            "1.5",
            "--margin-memory",
            "1.2",
        ])
        .unwrap();
        assert_eq!(args1.margin_cpu, args2.margin_cpu);
        assert_eq!(args1.margin_memory, args2.margin_memory);
        assert_eq!(args1.margin_read, args2.margin_read);
        assert_eq!(args1.margin_write, args2.margin_write);
    }

    // ========================================================================
    // SECTION 8: Documentation consistency checks
    // ========================================================================
    //
    // These tests verify that CLI parsing matches documented behavior in
    // docs/src/reference.md. If these fail, either the docs or the code
    // need updating.

    #[test]
    fn test_documented_check_default() {
        // Reference.md documents --check defaults to not set (false)
        let args = parse_args(&[]).unwrap();
        assert!(!args.check, "reference.md documents --check defaults to false");
    }

    #[test]
    fn test_documented_json_default() {
        // Reference.md documents --json is optional and not set by default
        let args = parse_args(&[]).unwrap();
        assert!(!args.json, "reference.md documents --json defaults to false");
    }

    #[test]
    fn test_documented_network_required() {
        // Reference.md says network is required (via flag or budget.toml)
        // At CLI parse level, it's optional; main.rs enforces the requirement
        let args = parse_args(&[]).unwrap();
        assert_eq!(
            args.network, None,
            "network is optional at CLI level, required at runtime"
        );
    }

    #[test]
    fn test_documented_source_required() {
        // Reference.md says source is required (via flag or budget.toml)
        // At CLI parse level, it's optional; main.rs enforces the requirement
        let args = parse_args(&[]).unwrap();
        assert_eq!(
            args.source, None,
            "source is optional at CLI level, required at runtime"
        );
    }

    #[test]
    fn test_documented_max_retry_attempts_default() {
        // Reference.md documents default is 4 (at runtime, not CLI parse time)
        let args = parse_args(&[]).unwrap();
        assert_eq!(
            args.max_retry_attempts, None,
            "max_retry_attempts defaults to None at CLI level, 4 at runtime"
        );
    }

    #[test]
    fn test_documented_retry_backoff_default() {
        // Reference.md documents default is 2 seconds (at runtime, not CLI parse time)
        let args = parse_args(&[]).unwrap();
        assert_eq!(
            args.retry_backoff_secs, None,
            "retry_backoff_secs defaults to None at CLI level, 2 at runtime"
        );
    }

    #[test]
    fn test_documented_csv_optional() {
        // Reference.md mentions CSV output via --csv flag
        let args = parse_args(&[]).unwrap();
        assert!(!args.csv, "csv should default to false");

        let args_with_csv = parse_args(&["--csv"]).unwrap();
        assert!(args_with_csv.csv, "csv should be true when --csv is passed");
    }

    #[test]
    fn test_documented_validate_optional() {
        // Reference.md documents --validate as optional
        let args = parse_args(&[]).unwrap();
        assert!(!args.validate, "validate should default to false");

        let args_with_validate = parse_args(&["--validate"]).unwrap();
        assert!(
            args_with_validate.validate,
            "validate should be true when --validate is passed"
        );
    }

    // ========================================================================
    // SECTION 9: Real-world usage patterns
    // ========================================================================

    #[test]
    fn test_typical_check_invocation() {
        // Typical CI usage: cargo budget-report --network testnet --source alice --check
        let args = parse_args(&["--network", "testnet", "--source", "alice", "--check"]).unwrap();
        assert_eq!(args.network, Some("testnet".to_string()));
        assert_eq!(args.source, Some("alice".to_string()));
        assert!(args.check);
    }

    #[test]
    fn test_typical_json_output() {
        // Typical CI usage for JSON output
        let args = parse_args(&["--network", "testnet", "--source", "alice", "--json"]).unwrap();
        assert_eq!(args.network, Some("testnet".to_string()));
        assert_eq!(args.source, Some("alice".to_string()));
        assert!(args.json);
    }

    #[test]
    fn test_typical_baseline_recording() {
        // Recording a new baseline
        let args =
            parse_args(&["--network", "testnet", "--record-baseline", "baseline.json"]).unwrap();
        assert_eq!(args.network, Some("testnet".to_string()));
        assert_eq!(args.record_baseline, Some("baseline.json".to_string()));
    }

    #[test]
    fn test_typical_baseline_checking() {
        // Checking against a baseline
        let args = parse_args(&[
            "--network",
            "testnet",
            "--check-baseline",
            "baseline.json",
            "--tolerance",
            "0.10",
        ])
        .unwrap();
        assert_eq!(args.network, Some("testnet".to_string()));
        assert_eq!(args.check_baseline, Some("baseline.json".to_string()));
        assert_eq!(args.tolerance, Some("0.10".to_string()));
    }

    #[test]
    fn test_typical_derive_limits_workflow() {
        // Deriving Tier A limits from Tier B report
        let args = parse_args(&[
            "--derive-limits",
            "tier-a.env",
            "--from",
            "tier-b.json",
            "--margin-cpu",
            "1.5",
            "--margin-memory",
            "1.2",
            "--margin-read",
            "1.3",
            "--margin-write",
            "1.4",
        ])
        .unwrap();
        assert_eq!(args.derive_limits, Some("tier-a.env".to_string()));
        assert_eq!(args.from, Some("tier-b.json".to_string()));
        assert_eq!(args.margin_cpu, Some("1.5".to_string()));
        assert_eq!(args.margin_memory, Some("1.2".to_string()));
        assert_eq!(args.margin_read, Some("1.3".to_string()));
        assert_eq!(args.margin_write, Some("1.4".to_string()));
    }

    #[test]
    fn test_typical_quiet_json_combo() {
        // Typical CI usage: quiet + json for clean output
        let args = parse_args(&["--quiet", "--json"]).unwrap();
        assert!(args.quiet);
        assert!(args.json);
    }

    #[test]
    fn test_custom_profile_usage() {
        // Using a custom build profile
        let args =
            parse_args(&["--network", "testnet", "--profile", "release-opt"]).unwrap();
        assert_eq!(args.network, Some("testnet".to_string()));
        assert_eq!(args.profile, Some("release-opt".to_string()));
    }

    #[test]
    fn test_validation_with_check() {
        // Combining validation with checking
        let args = parse_args(&["--check", "--validate"]).unwrap();
        assert!(args.check);
        assert!(args.validate);
    }

    // ========================================================================
    // SECTION 10: Mutually exclusive patterns (not enforced by clap)
    // ========================================================================
    //
    // These combinations are allowed by the CLI parser but may be rejected
    // at runtime by the main logic. We document them here for completeness.

    #[test]
    fn test_init_with_other_flags_allowed_at_parse_time() {
        // --init should likely be exclusive with other operations, but
        // clap doesn't enforce this - runtime logic should check
        let args = parse_args(&["--init", "--check"]).unwrap();
        assert!(args.init);
        assert!(args.check);
        // Runtime should probably reject this combination
    }

    #[test]
    fn test_record_and_check_baseline_together_allowed_at_parse_time() {
        // Recording and checking baseline at the same time doesn't make sense
        // but clap allows it - runtime should reject
        let args = parse_args(&[
            "--record-baseline",
            "new.json",
            "--check-baseline",
            "old.json",
        ])
        .unwrap();
        assert_eq!(args.record_baseline, Some("new.json".to_string()));
        assert_eq!(args.check_baseline, Some("old.json".to_string()));
        // Runtime should probably reject this combination
    }

    #[test]
    fn test_derive_limits_without_from_allowed_at_parse_time() {
        // --derive-limits without --from doesn't make sense (where's the input?)
        // but clap allows it - runtime should reject or default to stdin
        let args = parse_args(&["--derive-limits", "out.env"]).unwrap();
        assert_eq!(args.derive_limits, Some("out.env".to_string()));
        assert_eq!(args.from, None);
        // Runtime should probably require --from or default to stdin
    }

    #[test]
    fn test_margin_flags_without_derive_limits_allowed_at_parse_time() {
        // Margin flags without --derive-limits are meaningless
        // but clap allows it - runtime should ignore them
        let args = parse_args(&["--margin-cpu", "1.5"]).unwrap();
        assert_eq!(args.margin_cpu, Some("1.5".to_string()));
        assert_eq!(args.derive_limits, None);
        // Runtime should ignore margin flags when not deriving limits
    }

    #[test]
    fn test_json_and_csv_together_allowed() {
        // Both can be set; runtime chooses which takes precedence
        let args = parse_args(&["--json", "--csv"]).unwrap();
        assert!(args.json);
        assert!(args.csv);
        // Runtime should decide: JSON likely takes precedence
    }
}
