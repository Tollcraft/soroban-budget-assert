#![allow(dead_code)]

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

pub fn check_cpu_instructions(instructions: u32, limit: u64) -> Result<(), String> {
    check_bounds(u64::from(instructions), limit, "CPU Instructions")
}

pub fn check_read_bytes(bytes: u32, limit: u64) -> Result<(), String> {
    check_bounds(u64::from(bytes), limit, "Read Bytes")
}

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
