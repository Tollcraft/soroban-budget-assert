//! Integration tests for issue #1019: `--force` is accepted without `--init`.
//!
//! `BudgetReportArgs::force` is documented as "Allow `--init` to overwrite an
//! existing `budget.toml`", but its clap attribute carries no
//! `requires = "init"`. These tests pin today's parsing behaviour: `--force`
//! on its own parses without error and leaves `--init` off. Adding the
//! `requires` link later must therefore come with a deliberate update to this
//! file.

#[path = "../src/cli/mod.rs"]
mod cli;

use clap::Parser;
use cli::args::BudgetReportArgs;
use cli::CargoCli;

fn parse(argv: &[&str]) -> BudgetReportArgs {
    let mut full = vec!["cargo", "budget-report"];
    full.extend_from_slice(argv);
    match CargoCli::try_parse_from(full).expect("arguments should parse") {
        CargoCli::BudgetReport(args) => args,
    }
}

#[test]
fn force_without_init_is_silently_accepted() {
    let args = parse(&["--force"]);
    assert!(args.force);
    assert!(!args.init);
}

#[test]
fn force_with_init_sets_both_flags() {
    let args = parse(&["--force", "--init"]);
    assert!(args.force);
    assert!(args.init);
}

#[test]
fn init_alone_does_not_enable_force() {
    let args = parse(&["--init"]);
    assert!(args.init);
    assert!(!args.force);
}

#[test]
fn both_scaffolding_flags_default_off() {
    let args = parse(&[]);
    assert!(!args.force);
    assert!(!args.init);
}
