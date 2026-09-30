//! Comprehensive CLI argument parsing tests.
//!
//! These tests validate:
//! 1. Each argument parses to the expected value
//! 2. All documented default values
//! 3. Precedence between CLI flags and budget.toml (verified at integration level)
//! 4. Invalid combinations are rejected with useful messages
//!
//! Tests target specific argument fields rather than whole-struct assertions
//! to remain stable as the CLI evolves.

#[cfg(test)]
mod tests {
    use crate::cli::args::BudgetReportArgs;
    use crate::cli::CargoCli;
    use clap::error::ErrorKind;
    use clap::Parser;

    /// Helper to parse BudgetReportArgs from a vector of strings.
    /// Prepends "cargo budget-report" to simulate the cargo subcommand structure.
    fn parse_args(args: &[&str]) -> Result<BudgetReportArgs, clap::Error> {
        let mut full_args = Vec::with_capacity(args.len() + 2);
        full_args.extend_from_slice(&["cargo", "budget-report"]);
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
        // According to cli/args.rs, json has default_value_t = false
        let args = parse_args(&[]).unwrap();
        assert!(!args.json, "json should default to false");
    }

    #[test]
    fn test_default_check_is_false() {
        // According to cli/args.rs, check has default_value_t = false
        let args = parse_args(&[]).unwrap();
        assert!(!args.check, "check should default to false");
    }

    #[test]
    fn test_default_csv_is_false() {
        // According to cli/args.rs, csv has default_value_t = false
        let args = parse_args(&[]).unwrap();
        assert!(!args.csv, "csv should default to false");
    }

    #[test]
    fn test_default_quiet_is_false() {
        // According to cli/args.rs, quiet has default_value_t = false
        let args = parse_args(&[]).unwrap();
        assert!(!args.quiet, "quiet should default to false");
    }

    #[test]
    fn test_default_validate_is_false() {
        // According to cli/args.rs, validate has default_value_t = false
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
        assert_eq!(
            args.profile, None,
            "profile should default to None in CLI struct"
        );
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
        // `--json` and `--csv` name two different output encodings for the
        // same report, so clap rejects them as a conflicting pair rather
        // than silently letting one win at runtime.
        let err = parse_args(&["--json", "--csv"])
            .expect_err("--json and --csv together should be rejected");
        assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
    }

