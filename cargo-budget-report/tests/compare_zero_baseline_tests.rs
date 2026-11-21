//! Integration tests for issue #1015: a zero baseline has no defined
//! percentage change, so `MetricComparison::pct_change` returns `None` while
//! `abs_change` still reports the full absolute delta.

#[path = "../src/compare.rs"]
mod compare;

use compare::{MetricComparison, MetricKind, Tolerance, Verdict};

fn comparison(metric: MetricKind, baseline: u64, current: u64) -> MetricComparison {
    MetricComparison {
        metric,
        baseline,
        current,
        tolerance: Tolerance::new(compare::DEFAULT_TOLERANCE),
        verdict: Verdict::Pass,
    }
}

#[test]
fn zero_baseline_nonzero_current_has_no_percentage_change() {
    let c = comparison(MetricKind::CpuInstructions, 0, 1_000);
    assert_eq!(c.pct_change(), None);
}

#[test]
fn zero_baseline_nonzero_current_reports_the_full_absolute_delta() {
    let c = comparison(MetricKind::ReadBytes, 0, 4_096);
    assert_eq!(c.abs_change(), 4_096);
}

#[test]
fn zero_baseline_zero_current_has_neither_a_percentage_nor_an_absolute_change() {
    let c = comparison(MetricKind::WriteBytes, 0, 0);
    assert_eq!(c.pct_change(), None);
    assert_eq!(c.abs_change(), 0);
}

#[test]
fn zero_baseline_holds_for_every_metric_kind() {
    for metric in [
        MetricKind::CpuInstructions,
        MetricKind::ReadBytes,
        MetricKind::WriteBytes,
    ] {
        let c = comparison(metric, 0, u64::from(u32::MAX));
        assert_eq!(c.pct_change(), None, "metric {metric:?}");
        assert_eq!(c.abs_change(), i64::from(u32::MAX), "metric {metric:?}");
    }
}

#[test]
fn nonzero_baseline_still_yields_a_percentage() {
    let c = comparison(MetricKind::CpuInstructions, 1_000, 1_500);
    assert_eq!(c.pct_change(), Some(50.0));
    assert_eq!(c.abs_change(), 500);
}
