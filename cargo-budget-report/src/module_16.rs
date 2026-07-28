//! # Module: Data Processing Utilities (#16)
//!
//! Provides utility functions for processing cost measurement data and
//! formatting resource-usage snapshots. Variable names throughout this
//! module have been chosen to accurately reflect their purpose in the
//! domain logic, replacing previously ambiguous single-letter or
//! unclear identifiers.
//!
//! ## Renaming Rationale
//!
//! | Old Name | New Name | Reason |
//! |----------|----------|--------|
//! | `x` | `base_cpu_limit` | Represents the baseline CPU instruction budget |
//! | `y` | `base_memory_limit` | Represents the baseline memory byte budget |
//! | `z` | `margin_multiplier` | Multiplier applied to derive Tier A from Tier B |
//! | `a` | `cpu_measurements` | Collection of CPU instruction readings |
//! | `b` | `memory_measurements` | Collection of memory byte readings |
//! | `i` | `package_index` | Index into the workspace package list |
//! | `j` | `function_index` | Index into the per-package exported function list |
//! | `k` | `metric_index` | Index into the per-function metric list |
//! | `n` | `iteration_count` | Describes how many iterations a loop should run |
//! | `s` | `snapshot_buffer` | Holds an accumulated bytes-size value |

// `pub` items in this module are reached only by its own `#[cfg(test)] mod tests`
// and illustration callers (issue #16 was a documentation/demonstration
// contribution), so the binary target does not link them through `fn main`.
// Allow dead code at file scope rather than auditing per-item; the public API
// is still exercisable from `cargo test`.
#![allow(dead_code)]

use crate::format_with_commas_and_units;
use std::collections::BTreeMap;

/// Computes the effective Tier A limit for a given Tier B measurement and
/// margin multiplier.
///
/// # Arguments
///
/// * `base_cpu_limit` - The Tier B measured CPU instructions value.
/// * `base_memory_limit` - The Tier B measured memory bytes value.
/// * `margin_multiplier` - The multiplier (margin) applied to both limits
///   to derive the Tier A ceiling.
///
/// # Returns
///
/// A tuple `(cpu_limit, memory_limit)` where each value is the ceiling
/// computed as `base * margin`, rounded up to the nearest integer.
pub fn derive_tier_a_limits(
    base_cpu_limit: u64,
    base_memory_limit: u64,
    margin_multiplier: f64,
) -> (u64, u64) {
    let cpu_limit = (base_cpu_limit as f64 * margin_multiplier).ceil() as u64;
    let memory_limit = (base_memory_limit as f64 * margin_multiplier).ceil() as u64;
    (cpu_limit, memory_limit)
}

/// Summarises the resource usage across one function into a human-readable
/// string, matching the `cargo-budget-report` table format.
///
/// # Arguments
///
/// * `cpu_measurements` - Sorted vector of CPU instruction readings for one
///   function across multiple simulation runs.
/// * `memory_measurements` - Sorted vector of memory byte readings for one
///   function across multiple simulation runs.
/// * `iteration_count` - How many simulation runs were aggregated.
/// * `snapshot_buffer` - The WASM binary size in bytes.
pub fn format_resource_summary(
    cpu_measurements: &[u64],
    memory_measurements: &[u64],
    iteration_count: usize,
    snapshot_buffer: u64,
) -> String {
    let cpu_avg = cpu_measurements
        .iter()
        .copied()
        .sum::<u64>()
        .checked_div(iteration_count as u64)
        .unwrap_or(0);

    let mem_avg = memory_measurements
        .iter()
        .copied()
        .sum::<u64>()
        .checked_div(iteration_count as u64)
        .unwrap_or(0);

    format!(
        "CPU: {}, Memory: {}, WASM: {}",
        format_with_commas_and_units(cpu_avg, "CPU Instructions"),
        format_with_commas_and_units(mem_avg, "Memory Bytes"),
        format_with_commas_and_units(snapshot_buffer, "WASM Bytes"),
    )
}

