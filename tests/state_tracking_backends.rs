//! Issue #1011 — pin `LinearStateTracker` and `HashedStateTracker` for the
//! same recording sequence and prove `into_hashed` preserves state.
//!
//! Both backends implement [`StateTracker`] but keep entries in different
//! containers (`Vec<(String, CostSnapshot)>` vs `HashMap<String, CostSnapshot>`).
//! The docs on `src/state_tracking.rs:140-294` promise the two are
//! indistinguishable through the trait API; the assertions below hold that
//! promise for the operation sequences the rest of the workspace actually
//! exercises (unique inserts, overwrites, lookups of absent keys, clear, and
//! reinsert). They also prove `LinearStateTracker::into_hashed`
//! (`src/state_tracking.rs:169`) keeps the **last** snapshot for a repeated
//! operation name, matching what `HashedStateTracker::record` would have
//! written had the hashed backend been used from the start.

use soroban_budget_assert_core::{
    CostSnapshot, HashedStateTracker, LinearStateTracker, StateTracker,
};

/// Runs a deterministic insert sequence on `tracker`, so the same recipe can
/// drive either backend without duplicating the sequence literal.
fn apply_unique_records<T: StateTracker>(tracker: &mut T, n: u64) {
    for i in 0..n {
        tracker.record(&format!("op_{i}"), CostSnapshot::new(i * 10, i * 4));
    }
}

/// Applies an interleaved insert/overwrite sequence: each op is recorded
/// twice with different snapshots so the second value must win.
fn apply_overwrite_sequence<T: StateTracker>(tracker: &mut T) {
    let names = ["alpha", "beta", "gamma", "delta"];
    for (idx, name) in names.iter().enumerate() {
        tracker.record(name, CostSnapshot::new(idx as u64, idx as u64));
    }
    for (idx, name) in names.iter().enumerate() {
        tracker.record(name, CostSnapshot::new(100 + idx as u64, 200 + idx as u64));
    }
}

/// Compares the aggregate view of two trackers that must be indistinguishable
/// through the `StateTracker` API — `ops()` iteration order is
/// implementation-defined, so the names are sorted before comparison.
fn assert_trackers_equivalent<A: StateTracker, B: StateTracker>(a: &A, b: &B) {
    assert_eq!(a.len(), b.len(), "distinct-op count must match");
    assert_eq!(a.is_empty(), b.is_empty());
    assert_eq!(a.total_cpu(), b.total_cpu());
    assert_eq!(a.total_memory(), b.total_memory());
    let mut a_ops = a.ops();
    let mut b_ops = b.ops();
    a_ops.sort();
    b_ops.sort();
    assert_eq!(a_ops, b_ops, "recorded op names must match");
    for name in &a_ops {
        assert_eq!(
            a.lookup(name),
            b.lookup(name),
            "per-op snapshot for `{name}` must match",
        );
        assert_eq!(a.contains(name), b.contains(name));
    }
}

#[test]
fn unique_recordings_yield_identical_aggregates() {
    let mut linear = LinearStateTracker::new();
    let mut hashed = HashedStateTracker::new();
    apply_unique_records(&mut linear, 40);
    apply_unique_records(&mut hashed, 40);
    assert_trackers_equivalent(&linear, &hashed);
    assert_eq!(linear.len(), 40);
    assert_eq!(hashed.len(), 40);
    // Cross-check the totals against an independent recomputation so a
    // bug in either backend's aggregation surfaces rather than cancelling
    // out under the equivalence check.
    let expected_cpu: u64 = (0..40).map(|i| i * 10).sum();
    let expected_mem: u64 = (0..40).map(|i| i * 4).sum();
    assert_eq!(linear.total_cpu(), expected_cpu);
    assert_eq!(hashed.total_cpu(), expected_cpu);
    assert_eq!(linear.total_memory(), expected_mem);
    assert_eq!(hashed.total_memory(), expected_mem);
}

#[test]
fn overwrites_replace_the_previous_snapshot_in_both() {
    let mut linear = LinearStateTracker::new();
    let mut hashed = HashedStateTracker::new();
    apply_overwrite_sequence(&mut linear);
    apply_overwrite_sequence(&mut hashed);
    assert_trackers_equivalent(&linear, &hashed);
    assert_eq!(linear.len(), 4);
    // The second pass writes (100+idx, 200+idx) — the equivalence check
    // alone would pass if BOTH trackers kept the first value, so pin the
    // final snapshot directly for at least one op.
    assert_eq!(linear.lookup("alpha"), Some(&CostSnapshot::new(100, 200)),);
    assert_eq!(hashed.lookup("alpha"), Some(&CostSnapshot::new(100, 200)),);
}

#[test]
fn lookup_of_unknown_op_agrees_in_both_backends() {
    let mut linear = LinearStateTracker::new();
    let mut hashed = HashedStateTracker::new();
    linear.record("present", CostSnapshot::new(1, 1));
    hashed.record("present", CostSnapshot::new(1, 1));
    for missing in ["absent", "", "PRESENT", "present ", " present"] {
        assert_eq!(
            linear.lookup(missing),
            None,
            "`{missing}` must be absent (linear)"
        );
        assert_eq!(
            hashed.lookup(missing),
            None,
            "`{missing}` must be absent (hashed)"
        );
        assert!(!linear.contains(missing));
        assert!(!hashed.contains(missing));
    }
}

