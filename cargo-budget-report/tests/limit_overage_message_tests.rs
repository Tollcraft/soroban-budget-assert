#[path = "../src/limit_checks.rs"]
mod limit_checks;

use limit_checks::{check_cpu_instructions, check_read_bytes, check_write_bytes};

#[test]
fn test_cpu_instructions_overage_formatting() {
    let err = check_cpu_instructions(1200, 1000).unwrap_err();
    assert_eq!(
        err,
        "CPU Instructions 1200 exceeds limit 1000 by 200 (+20.0%)"
    );
}

#[test]
fn test_read_bytes_overage_formatting() {
    let err = check_read_bytes(3000, 2000).unwrap_err();
    assert_eq!(err, "Read Bytes 3000 exceeds limit 2000 by 1000 (+50.0%)");
}

#[test]
fn test_write_bytes_one_unit_overage() {
    let err = check_write_bytes(1001, 1000).unwrap_err();
    assert_eq!(err, "Write Bytes 1001 exceeds limit 1000 by 1 (+0.1%)");
}

#[test]
fn test_zero_limit_avoids_division_by_zero() {
    let err = check_read_bytes(50, 0).unwrap_err();
    assert_eq!(err, "Read Bytes 50 exceeds limit 0 by 50 (+inf%)");
}

#[test]
fn test_large_u64_values_prevent_overflow() {
    let err = check_cpu_instructions(u32::MAX, 100).unwrap_err();
    let expected_delta = u64::from(u32::MAX) - 100;
    let expected_pct = (expected_delta as f64 / 100.0) * 100.0;
    assert_eq!(
        err,
        format!(
            "CPU Instructions {} exceeds limit 100 by {} (+{:.1}%)",
            u32::MAX,
            expected_delta,
            expected_pct
        )
    );
}
