//! Unit tests that document and verify the bitwise and numeric manipulation
//! logic inside `cargo-budget-report`.
//!
//! **Implementation location**: the functions exercised here are defined in
//! `cargo-budget-report/src/main.rs`.  Specifically:
//! * `format_with_commas_and_units` — the comma-insertion loop (~line 300).
//! * `evaluate_check` — the widening-cast comparison (~line 243).
//!
//! ## What "bitwise / numeric manipulation" means here
//!
//! The tool does not use explicit bit-shift (`<<`, `>>`) or bitwise-AND/OR/XOR
//! (`&`, `|`, `^`) operators in its formatting or check paths.  Instead, the
//! numeric manipulation that is easy to misread lives in two places:
//!
//! 1. **`format_with_commas_and_units`** — builds a comma-separated decimal
//!    string by iterating digit characters *in reverse*, using a modular
//!    counter (`digit_count % 3 == 0`) to decide when to insert a comma, then
//!    reversing the accumulated `String` a second time to restore the natural
//!    left-to-right order.  The double-reversal pattern is easy to confuse
//!    with an off-by-one error, so these tests pin every significant boundary.
//!
//!    Step-by-step for `1_000`:
//!    ```text
//!    value.to_string() = "1000"
//!    reversed chars:  '0', '0', '0', '1'
//!    iteration:
//!      push '0' → digit_count=1
//!      push '0' → digit_count=2
//!      push '0' → digit_count=3 → reset to 0, push ','
//!      push '1' → digit_count=1
//!    accumulated = "000,1"
//!    second reversal → "1,000"
//!    ```
//!
//! 2. **`evaluate_check`** — compares a `u32` measurement against a `u64`
//!    limit.  The operands deliberately have *different widths*: the
//!    measurement comes from a Soroban XDR `u32` field, while budget limits
//!    can exceed `u32::MAX` in theory.  A widening cast (`u64::from(value)`)
//!    is used instead of `as u64` or `value as u64` to make the intent
//!    explicit and to avoid any future accidental sign-extension if the type
//!    of `value` were changed to a signed integer.  The comparison is
//!    *inclusive*: `value <= limit` so that a measurement that exactly meets
//!    the limit is considered a pass.
//!
//!    The three possible outcomes of `evaluate_check`:
//!    | `limit` arg | result              | meaning                          |
//!    |-------------|---------------------|----------------------------------|
//!    | `None`      | `(None, None)`      | metric reported, not enforced    |
//!    | `Some(L)`, value ≤ L | `(Some(L), Some(true))`  | within budget  |
//!    | `Some(L)`, value > L | `(Some(L), Some(false))` | budget breached |

#[cfg(test)]
mod bitwise_and_numeric_tests {
    use crate::*;

    // ── format_with_commas_and_units ──────────────────────────────────────
    //
    // The function works by:
    //   1. Converting `value` to its decimal `String` representation.
    //   2. Iterating the characters in **reverse** (least-significant digit
    //      first) and accumulating them into a new `String`.
    //   3. Every time `digit_count` reaches 3 it resets to 0 and a comma is
    //      pushed *before* the next digit.  This is modular counting: the
    //      natural modulo-3 boundary triggers the separator.
    //   4. The accumulated string is then reversed a second time (via
    //      `.chars().rev().collect()`) to restore the most-significant-digit-
    //      first order.
    //   5. A unit suffix is appended: `" B"` when the metric name contains
    //      the substring `"Bytes"`, or `" inst."` otherwise.
    //
    // The double-reversal is semantically equivalent to a right-to-left
    // insertion with a running digit counter — commonly seen in low-level
    // numeric formatting routines where building the string right-to-left
    // avoids the need to know the total digit count in advance.

    #[test]
    fn format_single_digit_is_not_grouped() {
        // A single decimal digit has no group boundary, so no comma is
        // inserted.  The reverse-then-re-reverse path is a no-op for a
        // one-character string.
        assert_eq!(
            format_with_commas_and_units(0, "CPU Instructions"),
            "0 inst."
        );
        assert_eq!(
            format_with_commas_and_units(1, "CPU Instructions"),
            "1 inst."
        );
        assert_eq!(format_with_commas_and_units(9, "Read Bytes"), "9 B");
    }

