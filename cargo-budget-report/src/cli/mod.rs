//! Command-line surface for `cargo budget-report`.
//!
//! This module owns *only* the argument definitions: the flags and options
//! users type, their defaults, and the relationships between them
//! (`conflicts_with`, `requires`, `env`). Every doc comment on a field below
//! is user-facing — clap renders it as the `--help` text — so keep the
//! wording accurate for end users rather than describing implementation
//! details.
//!
//! Behaviour lives in `main.rs`. A field here should never do work; it only
//! carries a parsed value that `main.rs` interprets (typically in
//! [`crate::Mode::from_args`]).
//!
//! The surface is split into focused submodules so each file stays small and
//! a reader can find one concern without scanning the whole flag list:
//!
//! * [`args`] holds [`args::BudgetReportArgs`], the parsed flag/option
//!   definitions.
//! * [`color`] holds [`color::ColorChoice`], the `--color` policy enum.

pub mod args;
pub mod color;

#[cfg(test)]
mod tests;

use args::BudgetReportArgs;
use clap::Parser;

/// Top-level CLI entry point for `cargo budget-report`.
///
/// A cargo subcommand is invoked as `cargo budget-report [OPTIONS]`, i.e. the
/// binary receives `budget-report` as its first positional argument. Wrapping
/// the real argument struct in a single-variant enum (instead of parsing
/// [`BudgetReportArgs`] directly) lets clap model that leading subcommand
/// token while still accepting the arguments that follow it.
///
/// `name`/`bin_name` are deliberately `cargo`: they make `--help` and error
/// messages read `cargo budget-report ...`, matching how the user actually
/// typed the command, rather than the internal binary name.
#[derive(Parser, Debug)]
#[command(name = "cargo", bin_name = "cargo")]
pub enum CargoCli {
    BudgetReport(BudgetReportArgs),
}

/// Conservative default for `--concurrency` (functions in flight per
/// package). See the flag help text for the rationale.
pub const DEFAULT_CONCURRENCY: usize = 4;
