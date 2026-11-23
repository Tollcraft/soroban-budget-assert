//! Integration tests for issue #1020: `--rpc-url` requires
//! `--network-passphrase` at parse time.
//!
//! `BudgetReportArgs::rpc_url` carries `requires = "network_passphrase"`, so
//! a custom endpoint without a passphrase is rejected by clap before any
//! network activity starts.

#[path = "../src/cli/mod.rs"]
mod cli;

use clap::error::ErrorKind;
use clap::Parser;
use cli::args::BudgetReportArgs;
use cli::CargoCli;

fn parse(argv: &[&str]) -> Result<BudgetReportArgs, clap::Error> {
    let mut full = vec!["cargo", "budget-report"];
    full.extend_from_slice(argv);
    CargoCli::try_parse_from(full).map(|CargoCli::BudgetReport(args)| args)
}

#[test]
fn rpc_url_without_passphrase_fails_at_parse_time() {
    let err = parse(&["--rpc-url", "http://127.0.0.1:8000/soroban/rpc"])
        .expect_err("--rpc-url without --network-passphrase must be rejected");
    assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);

    let message = err.to_string();
    assert!(
        message.contains("--network-passphrase"),
        "the parse error should name the required option, got:\n{message}"
    );
}

#[test]
fn passphrase_alone_parses_cleanly() {
    let args = parse(&["--network-passphrase", "Standalone Network ; February 2017"])
        .expect("--network-passphrase alone should parse");
    assert_eq!(
        args.network_passphrase.as_deref(),
        Some("Standalone Network ; February 2017")
    );
    assert!(args.rpc_url.is_none());
}

#[test]
fn rpc_url_with_passphrase_parses_and_keeps_both_values() {
    let args = parse(&[
        "--rpc-url",
        "http://127.0.0.1:8000/soroban/rpc",
        "--network-passphrase",
        "Standalone Network ; February 2017",
    ])
    .expect("--rpc-url plus --network-passphrase should parse");
    assert_eq!(
        args.rpc_url.as_deref(),
        Some("http://127.0.0.1:8000/soroban/rpc")
    );
    assert_eq!(
        args.network_passphrase.as_deref(),
        Some("Standalone Network ; February 2017")
    );
}
