//! Integration tests for issue #1023:
//! Support package-qualified function names in tolerance lookups, checking the qualified
//! name first before falling back to bare function name.

#[path = "../src/compare.rs"]
mod compare;

use compare::{check_against_baseline, Baseline, BaselineEntry, Measurement, Tolerance};
use std::collections::BTreeMap;

fn make_measurement(cpu: u64) -> Measurement {
    Measurement {
        cpu_instructions: cpu,
        read_bytes: 100,
        write_bytes: 100,
    }
}

#[test]
fn test_package_qualified_tolerance_override() {
    let mut baseline_entries = BTreeMap::new();
    baseline_entries.insert(
        "pkg-a::transfer".to_string(),
        BaselineEntry {
            cpu_instructions: 1000,
            read_bytes: 100,
            write_bytes: 100,
        },
    );
    baseline_entries.insert(
        "pkg-b::transfer".to_string(),
        BaselineEntry {
            cpu_instructions: 1000,
            read_bytes: 100,
            write_bytes: 100,
        },
    );
    let baseline = Baseline {
        entries: baseline_entries,
    };

    // Both measured at 1300 (+30% increase)
    let mut current = BTreeMap::new();
    let mut pkg_a_fns = BTreeMap::new();
    pkg_a_fns.insert("transfer".to_string(), make_measurement(1300));
    current.insert("pkg-a".to_string(), pkg_a_fns);

    let mut pkg_b_fns = BTreeMap::new();
    pkg_b_fns.insert("transfer".to_string(), make_measurement(1300));
    current.insert("pkg-b".to_string(), pkg_b_fns);

    let default_tolerance = Tolerance::new(0.10); // 10%

    // Override only `pkg-a::transfer` with 50% tolerance (0.50).
    // `pkg-b::transfer` has no specific override and should use default tolerance (10%).
    let mut tolerance_overrides = BTreeMap::new();
    tolerance_overrides.insert("pkg-a::transfer".to_string(), Tolerance::new(0.50));

    let report =
        check_against_baseline(&baseline, &current, default_tolerance, &tolerance_overrides);

    // Find pkg-a comparison
    let pkg_a_comp = report
        .compared
        .iter()
        .find(|f| f.package == "pkg-a" && f.function == "transfer")
        .expect("pkg-a comparison must exist");

    // With 50% tolerance, 30% increase should pass
    assert!(!pkg_a_comp.has_regressions());
    let cpu_metric = pkg_a_comp
        .metrics
        .iter()
        .find(|m| m.metric == compare::MetricKind::CpuInstructions)
        .unwrap();
    assert_eq!(cpu_metric.verdict, compare::Verdict::Pass);
    assert!((cpu_metric.tolerance.value - 0.50).abs() < f64::EPSILON);

    // Find pkg-b comparison
    let pkg_b_comp = report
        .compared
        .iter()
        .find(|f| f.package == "pkg-b" && f.function == "transfer")
        .expect("pkg-b comparison must exist");

    // With 10% default tolerance, 30% increase is a regression
    assert!(pkg_b_comp.has_regressions());
    let cpu_metric_b = pkg_b_comp
        .metrics
        .iter()
        .find(|m| m.metric == compare::MetricKind::CpuInstructions)
        .unwrap();
    assert_eq!(cpu_metric_b.verdict, compare::Verdict::Regression);
    assert!((cpu_metric_b.tolerance.value - 0.10).abs() < f64::EPSILON);
}

#[test]
fn test_bare_function_override_matches_any_package() {
    let mut baseline_entries = BTreeMap::new();
    baseline_entries.insert(
        "pkg-c::swap".to_string(),
        BaselineEntry {
            cpu_instructions: 1000,
            read_bytes: 100,
            write_bytes: 100,
        },
    );
    let baseline = Baseline {
        entries: baseline_entries,
    };

    let mut current = BTreeMap::new();
    let mut pkg_c_fns = BTreeMap::new();
    pkg_c_fns.insert("swap".to_string(), make_measurement(1200));
    current.insert("pkg-c".to_string(), pkg_c_fns);

    let default_tolerance = Tolerance::new(0.05); // 5%

    // Bare function override
    let mut tolerance_overrides = BTreeMap::new();
    tolerance_overrides.insert("swap".to_string(), Tolerance::new(0.25)); // 25%

    let report =
        check_against_baseline(&baseline, &current, default_tolerance, &tolerance_overrides);

    let comp = &report.compared[0];
    assert!(!comp.has_regressions());
    let cpu = comp
        .metrics
        .iter()
        .find(|m| m.metric == compare::MetricKind::CpuInstructions)
        .unwrap();
    assert_eq!(cpu.verdict, compare::Verdict::Pass);
    assert!((cpu.tolerance.value - 0.25).abs() < f64::EPSILON);
}

#[test]
fn test_qualified_override_beats_bare_function_override() {
    let mut baseline_entries = BTreeMap::new();
    baseline_entries.insert(
        "pkg-d::mint".to_string(),
        BaselineEntry {
            cpu_instructions: 1000,
            read_bytes: 100,
            write_bytes: 100,
        },
    );
    let baseline = Baseline {
        entries: baseline_entries,
    };

    let mut current = BTreeMap::new();
    let mut pkg_d_fns = BTreeMap::new();
    pkg_d_fns.insert("mint".to_string(), make_measurement(1150));
    current.insert("pkg-d".to_string(), pkg_d_fns);

    let default_tolerance = Tolerance::new(0.05); // 5%

    // Both bare and qualified override: bare is 20% (would pass), qualified is 10% (should regress)
    let mut tolerance_overrides = BTreeMap::new();
    tolerance_overrides.insert("mint".to_string(), Tolerance::new(0.20));
    tolerance_overrides.insert("pkg-d::mint".to_string(), Tolerance::new(0.10));

    let report =
        check_against_baseline(&baseline, &current, default_tolerance, &tolerance_overrides);

    let comp = &report.compared[0];
    assert!(comp.has_regressions());
    let cpu = comp
        .metrics
        .iter()
        .find(|m| m.metric == compare::MetricKind::CpuInstructions)
        .unwrap();
    assert_eq!(cpu.verdict, compare::Verdict::Regression);
    assert!((cpu.tolerance.value - 0.10).abs() < f64::EPSILON);
}