    #[test]
    fn format_three_digit_boundary_no_comma() {
        // A 3-digit value sits exactly at the grouping boundary but does NOT
        // receive a leading comma: `digit_count` reaches 3 only *after* the
        // third digit has already been pushed, so the comma is inserted before
        // the *fourth* digit (if one exists).
        assert_eq!(
            format_with_commas_and_units(999, "CPU Instructions"),
            "999 inst."
        );
        assert_eq!(format_with_commas_and_units(100, "Write Bytes"), "100 B");
    }

    #[test]
    fn format_four_digit_value_gets_one_comma() {
        // The first comma appears when the fourth digit (thousands place) is
        // about to be pushed: after writing "001" in the reversed accumulator,
        // `digit_count` wraps back to 0 and the comma is inserted, yielding
        // "001," → reversed: ",100" → second reversal restores "1,000".
        //
        // More precisely:
        //   value = 1000  →  chars().rev() = ['0','0','0','1']
        //   digit 0 → push '0', count=1
        //   digit 0 → push '0', count=2
        //   digit 0 → push '0', count=3 → count resets to 0, comma pushed
        //   digit 1 → push '1', count=1
        //   accumulated = "000,1"  →  reversed = "1,000"
        assert_eq!(
            format_with_commas_and_units(1_000, "CPU Instructions"),
            "1,000 inst."
        );
        assert_eq!(
            format_with_commas_and_units(9_999, "CPU Instructions"),
            "9,999 inst."
        );
    }

    #[test]
    fn format_six_digit_value_gets_one_comma() {
        // 6 digits = two groups of 3; exactly one comma separates them.
        //
        // Trace for 123_456:
        //   reversed chars: '6','5','4','3','2','1'
        //   push '6' count=1, push '5' count=2, push '4' count=3
        //     → count resets to 0, push ','
        //   push '3' count=1, push '2' count=2, push '1' count=3
        //   accumulated = "654,321"  →  reversed = "123,456"
        assert_eq!(
            format_with_commas_and_units(123_456, "CPU Instructions"),
            "123,456 inst."
        );
    }

    #[test]
    fn format_seven_digit_value_gets_two_commas() {
        // 7 digits = one 1-digit group + two 3-digit groups → two commas.
        //
        // The modular counter resets independently at each group boundary,
        // so it cycles: 1→2→3→reset, 1→2→3→reset, 1.
        // Two resets = two commas inserted.
        assert_eq!(
            format_with_commas_and_units(1_234_567, "CPU Instructions"),
            "1,234,567 inst."
        );
    }

    #[test]
    fn format_typical_cpu_instruction_count() {
        // Representative values seen in real Soroban simulations.  The large
        // numbers exercise three comma positions, confirming the modular
        // counter resets correctly on each group boundary.
        assert_eq!(
            format_with_commas_and_units(2_307_555, "CPU Instructions"),
            "2,307,555 inst."
        );
        assert_eq!(
            format_with_commas_and_units(5_000_000, "CPU Instructions"),
            "5,000,000 inst."
        );
    }

    #[test]
    fn format_byte_metric_uses_b_suffix() {
        // The unit branch is chosen by a substring test: `metric.contains("Bytes")`.
        // Both "Read Bytes" and "Write Bytes" match; "CPU Instructions" does not.
        assert_eq!(format_with_commas_and_units(1_024, "Read Bytes"), "1,024 B");
        assert_eq!(format_with_commas_and_units(512, "Write Bytes"), "512 B");
    }

    #[test]
    fn format_instruction_metric_uses_inst_suffix() {
        // Any metric name that does *not* contain "Bytes" falls through to the
        // `inst.` branch — including the empty string, which the test below
        // confirms for defensive coverage.
        assert_eq!(
            format_with_commas_and_units(42, "CPU Instructions"),
            "42 inst."
        );
        assert_eq!(format_with_commas_and_units(42, ""), "42 inst.");
    }

    #[test]
    fn format_u64_max_is_representable() {
        // `u64::MAX` = 18_446_744_073_709_551_615 — 20 decimal digits.
        // The modular counter must cycle 6 full times (6 commas) without
        // overflow.  Verifying this guards against any hypothetical integer
        // wraparound in a hand-rolled counter.
        let formatted = format_with_commas_and_units(u64::MAX, "CPU Instructions");
        assert_eq!(formatted, "18,446,744,073,709,551,615 inst.");
    }

