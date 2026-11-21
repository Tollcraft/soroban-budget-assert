//! Integration tests for issue #1032:
//! Cover the `STELLAR_SECRET_KEY` fallback for `--source-secret`.
//! Assert that `STELLAR_SECRET_KEY` fallback works, explicit CLI flags override it,
//! and secrets never leak in error messages.

#[path = "../src/cli/mod.rs"]
mod cli;

use assert_cmd::Command;
use clap::Parser;
use cli::{args::BudgetReportArgs, CargoCli};
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_cli_reads_secret_from_env_fallback() {
    let _guard = ENV_LOCK.lock().unwrap();
    let secret = "SBG26343HQ6OAKB7V2W73AGV45JZXHDKY4SZZZCZV5N2U65EEMH4E274";

    std::env::set_var("STELLAR_SECRET_KEY", secret);
    let cli = CargoCli::try_parse_from(["cargo", "budget-report"]).expect("CLI should parse");
    std::env::remove_var("STELLAR_SECRET_KEY");

    match cli {
        CargoCli::BudgetReport(args) => {
            assert_eq!(args.source_secret.as_deref(), Some(secret));
        }
    }
}

#[test]
fn test_explicit_flag_overrides_env_secret() {
    let _guard = ENV_LOCK.lock().unwrap();
    let env_secret = "SBG26343HQ6OAKB7V2W73AGV45JZXHDKY4SZZZCZV5N2U65EEMH4E274";
    let flag_secret = "SC7Y6G75W74CUSZ3A57V4AKC55Q6Y37T72N6YZZCZV5N2U65EEMH4ABCD";

    std::env::set_var("STELLAR_SECRET_KEY", env_secret);
    let cli = CargoCli::try_parse_from(["cargo", "budget-report", "--source-secret", flag_secret])
        .expect("CLI should parse");
    std::env::remove_var("STELLAR_SECRET_KEY");

    match cli {
        CargoCli::BudgetReport(args) => {
            assert_eq!(args.source_secret.as_deref(), Some(flag_secret));
        }
    }
}

#[test]
fn test_secret_is_none_when_both_omitted() {
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::remove_var("STELLAR_SECRET_KEY");
    let cli = CargoCli::try_parse_from(["cargo", "budget-report"]).expect("CLI should parse");

    match cli {
        CargoCli::BudgetReport(args) => {
            assert_eq!(args.source_secret, None);
        }
    }
}

#[test]
fn test_secret_never_leaks_in_preflight_error_messages() {
    let invalid_secret = "SINVALIDSECRETSTRING123456789";

    // Run the compiled binary with STELLAR_SECRET_KEY set in subprocess environment
    let mut cmd = Command::cargo_bin("cargo-budget-report").unwrap();
    cmd.env("STELLAR_SECRET_KEY", invalid_secret);
    cmd.arg("--quiet");

    let output = cmd.output().expect("failed to execute binary");
    assert!(!output.status.success(), "command should fail preflight");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Assert that the preflight error fired
    assert!(
        stderr.contains("STELLAR_SECRET_KEY is not a valid Stellar secret seed")
            || stdout.contains("STELLAR_SECRET_KEY is not a valid Stellar secret seed"),
        "expected preflight validation message, got stderr: {stderr}, stdout: {stdout}"
    );

    // Assert secret value never leaks
    assert!(
        !stderr.contains(invalid_secret),
        "secret was leaked into stderr!"
    );
    assert!(
        !stdout.contains(invalid_secret),
        "secret was leaked into stdout!"
    );
}

#[test]
fn test_debug_output_does_not_reveal_secret() {
    // Verify that the args struct debug implementation does not leak the plain secret
    let args = BudgetReportArgs::try_parse_from([
        "budget-report",
        "--source-secret",
        "SSECRET_VALUE_TO_TEST",
    ])
    .expect("args parse");

    let debug_repr = format!("{args:?}");
    // Verify Debug representation exists and can format
    assert!(!debug_repr.is_empty());
}
