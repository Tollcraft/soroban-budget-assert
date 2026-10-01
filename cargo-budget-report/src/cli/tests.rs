use super::args::BudgetReportArgs;
use super::*;
use clap::error::ErrorKind;

#[test]
fn json_and_csv_are_mutually_exclusive() {
    let err = CargoCli::try_parse_from(["cargo", "budget-report", "--json", "--csv"])
        .expect_err("--json and --csv together should be rejected");
    assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
}

#[test]
fn json_alone_is_accepted() {
    let result = CargoCli::try_parse_from(["cargo", "budget-report", "--json"]);
    assert!(result.is_ok(), "--json alone should parse: {result:?}");
}

#[test]
fn csv_alone_is_accepted() {
    let result = CargoCli::try_parse_from(["cargo", "budget-report", "--csv"]);
    assert!(result.is_ok(), "--csv alone should parse: {result:?}");
}

fn parse_args(argv: &[&str]) -> Result<BudgetReportArgs, clap::Error> {
    let mut full = vec!["cargo", "budget-report"];
    full.extend_from_slice(argv);
    CargoCli::try_parse_from(full).map(|CargoCli::BudgetReport(a)| a)
}

#[test]
fn rpc_url_requires_network_passphrase() {
    let err = parse_args(&["--rpc-url", "http://localhost:8000/soroban/rpc"])
        .expect_err("--rpc-url without --network-passphrase should be rejected");
    assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
}

#[test]
fn rpc_url_with_passphrase_parses_and_overrides() {
    let args = parse_args(&[
        "--rpc-url",
        "http://localhost:8000/soroban/rpc",
        "--network-passphrase",
        "Standalone Network ; February 2017",
    ])
    .expect("--rpc-url + --network-passphrase should parse");
    assert_eq!(
        args.rpc_url.as_deref(),
        Some("http://localhost:8000/soroban/rpc")
    );
    assert_eq!(
        args.network_passphrase.as_deref(),
        Some("Standalone Network ; February 2017")
    );
}

#[test]
fn no_deploy_cache_defaults_off_and_parses_on() {
    assert!(!parse_args(&[]).unwrap().no_deploy_cache);
    assert!(parse_args(&["--no-deploy-cache"]).unwrap().no_deploy_cache);
}

#[test]
fn source_secret_parses_from_flag() {
    let args = parse_args(&["--source-secret", "SXXXXXXXX"]).unwrap();
    assert_eq!(args.source_secret.as_deref(), Some("SXXXXXXXX"));
}

#[test]
fn record_baseline_and_check_baseline_are_mutually_exclusive() {
    let err = CargoCli::try_parse_from([
        "cargo",
        "budget-report",
        "--record-baseline",
        "out.json",
        "--check-baseline",
        "base.json",
    ])
    .expect_err("--record-baseline and --check-baseline together should be rejected");
    assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
}
