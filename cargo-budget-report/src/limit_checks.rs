//! Boundary checking utilities for resource limit enforcement.
//!
//! Provides a generic `check_bounds` function and convenience wrappers
//! for CPU instructions, read bytes, and write bytes. Each wrapper
//! converts the `u32` measured value to `u64` before comparing against
//! the configured `u64` limit.
//!
//! # Examples
//!
//! ```rust
//! use limit_checks::check_cpu_instructions;
//!
//! // Within the limit
//! assert!(check_cpu_instructions(100_000, 1_000_000).is_ok());
//!
//! // Over the limit
//! assert!(check_cpu_instructions(1_500_000, 1_000_000).is_err());
//! ```

use std::fmt::Display;

/// Error type for boundary check failures.
#[derive(Debug, PartialEq)]
pub enum CheckError {
    /// The measured value exceeds the configured limit.
    ExceedsLimit { metric: String, value: u64, limit: u64 },
}

impl std::fmt::Display for CheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckError::ExceedsLimit { metric, value, limit } => {
                write!(f, "{metric} {value} exceeds limit {limit}")
            }
        }
    }
}

/// Generic boundary check: returns `Ok(())` if `value <= max`, otherwise
/// returns an error describing the overflow.
pub fn check_bounds<T: PartialOrd + Display>(value: T, max: T, name: &str) -> Result<(), CheckError> {
    if value > max {
        Err(CheckError::ExceedsLimit {
            metric: name.to_string(),
            value: value.to_string(),
            limit: max.to_string(),
        })
    } else {
        Ok(())
    }
}

/// Check CPU instruction count against a limit.
pub fn check_cpu_instructions(instructions: u32, limit: u64) -> Result<(), CheckError> {
    check_bounds(u64::from(instructions), limit, "CPU Instructions")
}

/// Check read byte count against a limit.
pub fn check_read_bytes(bytes: u32, limit: u64) -> Result<(), CheckError> {
    check_bounds(u64::from(bytes), limit, "Read Bytes")
}

/// Check write byte count against a limit.
pub fn check_write_bytes(bytes: u32, limit: u64) -> Result<(), CheckError> {
    check_bounds(u64::from(bytes), limit, "Write Bytes")
}

/// Trait for types that can be checked against a limit.
pub trait LimitCheck {
    /// Check the value against the given limit.
    fn check_against(&self, limit: u64, name: &str) -> Result<(), CheckError>;
}

impl LimitCheck for u32 {
    fn check_against(&self, limit: u64, name: &str) -> Result<(), CheckError> {
        check_bounds(u64::from(*self), limit, name)
    }
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
        assert!(err.to_string().contains("CPU Instructions"));
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
        assert!(err.to_string().contains("Write Bytes"));
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
        assert_eq!(err.to_string(), "CPU Instructions 500 exceeds limit 100");
    }

    #[test]
    fn check_bounds_generic_passes() {
        assert!(check_bounds(50u32, 100u32, "Test").is_ok());
    }

    #[test]
    fn check_bounds_generic_fails() {
        let err = check_bounds(150u32, 100u32, "Test").unwrap_err();
        assert_eq!(err.to_string(), "Test 150 exceeds limit 100");
    }

    #[test]
    fn limit_check_trait_works() {
        let value: u32 = 500;
        assert!(value.check_against(1000, "Test").is_ok());
        assert!(value.check_against(100, "Test").is_err());
    }
}