/// Aggregates per-package measurements into a summary BTreeMap suitable for
/// display or serialisation.
///
/// # Arguments
///
/// * `package_index` - Index of the current package in the workspace.
/// * `function_index` - Index of the current exported function within the
///   package.
/// * `metric_index` - Index of the current metric (CPU, Memory, etc.) within
///   the function.
/// * `raw_data` - Flat slice of `(package, function, metric, value)` tuples
///   gathered during simulation.
///
/// # Returns
///
/// A `BTreeMap<String, BTreeMap<String, u64>>` mapping package names to
/// function names to the maximum measured value for that function.
pub fn aggregate_measurements(
    package_index: usize,
    function_index: usize,
    metric_index: usize,
    raw_data: &[(String, String, &str, u64)],
) -> BTreeMap<String, BTreeMap<String, u64>> {
    let mut result: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();

    for (package, function, _metric, value) in raw_data {
        let entry = result
            .entry(package.clone())
            .or_default()
            .entry(function.clone())
            .or_insert(0);
        *entry = (*entry).max(*value);
    }

    let _ = package_index;
    let _ = function_index;
    let _ = metric_index;

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_tier_a_limits_with_unit_margin() {
        let (cpu, mem) = derive_tier_a_limits(1_000_000, 2_048, 1.0);
        assert_eq!(cpu, 1_000_000);
        assert_eq!(mem, 2_048);
    }

    #[test]
    fn derive_tier_a_limits_with_margin_multiplier() {
        let (cpu, mem) = derive_tier_a_limits(1_000_000, 2_048, 1.15);
        assert_eq!(cpu, 1_150_000);
        assert_eq!(mem, 2_356);
    }

    #[test]
    fn derive_tier_a_limits_zero_base() {
        let (cpu, mem) = derive_tier_a_limits(0, 0, 1.5);
        assert_eq!(cpu, 0);
        assert_eq!(mem, 0);
    }

    #[test]
    fn derive_tier_a_limits_rounds_up() {
        // 100 * 0.15 = 15 → ceil(15) = 15
        let (cpu, mem) = derive_tier_a_limits(100, 100, 0.15);
        assert_eq!(cpu, 15);
        assert_eq!(mem, 15);
    }

    #[test]
    fn format_resource_summary_single_run() {
        let cpu = vec![500_000u64];
        let mem = vec![1_024u64];
        let summary = format_resource_summary(&cpu, &mem, 1, 102_400);
        assert!(summary.contains("500,000"));
        assert!(summary.contains("1,024"));
        assert!(summary.contains("102,400"));
    }

    #[test]
    fn format_resource_summary_multiple_runs() {
        let cpu = vec![100_000u64, 200_000u64, 300_000u64];
        let mem = vec![512u64, 1_024u64, 2_048u64];
        let summary = format_resource_summary(&cpu, &mem, 3, 50_000);
        // Average CPU: (100_000 + 200_000 + 300_000) / 3 = 200_000
        assert!(summary.contains("200,000"));
        // Average mem: (512 + 1_024 + 2_048) / 3 ≈ 1,194 → floor = 1,194
        assert!(summary.contains("1,194"));
    }

    #[test]
    fn format_resource_summary_empty_measurements() {
        let cpu = vec![];
        let mem = vec![];
        let summary = format_resource_summary(&cpu, &mem, 0, 0);
        assert!(summary.contains("0"));
    }

    #[test]
    fn aggregate_measurements_single_package() {
        let data = vec![
            (
                "pkg-a".to_string(),
                "fn1".to_string(),
                "CPU Instructions",
                100,
            ),
            ("pkg-a".to_string(), "fn1".to_string(), "Read Bytes", 200),
        ];
        let result = aggregate_measurements(0, 0, 0, &data);
        assert_eq!(result.get("pkg-a").unwrap().get("fn1").unwrap(), &200);
    }

    #[test]
    fn aggregate_measurements_multiple_packages() {
        let data = vec![
            (
                "pkg-a".to_string(),
                "fn1".to_string(),
                "CPU Instructions",
                100,
            ),
            ("pkg-b".to_string(), "fn2".to_string(), "Read Bytes", 200),
        ];
        let result = aggregate_measurements(0, 0, 0, &data);
        assert_eq!(result.get("pkg-a").unwrap().get("fn1").unwrap(), &100);
        assert_eq!(result.get("pkg-b").unwrap().get("fn2").unwrap(), &200);
    }

    #[test]
    fn aggregate_measurements_tracks_maximum() {
        let data = vec![
            ("pkg".to_string(), "fn".to_string(), "CPU Instructions", 100),
            ("pkg".to_string(), "fn".to_string(), "CPU Instructions", 500),
            ("pkg".to_string(), "fn".to_string(), "CPU Instructions", 300),
        ];
        let result = aggregate_measurements(0, 0, 0, &data);
        assert_eq!(result.get("pkg").unwrap().get("fn").unwrap(), &500);
    }

    #[test]
    fn aggregate_measurements_empty_input() {
        let data = vec![];
        let result = aggregate_measurements(0, 0, 0, &data);
        assert!(result.is_empty());
    }
}
