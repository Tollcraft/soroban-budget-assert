//! Budget limit enforcement for Soroban resource metering.
//!
//! Each public function in this module takes the measured resource value from
//! a simulation response and compares it against the configured budget limit.
//! A value that equals the limit is still accepted; only a strict exceedance
//! returns an error.
//!
//! The error strings are deliberately human-readable so they can be surfaced
//! directly in CLI output and test assertions without further formatting.

/// Assert that `value` does not exceed `max`, returning a descriptive error
/// when it does.
///
/// The generic bound requires `T: PartialOrd + Display` so the same helper
/// works for both `u32` and `u64` comparisons without duplicating logic.
/// All callers widen their `u32` inputs to `u64` before calling this function
/// to keep the comparisons consistent and avoid silent truncation.
fn check_bounds<T: PartialOrd + std::fmt::Display>(
    value: T,
    max: T,
    name: &str,
) -> Result<(), String> {
    if value > max {
        Err(format!("{} {} exceeds limit {}", name, value, max))
    } else {
        Ok(())
    }
}

/// Check that the measured CPU instruction count is within `limit`.
///
/// `instructions` is widened from `u32` to `u64` before the comparison so
/// that limits larger than `u32::MAX` (which Soroban allows) are handled
/// correctly without overflow.
pub fn check_cpu_instructions(instructions: u32, limit: u64) -> Result<(), String> {
    check_bounds(u64::from(instructions), limit, "CPU Instructions")
}

/// Check that the number of ledger bytes read by the transaction is within
/// `limit`.
///
/// The widening from `u32` to `u64` mirrors [`check_cpu_instructions`] and
/// ensures the comparison is lossless even when `limit` exceeds `u32::MAX`.
pub fn check_read_bytes(bytes: u32, limit: u64) -> Result<(), String> {
    check_bounds(u64::from(bytes), limit, "Read Bytes")
}

/// Check that the number of ledger bytes written by the transaction is within
/// `limit`.
///
/// The widening from `u32` to `u64` mirrors [`check_cpu_instructions`] and
/// ensures the comparison is lossless even when `limit` exceeds `u32::MAX`.
pub fn check_write_bytes(bytes: u32, limit: u64) -> Result<(), String> {
    check_bounds(u64::from(bytes), limit, "Write Bytes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_under_limit_passes() {
        assert!(check_cpu_instructions(100_000, 1_000_000).is_ok());
    }

    #[test]
    fn cpu_at_limit_passes() {
        assert!(check_cpu_instructions(1_000_000, 1_000_000).is_ok());
    }

    #[test]
    fn cpu_over_limit_fails() {
        let err = check_cpu_instructions(1_500_000, 1_000_000).unwrap_err();
        assert!(err.contains("CPU Instructions"));
        assert!(err.contains("1,500,000") || err.contains("1500000"));
    }

    #[test]
    fn read_bytes_under_limit_passes() {
        assert!(check_read_bytes(500, 2_048).is_ok());
    }

    #[test]
    fn read_bytes_over_limit_fails() {
        assert!(check_read_bytes(3_000, 2_048).is_err());
    }

    #[test]
    fn write_bytes_under_limit_passes() {
        assert!(check_write_bytes(1_000, 10_000).is_ok());
    }

    #[test]
    fn write_bytes_over_limit_fails() {
        let err = check_write_bytes(20_000, 10_000).unwrap_err();
        assert!(err.contains("Write Bytes"));
    }

    #[test]
    fn u32_max_at_u64_limit_passes() {
        assert!(check_cpu_instructions(u32::MAX, u64::from(u32::MAX)).is_ok());
    }

    #[test]
    fn zero_values_always_pass() {
        assert!(check_read_bytes(0, 0).is_ok());
        assert!(check_write_bytes(0, 0).is_ok());
        assert!(check_cpu_instructions(0, 0).is_ok());
    }

    #[test]
    fn error_message_format() {
        let err = check_cpu_instructions(500, 100).unwrap_err();
        assert_eq!(err, "CPU Instructions 500 exceeds limit 100");
    }
}
