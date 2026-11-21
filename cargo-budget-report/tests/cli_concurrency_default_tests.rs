//! Integration tests for issue #1013: pin `DEFAULT_CONCURRENCY` as the
//! `--concurrency` default and confirm explicit overrides are respected.
//!
//! `BudgetReportArgs::concurrency` declares
//! `default_value_t = DEFAULT_CONCURRENCY`, so changing either the constant
//! or the clap attribute must be a deliberate, visible edit. Parsing through
//! `CargoCli` exercises the same path `cargo budget-report` takes.

#[path = "../src/cli/mod.rs"]
mod cli;

use clap::Parser;
use cli::{args::BudgetReportArgs, CargoCli, DEFAULT_CONCURRENCY};

fn parse(argv: &[&str]) -> BudgetReportArgs {
    let mut full = vec!["cargo", "budget-report"];
    full.extend_from_slice(argv);
    match CargoCli::try_parse_from(full).expect("arguments should parse") {
        CargoCli::BudgetReport(args) => args,
    }
}

#[test]
fn omitted_concurrency_parses_to_the_default_constant() {
    assert_eq!(parse(&[]).concurrency, DEFAULT_CONCURRENCY);
}

#[test]
fn explicit_lower_bound_override_is_respected() {
    assert_eq!(parse(&["--concurrency", "1"]).concurrency, 1);
}

#[test]
fn explicit_override_above_default_is_respected() {
    assert_eq!(parse(&["--concurrency", "16"]).concurrency, 16);
}

#[test]
fn explicit_value_equal_to_default_matches_the_constant() {
    let value = DEFAULT_CONCURRENCY.to_string();
    assert_eq!(
        parse(&["--concurrency", &value]).concurrency,
        DEFAULT_CONCURRENCY
    );
}