    #[test]
    fn test_check_baseline_with_tolerance() {
        let args =
            parse_args(&["--check-baseline", "baseline.json", "--tolerance", "0.10"]).unwrap();
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
        let args = parse_args(&["--max-retry-attempts", "3", "--retry-backoff-secs", "5"]).unwrap();
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
    fn test_record_and_check_baseline_conflict() {
        // Writing a new baseline and checking against an old one are opposite
        // modes for the same run, so clap rejects the pair at parse time
        // instead of deferring to runtime.
        let err = parse_args(&[
            "--record-baseline",
            "new.json",
            "--check-baseline",
            "old.json",
        ])
        .expect_err("--record-baseline and --check-baseline together should be rejected");
        assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
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
        assert!(result.is_err(), "unknown flags should be rejected by clap");
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
            err_str.contains("a value is required"),
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
        assert!(
            !args.check,
            "reference.md documents --check defaults to false"
        );
    }

    #[test]
    fn test_documented_json_default() {
        // Reference.md documents --json is optional and not set by default
        let args = parse_args(&[]).unwrap();
        assert!(
            !args.json,
            "reference.md documents --json defaults to false"
        );
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
        let args = parse_args(&["--network", "testnet", "--profile", "release-opt"]).unwrap();
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
    fn test_record_and_check_baseline_together_rejected() {
        // Same conflict as `test_record_and_check_baseline_conflict`; kept as
        // a separate case so the rejection is covered alongside the other
        // baseline-flag tests in this section.
        let err = parse_args(&[
            "--record-baseline",
            "new.json",
            "--check-baseline",
            "old.json",
        ])
        .expect_err("--record-baseline and --check-baseline together should be rejected");
        assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
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
    fn test_json_and_csv_together_rejected() {
        // Same conflict as `test_json_and_csv_together`, asserted through the
        // error kind clap reports rather than the message text.
        let err = parse_args(&["--json", "--csv"])
            .expect_err("--json and --csv together should be rejected");
        assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
    }

    // ========================================================================
    // SECTION 9: Arguments added after the original suite (#700)
    //
    // Each flag below previously had no parse-level coverage. For every one we
    // pin: the default, the happy path, and the failure modes clap owns
    // (missing value, wrong type, declared conflicts and requirements).
    // ========================================================================

    use crate::cli::color::ColorChoice;
    use crate::cli::DEFAULT_CONCURRENCY;
    use clap::CommandFactory;

    /// Looks up a clap argument definition by its field id.
    fn arg_def(id: &str) -> clap::Arg {
        BudgetReportArgs::command()
            .get_arguments()
            .find(|a| a.get_id() == id)
            .unwrap_or_else(|| panic!("no argument with id `{id}`"))
            .clone()
    }

    // ---- boolean switches: --allow-mainnet, --markdown, --hide-unchanged, --watch ----

    #[test]
    fn test_default_allow_mainnet_is_false() {
        // Safety-relevant: a mainnet deploy must be an explicit opt-in.
        assert!(!parse_args(&[]).unwrap().allow_mainnet);
    }

    #[test]
    fn test_allow_mainnet_flag_parses() {
        assert!(parse_args(&["--allow-mainnet"]).unwrap().allow_mainnet);
    }

    #[test]
    fn test_default_markdown_is_false() {
        assert!(!parse_args(&[]).unwrap().markdown);
    }

    #[test]
    fn test_markdown_flag_parses() {
        assert!(parse_args(&["--markdown"]).unwrap().markdown);
    }

    #[test]
    fn test_markdown_has_no_parse_time_conflict_with_other_formats() {
        // Only `--json`/`--csv` declare a conflict. Combining `--markdown` with
        // either is accepted by clap; the format precedence is resolved at
        // runtime, so this pins that it is *not* a parse error.
        let args = parse_args(&["--markdown", "--json"]).unwrap();
        assert!(args.markdown && args.json);
        let args = parse_args(&["--markdown", "--csv"]).unwrap();
        assert!(args.markdown && args.csv);
    }

    #[test]
    fn test_default_hide_unchanged_is_false() {
        assert!(!parse_args(&[]).unwrap().hide_unchanged);
    }

    #[test]
    fn test_hide_unchanged_flag_parses() {
        assert!(parse_args(&["--hide-unchanged"]).unwrap().hide_unchanged);
    }

    #[test]
    fn test_hide_unchanged_with_check_baseline() {
        let args = parse_args(&["--check-baseline", "base.json", "--hide-unchanged"]).unwrap();
        assert!(args.hide_unchanged);
        assert_eq!(args.check_baseline.as_deref(), Some("base.json"));
    }

    #[test]
    fn test_default_watch_is_false() {
        assert!(!parse_args(&[]).unwrap().watch);
    }

    #[test]
    fn test_watch_flag_parses() {
        assert!(parse_args(&["--watch"]).unwrap().watch);
    }

    #[test]
    fn test_default_no_deploy_cache_is_false() {
        assert!(!parse_args(&[]).unwrap().no_deploy_cache);
    }

    #[test]
    fn test_no_deploy_cache_flag_parses() {
        assert!(parse_args(&["--no-deploy-cache"]).unwrap().no_deploy_cache);
    }

    #[test]
    fn test_boolean_switches_reject_an_explicit_value() {
        // These are presence flags, not `--flag=true`; a value is an error
        // rather than being silently ignored.
        for flag in [
            "--allow-mainnet=true",
            "--markdown=false",
            "--hide-unchanged=1",
            "--watch=yes",
            "--no-deploy-cache=true",
        ] {
            let err = parse_args(&[flag]).expect_err(flag);
            assert_eq!(err.kind(), ErrorKind::TooManyValues, "{flag}");
        }
    }

    // ---- --html ----

    #[test]
    fn test_default_html_is_none() {
        assert_eq!(parse_args(&[]).unwrap().html, None);
    }

    #[test]
    fn test_html_parses_a_path() {
        let args = parse_args(&["--html", "report.html"]).unwrap();
        assert_eq!(args.html.as_deref(), Some("report.html"));
    }

    #[test]
    fn test_html_parses_equals_syntax_and_nested_path() {
        let args = parse_args(&["--html=out/nested dir/report.html"]).unwrap();
        assert_eq!(args.html.as_deref(), Some("out/nested dir/report.html"));
    }

    #[test]
    fn test_html_requires_value() {
        let err = parse_args(&["--html"]).expect_err("--html needs a path");
        assert_eq!(err.kind(), ErrorKind::InvalidValue);
    }

    #[test]
    fn test_html_combines_with_check() {
        let args = parse_args(&["--check", "--html", "r.html"]).unwrap();
        assert!(args.check);
        assert_eq!(args.html.as_deref(), Some("r.html"));
    }

    // ---- --record / --replay ----

    #[test]
    fn test_default_record_and_replay_are_none() {
        let args = parse_args(&[]).unwrap();
        assert_eq!(args.record, None);
        assert_eq!(args.replay, None);
    }

    #[test]
    fn test_record_parses_a_path() {
        let args = parse_args(&["--record", "run.fixture.json"]).unwrap();
        assert_eq!(args.record.as_deref(), Some("run.fixture.json"));
        assert_eq!(args.replay, None);
    }

    #[test]
    fn test_replay_parses_a_path() {
        let args = parse_args(&["--replay", "run.fixture.json"]).unwrap();
        assert_eq!(args.replay.as_deref(), Some("run.fixture.json"));
        assert_eq!(args.record, None);
    }

    #[test]
    fn test_record_and_replay_conflict_in_both_orders() {
        for argv in [
            ["--record", "a.json", "--replay", "b.json"],
            ["--replay", "b.json", "--record", "a.json"],
        ] {
            let err = parse_args(&argv).expect_err("--record and --replay are mutually exclusive");
            assert_eq!(err.kind(), ErrorKind::ArgumentConflict, "{argv:?}");
        }
    }

    #[test]
    fn test_record_and_replay_require_values() {
        for flag in ["--record", "--replay"] {
            let err = parse_args(&[flag]).expect_err(flag);
            assert_eq!(err.kind(), ErrorKind::InvalidValue, "{flag}");
        }
    }

    #[test]
    fn test_replay_allows_offline_output_flags() {
        let args = parse_args(&["--replay", "f.json", "--json", "--check", "--quiet"]).unwrap();
        assert!(args.json && args.check && args.quiet);
    }

    // ---- --color ----

    #[test]
    fn test_default_color_is_auto() {
        assert_eq!(parse_args(&[]).unwrap().color, ColorChoice::Auto);
        assert_eq!(ColorChoice::default(), ColorChoice::Auto);
    }

    #[test]
    fn test_color_accepts_each_documented_value() {
        for (value, expected) in [
            ("auto", ColorChoice::Auto),
            ("always", ColorChoice::Always),
            ("never", ColorChoice::Never),
        ] {
            let args = parse_args(&["--color", value]).unwrap();
            assert_eq!(args.color, expected, "--color {value}");
            let args = parse_args(&[&format!("--color={value}")]).unwrap();
            assert_eq!(args.color, expected, "--color={value}");
        }
    }

    #[test]
    fn test_color_rejects_unknown_and_wrongly_cased_values() {
        // Matching is case-sensitive and there is no boolean shorthand.
        for value in ["ALWAYS", "Never", "yes", "true", "0", ""] {
            let err = parse_args(&["--color", value]).expect_err(value);
            assert_eq!(err.kind(), ErrorKind::InvalidValue, "--color {value:?}");
        }
    }

    #[test]
    fn test_color_invalid_value_error_lists_the_valid_choices() {
        let err = parse_args(&["--color", "rainbow"]).unwrap_err();
        let message = err.to_string();
        for choice in ["auto", "always", "never"] {
            assert!(message.contains(choice), "missing `{choice}` in: {message}");
        }
    }

    #[test]
    fn test_color_requires_value() {
        let err = parse_args(&["--color"]).expect_err("--color needs a value");
        assert_eq!(err.kind(), ErrorKind::InvalidValue);
    }

    // ---- --rpc-url / --network-passphrase ----

    #[test]
    fn test_default_rpc_url_and_passphrase_are_none() {
        let args = parse_args(&[]).unwrap();
        assert_eq!(args.rpc_url, None);
        assert_eq!(args.network_passphrase, None);
    }

    #[test]
    fn test_rpc_url_requires_network_passphrase_in_either_order() {
        let err = parse_args(&["--rpc-url", "http://localhost:8000/soroban/rpc"])
            .expect_err("--rpc-url alone must be rejected");
        assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
        assert!(
            err.to_string().contains("--network-passphrase"),
            "the error should name the missing flag: {err}"
        );

        for argv in [
            [
                "--rpc-url",
                "http://localhost:8000/soroban/rpc",
                "--network-passphrase",
                "P",
            ],
            [
                "--network-passphrase",
                "P",
                "--rpc-url",
                "http://localhost:8000/soroban/rpc",
            ],
        ] {
            let args = parse_args(&argv).unwrap();
            assert_eq!(
                args.rpc_url.as_deref(),
                Some("http://localhost:8000/soroban/rpc")
            );
            assert_eq!(args.network_passphrase.as_deref(), Some("P"));
        }
    }

    #[test]
    fn test_network_passphrase_alone_is_accepted() {
        // The requirement is one-directional: only `--rpc-url` needs the
        // passphrase, not the other way round.
        let args =
            parse_args(&["--network-passphrase", "Standalone Network ; February 2017"]).unwrap();
        assert_eq!(
            args.network_passphrase.as_deref(),
            Some("Standalone Network ; February 2017")
        );
        assert_eq!(args.rpc_url, None);
    }

    #[test]
    fn test_network_passphrase_keeps_spaces_and_semicolons() {
        let passphrase = "Test SDF Network ; September 2015";
        let args = parse_args(&["--network-passphrase", passphrase]).unwrap();
        assert_eq!(args.network_passphrase.as_deref(), Some(passphrase));
    }

    #[test]
    fn test_rpc_url_is_not_validated_at_parse_time() {
        // URL checking happens later (url_checks); clap only carries the string.
        let args = parse_args(&["--rpc-url", "not a url", "--network-passphrase", "P"]).unwrap();
        assert_eq!(args.rpc_url.as_deref(), Some("not a url"));
    }

    #[test]
    fn test_rpc_url_and_passphrase_require_values() {
        for argv in [
            vec!["--rpc-url"],
            vec!["--network-passphrase"],
            vec!["--network-passphrase", "P", "--rpc-url"],
        ] {
            let err = parse_args(&argv).expect_err("a value-less flag must be rejected");
            assert_eq!(err.kind(), ErrorKind::InvalidValue, "{argv:?}");
        }
    }

    #[test]
    fn test_rpc_url_combines_with_network_flag() {
        let args = parse_args(&[
            "--network",
            "local",
            "--rpc-url",
            "http://localhost:8000/soroban/rpc",
            "--network-passphrase",
            "Standalone Network ; February 2017",
        ])
        .unwrap();
        assert_eq!(args.network.as_deref(), Some("local"));
        assert!(args.rpc_url.is_some());
    }

    // ---- --source-secret ----

    #[test]
    fn test_source_secret_parses_from_flag() {
        let args = parse_args(&["--source-secret", "SABC"]).unwrap();
        assert_eq!(args.source_secret.as_deref(), Some("SABC"));
    }

    #[test]
    fn test_source_secret_requires_value() {
        let err = parse_args(&["--source-secret"]).expect_err("--source-secret needs a value");
        assert_eq!(err.kind(), ErrorKind::InvalidValue);
    }

    #[test]
    fn test_source_secret_falls_back_to_stellar_secret_key_env() {
        // Inspect the definition instead of mutating the process environment:
        // setting variables from a parallel test run would race with every
        // other test that parses arguments.
        assert_eq!(
            arg_def("source_secret").get_env().and_then(|e| e.to_str()),
            Some("STELLAR_SECRET_KEY")
        );
    }

    #[test]
    fn test_only_source_secret_reads_the_environment() {
        // Everything else must come from flags or budget.toml; an extra env
        // binding would be an undocumented configuration channel.
        for arg in BudgetReportArgs::command().get_arguments() {
            if arg.get_id() != "source_secret" {
                assert!(
                    arg.get_env().is_none(),
                    "`{}` unexpectedly reads an environment variable",
                    arg.get_id()
                );
            }
        }
    }

    // ---- --concurrency ----

    #[test]
    fn test_default_concurrency_matches_the_documented_constant() {
        assert_eq!(DEFAULT_CONCURRENCY, 4);
        assert_eq!(parse_args(&[]).unwrap().concurrency, DEFAULT_CONCURRENCY);
    }

    #[test]
    fn test_concurrency_parses_values() {
        for (value, expected) in [("1", 1), ("8", 8), ("64", 64)] {
            assert_eq!(
                parse_args(&["--concurrency", value]).unwrap().concurrency,
                expected
            );
        }
        assert_eq!(parse_args(&["--concurrency=2"]).unwrap().concurrency, 2);
    }

    #[test]
    fn test_concurrency_zero_is_accepted_at_parse_time() {
        // Not rejected by clap; the runner clamps it to 1 (sequential).
        assert_eq!(parse_args(&["--concurrency", "0"]).unwrap().concurrency, 0);
    }

    #[test]
    fn test_concurrency_rejects_negative_fractional_and_non_numeric_values() {
        for value in ["-1", "1.5", "four", "", "0x10", "1e3"] {
            // `=` binds the value, so even `-1` reaches value validation.
            let err = parse_args(&[&format!("--concurrency={value}")]).expect_err(value);
            assert_eq!(
                err.kind(),
                ErrorKind::ValueValidation,
                "--concurrency={value:?}"
            );
        }
    }

    #[test]
    fn test_separated_negative_number_is_read_as_a_flag_not_a_value() {
        // Without `=`, clap treats `-1` as the (unknown) short flag `-1`, so the
        // user sees "unexpected argument" rather than a range error.
        let err = parse_args(&["--concurrency", "-1"]).expect_err("-1 is not a valid value");
        assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    }

    #[test]
    fn test_concurrency_rejects_values_beyond_usize() {
        let err = parse_args(&["--concurrency", "99999999999999999999999999"])
            .expect_err("overflowing usize must be rejected");
        assert_eq!(err.kind(), ErrorKind::ValueValidation);
    }

    #[test]
    fn test_concurrency_requires_value() {
        let err = parse_args(&["--concurrency"]).expect_err("--concurrency needs a value");
        assert_eq!(err.kind(), ErrorKind::InvalidValue);
    }

    // ---- integer range limits of the retry flags ----

    #[test]
    fn test_max_retry_attempts_accepts_u32_max_and_rejects_overflow() {
        let args = parse_args(&["--max-retry-attempts", "4294967295"]).unwrap();
        assert_eq!(args.max_retry_attempts, Some(u32::MAX));
        let err = parse_args(&["--max-retry-attempts", "4294967296"])
            .expect_err("u32 overflow must be rejected");
        assert_eq!(err.kind(), ErrorKind::ValueValidation);
    }

    #[test]
    fn test_retry_backoff_secs_accepts_u64_max_and_rejects_negative() {
        let args = parse_args(&["--retry-backoff-secs", "18446744073709551615"]).unwrap();
        assert_eq!(args.retry_backoff_secs, Some(u64::MAX));
        let err = parse_args(&["--retry-backoff-secs=-1"]).expect_err("negative backoff");
        assert_eq!(err.kind(), ErrorKind::ValueValidation);
    }

    // ---- generic parser behaviour ----

    #[test]
    fn test_repeating_a_single_valued_flag_is_rejected() {
        let err = parse_args(&["--network", "testnet", "--network", "futurenet"])
            .expect_err("a repeated option must not silently keep one value");
        assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
    }

    #[test]
    fn test_unexpected_positional_argument_is_rejected() {
        let err = parse_args(&["stray"]).expect_err("no positional arguments are defined");
        assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    }

    #[test]
    fn test_flag_value_can_start_with_a_dash_only_via_equals_syntax() {
        let err = parse_args(&["--network", "--json"]).expect_err("--json is not a value");
        assert_eq!(err.kind(), ErrorKind::InvalidValue);
        let args = parse_args(&["--source=-alice"]).unwrap();
        assert_eq!(args.source.as_deref(), Some("-alice"));
    }

    #[test]
    fn test_missing_subcommand_is_rejected() {
        let err = CargoCli::try_parse_from(["cargo"]).expect_err("subcommand is required");
        assert_eq!(
            err.kind(),
            ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
        );
    }

    #[test]
    fn test_other_cargo_subcommand_is_rejected() {
        let err = CargoCli::try_parse_from(["cargo", "build"]).expect_err("not our subcommand");
        assert_eq!(err.kind(), ErrorKind::InvalidSubcommand);
    }

    #[test]
    fn test_help_flag_short_circuits_with_display_help() {
        for flag in ["--help", "-h"] {
            let err = parse_args(&[flag]).expect_err("help is reported as an early exit");
            assert_eq!(err.kind(), ErrorKind::DisplayHelp, "{flag}");
            assert!(!err.use_stderr(), "help goes to stdout, not stderr");
        }
    }

    #[test]
    fn test_help_documents_every_flag() {
        // Drift guard: a flag added to the struct but hidden from `--help` (or a
        // renamed flag with stale docs) fails here.
        let help = BudgetReportArgs::command().render_long_help().to_string();
        for arg in BudgetReportArgs::command().get_arguments() {
            if let Some(long) = arg.get_long() {
                if long == "help" {
                    continue;
                }
                assert!(
                    help.contains(&format!("--{long}")),
                    "`--{long}` is missing from --help"
                );
            }
        }
    }

    #[test]
    fn test_every_option_has_user_facing_help_text() {
        for arg in BudgetReportArgs::command().get_arguments() {
            if arg.get_id() == "help" {
                continue;
            }
            assert!(
                arg.get_help().is_some() || arg.get_long_help().is_some(),
                "`{}` has no help text",
                arg.get_id()
            );
        }
    }

    #[test]
    fn test_clap_definitions_are_internally_consistent() {
        // `debug_assert` panics on duplicate flags, dangling `requires` /
        // `conflicts_with` ids and similar definition mistakes.
        CargoCli::command().debug_assert();
        BudgetReportArgs::command().debug_assert();
    }

    #[test]
    fn test_declared_conflicts_are_exactly_these() {
        // Guards against a conflict being added or dropped without a matching
        // parse test above. clap records a conflict on the field that declares
        // it (`--json` names `csv`, not the reverse) though it enforces both
        // directions, so compare unordered pairs.
        let command = BudgetReportArgs::command();
        let mut pairs = std::collections::BTreeSet::new();
        for arg in command.get_arguments() {
            for other in command.get_arg_conflicts_with(arg) {
                let (a, b) = (arg.get_id().to_string(), other.get_id().to_string());
                pairs.insert(if a <= b { (a, b) } else { (b, a) });
            }
        }
        let expected: std::collections::BTreeSet<(String, String)> = [
            ("csv", "json"),
            ("record", "replay"),
            ("check_baseline", "record_baseline"),
        ]
        .into_iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
        assert_eq!(pairs, expected);
    }
}
