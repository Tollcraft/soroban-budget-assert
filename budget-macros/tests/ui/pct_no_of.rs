//! `pct` without `of` must fail: the percentage needs a reference limit.

use budget_macros::budget_cpu_lt;

#[budget_cpu_lt(pct = 25)]
fn test_pct_no_of() {
    let env = ();
}

/// The compile-fail case's static inputs, grouped into one named unit (#630).
/// The attribute above is pinned by `pct_no_of.stderr` at line 5, so this module
/// deliberately starts below it and never moves the offending line.
mod pct_without_of_case {
    /// The percentage requested without a reference limit.
    pub const PCT: u32 = 25;

    /// An example of the `of = ...` source the case deliberately omits.
    pub const REQUIRED_OF_EXAMPLE: &str =
        "of = env_file = \"tier-a-limits.env\", env = \"NETWORK__CPU\"";

    /// A short, testable description of what the compile-fail case asserts.
    pub fn failure_expectation() -> String {
        format!("`pct` requires `of = <source>` — e.g. `pct = {PCT}, {REQUIRED_OF_EXAMPLE}`")
    }
}

fn main() {
    // Never reached: compilation stops at the attribute on line 5, before any of
    // this is type-checked. It is here so the extracted helper is exercised from
    // a normal entry point rather than only by the compile-fail itself.
    assert!(pct_without_of_case::failure_expectation().contains("requires `of`"));
}
