//! Percentage values outside 1–100 must fail at compile time.

use budget_macros::budget_cpu_lt;

#[budget_cpu_lt(pct = 0, of = env_file = "tier-a-limits.env", env = "NETWORK__CPU")]
fn test_pct_zero() {
    let env = ();
}

/// The compile-fail case's static inputs, grouped into one named unit (#626).
/// The attribute above is pinned by `pct_out_of_range.stderr` at line 5, so this
/// module deliberately starts below it and never moves the offending line.
mod pct_zero_case {
    /// The out-of-range percentage the attribute is built from.
    pub const OUT_OF_RANGE_PCT: u32 = 0;

    /// The first percentage the macro accepts.
    pub const MIN_VALID_PCT: u32 = 1;

    /// The last percentage the macro accepts.
    pub const MAX_VALID_PCT: u32 = 100;

    /// Whether `pct` is inside the macro's accepted range.
    pub fn is_in_range(pct: u32) -> bool {
        (MIN_VALID_PCT..=MAX_VALID_PCT).contains(&pct)
    }

    /// The rejected value, asserted to be out of range so the case cannot drift
    /// into passing if the accepted range is ever widened.
    pub fn rejected_value_is_out_of_range() -> bool {
        !is_in_range(OUT_OF_RANGE_PCT)
    }
}

fn main() {
    // Never reached: compilation stops at the attribute on line 5, before any of
    // this is type-checked. It is here so the extracted helpers are exercised
    // from a normal entry point rather than only by the compile-fail itself.
    assert!(pct_zero_case::rejected_value_is_out_of_range());
}