    // ── evaluate_check ────────────────────────────────────────────────────
    //
    // `evaluate_check(value: u32, limit: Option<u64>)` resolves a single
    // metric pass/fail decision.
    //
    // The widening cast:
    //   `u64::from(value) <= limit_value`
    //
    // `u32` can represent values up to 4_294_967_295 (2^32 − 1).  The limit
    // stored in `budget.toml` is `u64`, which can represent values up to
    // 2^64 − 1 — large enough to hold any conceivable resource budget.
    // Using `u64::from(value)` (rather than `value as u64`) makes the
    // zero-extension explicit and is guaranteed not to change sign: a `u32`
    // has no sign bit, so the widening always fills the upper 32 bits with
    // zeros.
    //
    // The comparison is *inclusive* (`<=`), matching the documented semantics
    // "value must not exceed the limit".  A value that equals the limit
    // exactly is a pass.

    #[test]
    fn evaluate_check_no_limit_returns_none_pair() {
        // When no limit is configured the function returns `(None, None)`:
        // the metric is measured and reported but not checked.
        let (limit, pass) = evaluate_check(1_000, None);
        assert_eq!(limit, None);
        assert_eq!(pass, None);
    }

    #[test]
    fn evaluate_check_value_strictly_below_limit_passes() {
        // The common case: measurement is comfortably under the budget.
        let (limit, pass) = evaluate_check(999, Some(1_000));
        assert_eq!(limit, Some(1_000));
        assert_eq!(pass, Some(true));
    }

    #[test]
    fn evaluate_check_value_exactly_at_limit_passes_inclusive() {
        // The inclusive boundary: value == limit is still a pass.
        // This is the `<=` not `<` contract.
        let (limit, pass) = evaluate_check(1_000, Some(1_000));
        assert_eq!(limit, Some(1_000));
        assert_eq!(pass, Some(true));
    }

    #[test]
    fn evaluate_check_value_one_above_limit_fails() {
        // The first failing value: limit + 1.  Because the comparison is
        // inclusive, crossing the boundary by a single unit triggers failure.
        let (limit, pass) = evaluate_check(1_001, Some(1_000));
        assert_eq!(limit, Some(1_000));
        assert_eq!(pass, Some(false));
    }

    #[test]
    fn evaluate_check_u32_max_against_u64_limit_above_u32_max() {
        // This test exercises the widening cast directly.
        //
        // `u32::MAX` = 4_294_967_295.  If the cast were truncating or
        // sign-extending, the comparison against a `u64` limit that is larger
        // than `u32::MAX` could silently produce the wrong result.
        //
        // `u64::from(u32::MAX)` = 4_294_967_295_u64, which is strictly less
        // than `u32::MAX as u64 + 1` = 4_294_967_296_u64 → pass expected.
        let (limit, pass) = evaluate_check(u32::MAX, Some(u64::from(u32::MAX) + 1));
        assert_eq!(limit, Some(4_294_967_296));
        assert_eq!(pass, Some(true));
    }

    #[test]
    fn evaluate_check_u32_max_at_its_own_widened_limit_passes() {
        // `u32::MAX` compared against a limit that equals its widened value —
        // the inclusive boundary for the maximum representable measurement.
        let (limit, pass) = evaluate_check(u32::MAX, Some(u64::from(u32::MAX)));
        assert_eq!(limit, Some(4_294_967_295));
        assert_eq!(pass, Some(true));
    }

    #[test]
    fn evaluate_check_zero_value_against_zero_limit_passes() {
        // Edge case: a zero measurement against a zero limit.
        // `0 <= 0` is true, so the result is a pass.
        let (limit, pass) = evaluate_check(0, Some(0));
        assert_eq!(limit, Some(0));
        assert_eq!(pass, Some(true));
    }

    #[test]
    fn evaluate_check_nonzero_value_against_zero_limit_fails() {
        // Any non-zero measurement against a zero limit is a failure,
        // since `1 <= 0` is false.  Useful for asserting "no writes allowed".
        let (limit, pass) = evaluate_check(1, Some(0));
        assert_eq!(limit, Some(0));
        assert_eq!(pass, Some(false));
    }
}