#[test]
fn clear_resets_both_backends_to_the_empty_state() {
    let mut linear = LinearStateTracker::new();
    let mut hashed = HashedStateTracker::new();
    apply_unique_records(&mut linear, 8);
    apply_unique_records(&mut hashed, 8);
    linear.clear();
    hashed.clear();
    assert_eq!(linear.len(), 0);
    assert_eq!(hashed.len(), 0);
    assert!(linear.is_empty());
    assert!(hashed.is_empty());
    assert_eq!(linear.total_cpu(), 0);
    assert_eq!(hashed.total_cpu(), 0);
    assert_eq!(linear.total_memory(), 0);
    assert_eq!(hashed.total_memory(), 0);
    assert!(linear.ops().is_empty());
    assert!(hashed.ops().is_empty());
}

#[test]
fn reinsert_after_clear_reproduces_the_original_state() {
    let mut linear = LinearStateTracker::new();
    let mut hashed = HashedStateTracker::new();
    apply_unique_records(&mut linear, 12);
    apply_unique_records(&mut hashed, 12);
    let before_linear = linear.lookup("op_5").copied();
    let before_hashed = hashed.lookup("op_5").copied();
    linear.clear();
    hashed.clear();
    apply_unique_records(&mut linear, 12);
    apply_unique_records(&mut hashed, 12);
    assert_eq!(linear.lookup("op_5").copied(), before_linear);
    assert_eq!(hashed.lookup("op_5").copied(), before_hashed);
    assert_trackers_equivalent(&linear, &hashed);
}

#[test]
fn into_hashed_preserves_last_snapshot_for_each_name() {
    // `LinearStateTracker` keeps insertion order; a repeated op overwrites in
    // place (`src/state_tracking.rs:181`). `into_hashed` walks those entries
    // in vector order and calls `record`, so the **last** snapshot must win —
    // matching what a from-scratch hashed tracker would have recorded.
    let mut linear = LinearStateTracker::new();
    linear.record("x", CostSnapshot::new(1, 1));
    linear.record("y", CostSnapshot::new(2, 2));
    linear.record("x", CostSnapshot::new(3, 3));
    linear.record("z", CostSnapshot::new(4, 4));
    linear.record("y", CostSnapshot::new(5, 5));

    let converted: HashedStateTracker = linear.into_hashed();

    let mut from_scratch = HashedStateTracker::new();
    from_scratch.record("x", CostSnapshot::new(1, 1));
    from_scratch.record("y", CostSnapshot::new(2, 2));
    from_scratch.record("x", CostSnapshot::new(3, 3));
    from_scratch.record("z", CostSnapshot::new(4, 4));
    from_scratch.record("y", CostSnapshot::new(5, 5));

    assert_eq!(converted.len(), 3);
    assert_eq!(converted.lookup("x"), Some(&CostSnapshot::new(3, 3)));
    assert_eq!(converted.lookup("y"), Some(&CostSnapshot::new(5, 5)));
    assert_eq!(converted.lookup("z"), Some(&CostSnapshot::new(4, 4)));
    assert_eq!(converted.total_cpu(), 3 + 5 + 4);
    assert_eq!(converted.total_memory(), 3 + 5 + 4);

    let mut converted_ops = converted.ops();
    let mut scratch_ops = from_scratch.ops();
    converted_ops.sort();
    scratch_ops.sort();
    assert_eq!(converted_ops, scratch_ops);
    assert_eq!(converted.total_cpu(), from_scratch.total_cpu());
    assert_eq!(converted.total_memory(), from_scratch.total_memory());
}

#[test]
fn into_hashed_on_empty_linear_yields_empty_hashed() {
    let linear = LinearStateTracker::new();
    let converted = linear.into_hashed();
    assert!(converted.is_empty());
    assert_eq!(converted.len(), 0);
    assert_eq!(converted.total_cpu(), 0);
    assert_eq!(converted.total_memory(), 0);
    assert!(converted.ops().is_empty());
}

#[test]
fn distinct_key_counts_scale_aggregates_identically() {
    for n in [1usize, 2, 5, 17, 64, 129] {
        let mut linear = LinearStateTracker::with_capacity(n);
        let mut hashed = HashedStateTracker::with_capacity(n);
        apply_unique_records(&mut linear, n as u64);
        apply_unique_records(&mut hashed, n as u64);
        assert_trackers_equivalent(&linear, &hashed);
        let converted = linear.into_hashed();
        assert_trackers_equivalent(&converted, &hashed);
    }
}

#[test]
fn unicode_operation_names_are_tracked_identically() {
    let names = ["取引", "élan", "naïve", "🚀op", "نظام"];
    let mut linear = LinearStateTracker::new();
    let mut hashed = HashedStateTracker::new();
    for (idx, name) in names.iter().enumerate() {
        let snap = CostSnapshot::new(idx as u64 * 7, idx as u64 * 3);
        linear.record(name, snap);
        hashed.record(name, snap);
    }
    assert_trackers_equivalent(&linear, &hashed);
    for name in names {
        assert_eq!(linear.lookup(name), hashed.lookup(name));
    }
}
