//! Integration tests for issue #1018: `--color` defaults to `auto`, every
//! documented value parses, and the enum keeps the `NO_COLOR` convention.
//!
//! `ColorChoice` is pulled in through the crate's own `cli` module so the
//! tests exercise the same `ValueEnum` definition the binary parses.

#[path = "../src/cli/mod.rs"]
mod cli;

use clap::{Parser, ValueEnum};
use cli::args::BudgetReportArgs;
use cli::color::ColorChoice;
use cli::CargoCli;
use std::fs;

fn parse(argv: &[&str]) -> Result<BudgetReportArgs, clap::Error> {
    let mut full = vec!["cargo", "budget-report"];
    full.extend_from_slice(argv);
    CargoCli::try_parse_from(full).map(|CargoCli::BudgetReport(args)| args)
}

#[test]
fn default_color_choice_is_auto() {
    assert_eq!(ColorChoice::default(), ColorChoice::Auto);
    assert_eq!(
        parse(&[]).expect("empty argv should parse").color,
        ColorChoice::Auto
    );
}

#[test]
fn every_lowercase_color_value_parses_from_the_cli() {
    for (value, expected) in [
        ("always", ColorChoice::Always),
        ("auto", ColorChoice::Auto),
        ("never", ColorChoice::Never),
    ] {
        let args = parse(&["--color", value])
            .unwrap_or_else(|e| panic!("--color {value} should parse: {e}"));
        assert_eq!(args.color, expected, "--color {value}");
    }
}

#[test]
fn value_enum_parsing_can_ignore_case() {
    // `--color` does not set `Arg::ignore_case`, so the CLI itself matches
    // lowercase values only. The derived `ValueEnum` still supports
    // case-insensitive comparison through the `ignore_case` flag.
    for (value, expected) in [
        ("ALWAYS", ColorChoice::Always),
        ("Auto", ColorChoice::Auto),
        ("nEvEr", ColorChoice::Never),
    ] {
        assert_eq!(ColorChoice::from_str(value, true), Ok(expected), "{value}");
    }
    assert!(
        ColorChoice::from_str("rainbow", true).is_err(),
        "unknown colour names must be rejected even when case is ignored"
    );
}

#[test]
fn no_color_convention_stays_documented() {
    let source = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cli/color.rs"))
        .expect("src/cli/color.rs should be readable");
    assert!(
        source.contains("NO_COLOR"),
        "ColorChoice must keep documenting the NO_COLOR convention"
    );
    assert!(
        source.contains("no-color.org"),
        "ColorChoice must keep pointing at the no-color.org convention"
    );
}
