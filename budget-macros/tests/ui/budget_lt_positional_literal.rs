//! A positional literal is not a `budget_lt` property.
//!
//! `BudgetSpec` parses a comma-separated list of `key = value` pairs, so
//! `#[budget_lt(1_000_000)]` — the form the reference page documented as the
//! "static limit" example — fails while looking for the first key, before any
//! property check runs. The static form is `cpu = 1_000_000, mem = 1_000_000`.

use budget_macros::budget_lt;

#[budget_lt(1_000_000)]
fn test_positional_literal() {}

fn main() {}
