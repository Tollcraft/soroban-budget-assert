//! Integration tests for issue #1014: measured `u32` values are widened to
//! `u64` before being compared against configured limits, so a limit above
//! `u32::MAX` is evaluated without truncation or overflow.
//!
//! A regression that narrowed the limit back to 32 bits would turn these
//! limits into small numbers and produce false violations.

#[path = "../src/limit_checks.rs"]
mod limit_checks;

use limit_checks::{check_cpu_instructions, check_read_bytes, check_write_bytes};

/// The smallest limit that cannot be represented in `u32`.
const FIRST_LIMIT_ABOVE_U32_MAX: u64 = u32::MAX as u64 + 1;

#[test]
fn cpu_limit_just_above_u32_max_cannot_be_reached() {
    assert!(check_cpu_instructions(u32::MAX, FIRST_LIMIT_ABOVE_U32_MAX).is_ok());
    assert!(check_cpu_instructions(1_000_000, FIRST_LIMIT_ABOVE_U32_MAX).is_ok());
}

#[test]
fn read_and_write_limits_just_above_u32_max_cannot_be_reached() {
    assert!(check_read_bytes(u32::MAX, FIRST_LIMIT_ABOVE_U32_MAX).is_ok());
    assert!(check_write_bytes(u32::MAX, FIRST_LIMIT_ABOVE_U32_MAX).is_ok());
}

#[test]
fn u64_max_limit_accepts_every_measurable_value_without_overflow() {
    assert!(check_cpu_instructions(u32::MAX, u64::MAX).is_ok());
    assert!(check_read_bytes(u32::MAX, u64::MAX).is_ok());
    assert!(check_write_bytes(u32::MAX, u64::MAX).is_ok());
    assert!(check_cpu_instructions(0, u64::MAX).is_ok());
}

#[test]
fn u32_boundary_is_still_enforced_exactly() {
    let limit = u64::from(u32::MAX);
    assert!(check_cpu_instructions(u32::MAX, limit).is_ok());

    let err = check_cpu_instructions(u32::MAX, limit - 1).unwrap_err();
    assert!(
        err.contains(&(u32::MAX as u64).to_string()),
        "error must report the full widened measurement, got: {err}"
    );
    assert!(
        err.contains(&(limit - 1).to_string()),
        "error must report the full widened limit, got: {err}"
    );
}
