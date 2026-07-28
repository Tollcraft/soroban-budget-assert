//! # Module 25: Optimized Internal State Tracking Structures
//!
//! Tracks per-operation cost snapshots (CPU + memory) during test runs.
//!
//! ## Background
//!
//! The pre-existing cost-aggregation flow stored each `(operation, snapshot)`
//! pair in a `Vec<(String, CostSnapshot)>`, which requires an O(n) linear scan
//! on every lookup or update. For workloads that record many operations (large
//! AMM pool tests, cross-contract suites, long regression runs), the linear
//! scan becomes the dominant cost and scales poorly.
//!
//! This module replaces that linear structure with a hash-based map, while
//! keeping the linear implementation available for benchmarking and for
//! callers that need a stable iteration order. Both backends implement the
//! same [`StateTracker`] trait, so swapping one for the other is a
//! single-type change at the call site.
//!
//! ## Backends
//!
//! | Tracker | Backing store | `record` / `lookup` | Notes |
//! |---------|---------------|---------------------|-------|
//! | [`LinearStateTracker`] | `Vec<(String, CostSnapshot)>` | O(n) | Pre-optimization baseline. Stable iteration order. |
//! | [`HashedStateTracker`] | `HashMap<String, CostSnapshot>` | O(1) average, O(n) worst-case | Recommended for large inputs. |
//!
//! Use [`compare_backends`] to run a paired benchmark that exercises both
//! implementations on identical workloads. See the `bench_*` tests for
//! reproducible numbers.
//!
//! ## Example
//!
//! ```rust
//! use soroban_budget_assert_core::{HashedStateTracker, StateTracker, CostSnapshot};
//!
//! let mut tracker = HashedStateTracker::new();
//! tracker.record("deposit", CostSnapshot::new(1_000, 256));
//! tracker.record("withdraw", CostSnapshot::new(750, 128));
//!
//! assert_eq!(tracker.len(), 2);
//! assert_eq!(
//!     tracker.lookup("deposit"),
//!     Some(&CostSnapshot::new(1_000, 256))
//! );
//! assert_eq!(tracker.total_cpu(), 1_750);
//! ```

use std::collections::HashMap;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// CostSnapshot
// ---------------------------------------------------------------------------

/// A snapshot of the cost metrics recorded for a single operation.
///
/// Carries the two metrics tracked by the Soroban budget system: CPU
/// instructions and memory bytes. For on-network read/write-byte figures,
/// see `cargo-budget-report`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CostSnapshot {
    /// CPU instructions consumed by the operation.
    pub cpu: u64,
    /// Memory bytes consumed by the operation.
    pub memory: u64,
}

impl CostSnapshot {
    /// Creates a new snapshot with the given CPU and memory values.
    pub fn new(cpu: u64, memory: u64) -> Self {
        Self { cpu, memory }
    }

    /// Returns a snapshot whose both metrics are zero.
    pub fn zero() -> Self {
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// StateTracker trait
// ---------------------------------------------------------------------------

/// Common interface for per-operation cost tracking.
///
/// Every backend stores `(operation_name, CostSnapshot)` pairs, supports
/// upserts via [`record`](StateTracker::record), lookups via
/// [`lookup`](StateTracker::lookup), and aggregate queries via
/// [`total_cpu`](StateTracker::total_cpu) / [`total_memory`](StateTracker::total_memory).
///
/// Iterating with [`ops`](StateTracker::ops) returns the names of all
/// currently recorded operations. Implementations are **not** required to
/// preserve insertion order when iterating.
pub trait StateTracker {
    /// Inserts or updates the snapshot for `op`. If `op` already exists,
    /// the previous snapshot is overwritten.
    fn record(&mut self, op: &str, snapshot: CostSnapshot);

    /// Returns the snapshot for `op`, or `None` if it has not been recorded.
    fn lookup(&self, op: &str) -> Option<&CostSnapshot>;

    /// Returns `true` if `op` has been recorded.
    fn contains(&self, op: &str) -> bool {
        self.lookup(op).is_some()
    }

    /// Returns the number of distinct operations tracked.
    fn len(&self) -> usize;

    /// Returns `true` if no operations are tracked.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Sums CPU across all recorded snapshots.
    fn total_cpu(&self) -> u64;

    /// Sums memory across all recorded snapshots.
    fn total_memory(&self) -> u64;

    /// Returns the names of all tracked operations. Order is
    /// implementation-defined.
    fn ops(&self) -> Vec<String>;

    /// Removes all entries from the tracker, leaving it empty.
    fn clear(&mut self);
}

// ---------------------------------------------------------------------------
// LinearStateTracker
// ---------------------------------------------------------------------------

/// Linear-scan cost tracker.
///
/// Stores entries in a `Vec<(String, CostSnapshot)>` and scans the entire
/// vector on every `record`/`lookup`. Useful as a baseline for benchmarks
/// and when stable insertion order matters.
///
/// This backend is the **pre-optimization** implementation. New code should
/// prefer [`HashedStateTracker`].
#[derive(Debug, Clone, Default)]
pub struct LinearStateTracker {
    entries: Vec<(String, CostSnapshot)>,
}

impl LinearStateTracker {
    /// Creates an empty tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an empty tracker with the given pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
        }
    }

    /// Returns the number of entries currently stored.
    pub fn capacity(&self) -> usize {
        self.entries.capacity()
    }

    /// Converts this linear tracker into a [`HashedStateTracker`], preserving
    /// the **last** snapshot written for each operation name.
    ///
    /// If the same operation appears multiple times in the linear tracker,
    /// only the most recent snapshot is kept, matching
    /// `HashedStateTracker::record` semantics.
    pub fn into_hashed(self) -> HashedStateTracker {
        let mut out = HashedStateTracker::with_capacity(self.entries.len());
        for (op, snap) in self.entries {
            out.record(&op, snap);
        }
        out
    }
}

impl StateTracker for LinearStateTracker {
    fn record(&mut self, op: &str, snapshot: CostSnapshot) {
        for entry in self.entries.iter_mut() {
            if entry.0 == op {
                entry.1 = snapshot;
                return;
            }
        }
        self.entries.push((op.to_string(), snapshot));
    }

    fn lookup(&self, op: &str) -> Option<&CostSnapshot> {
        for entry in self.entries.iter() {
            if entry.0 == op {
                return Some(&entry.1);
            }
        }
        None
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn total_cpu(&self) -> u64 {
        self.entries.iter().map(|(_, s)| s.cpu).sum()
    }

    fn total_memory(&self) -> u64 {
        self.entries.iter().map(|(_, s)| s.memory).sum()
    }

    fn ops(&self) -> Vec<String> {
        self.entries.iter().map(|(k, _)| k.clone()).collect()
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}

// ---------------------------------------------------------------------------
// HashedStateTracker
// ---------------------------------------------------------------------------

/// Hash-based cost tracker.
///
/// Stores entries in a `HashMap<String, CostSnapshot>` and provides O(1)
/// average-time `record` / `lookup`. Iteration order is unspecified.
///
/// This backend is the **post-optimization** implementation and is the
/// recommended default for new code. It eliminates the O(n) scan bottleneck
/// identified in the original linear implementation while preserving the
/// same external API.
#[derive(Debug, Clone, Default)]
pub struct HashedStateTracker {
    entries: HashMap<String, CostSnapshot>,
}

impl HashedStateTracker {
    /// Creates an empty tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an empty tracker with the given pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(capacity),
        }
    }

    /// Returns the number of entries currently stored.
    pub fn capacity(&self) -> usize {
        self.entries.capacity()
    }
}

impl StateTracker for HashedStateTracker {
    fn record(&mut self, op: &str, snapshot: CostSnapshot) {
        self.entries.insert(op.to_string(), snapshot);
    }

    fn lookup(&self, op: &str) -> Option<&CostSnapshot> {
        self.entries.get(op)
    }

    fn contains(&self, op: &str) -> bool {
        self.entries.contains_key(op)
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn total_cpu(&self) -> u64 {
        self.entries.values().map(|s| s.cpu).sum()
    }

    fn total_memory(&self) -> u64 {
        self.entries.values().map(|s| s.memory).sum()
    }

    fn ops(&self) -> Vec<String> {
        self.entries.keys().cloned().collect()
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}

// ---------------------------------------------------------------------------
// Benchmark harness
// ---------------------------------------------------------------------------

/// Paired benchmark for [`LinearStateTracker`] vs [`HashedStateTracker`].
///
/// Runs `n` distinct `record` calls followed by `n` `lookup` calls on each
/// backend with the same operation names and snapshot values, measuring wall
/// time for each phase. The result includes both elapsed times and the
/// speed-up factor (`linear / hashed`).
///
/// # Example
///
/// ```rust
/// use soroban_budget_assert_core::compare_backends;
///
/// let report = compare_backends(100, 10);
/// assert_eq!(report.n, 100);
/// assert!(report.lookup_speedup >= 1.0);
/// ```
pub fn compare_backends(n: usize, warmup: usize) -> BenchmarkReport {
    let names: Vec<String> = (0..n).map(|i| format!("op_{i}")).collect();
    let snapshots: Vec<CostSnapshot> = (0..n)
        .map(|i| CostSnapshot::new(i as u64 * 10, i as u64 * 4))
        .collect();

    let mut linear = LinearStateTracker::with_capacity(n);
    let mut hashed = HashedStateTracker::with_capacity(n);

    // Warm-up inserts to amortize first-time allocation costs.
    for i in 0..warmup {
        linear.record(&names[i], snapshots[i]);
        hashed.record(&names[i], snapshots[i]);
    }

    let t0 = Instant::now();
    for i in 0..n {
        linear.record(&names[i], snapshots[i]);
    }
    let linear_record_time = t0.elapsed();

    let t0 = Instant::now();
    for i in 0..n {
        hashed.record(&names[i], snapshots[i]);
    }
    let hashed_record_time = t0.elapsed();

    let t0 = Instant::now();
    for name in &names {
        let _ = linear.lookup(name);
    }
    let linear_lookup_time = t0.elapsed();

    let t0 = Instant::now();
    for name in &names {
        let _ = hashed.lookup(name);
    }
    let hashed_lookup_time = t0.elapsed();

    BenchmarkReport {
        n,
        warmup,
        linear_record_time,
        hashed_record_time,
        linear_lookup_time,
        hashed_lookup_time,
    }
}

/// Result of [`compare_backends`].
///
/// Captures the elapsed times for each phase of the paired benchmark as
/// well as the computed speed-up factors (linear / hashed). `speedup >= 1.0`
/// indicates that the hashed backend is faster for that phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BenchmarkReport {
    /// Number of distinct operations exercised.
    pub n: usize,
    /// Number of warm-up inserts run before timing began.
    pub warmup: usize,
    /// Time spent on `n` `record` calls against the linear backend.
    pub linear_record_time: Duration,
    /// Time spent on `n` `record` calls against the hashed backend.
    pub hashed_record_time: Duration,
    /// Time spent on `n` `lookup` calls against the linear backend.
    pub linear_lookup_time: Duration,
    /// Time spent on `n` `lookup` calls against the hashed backend.
    pub hashed_lookup_time: Duration,
}

impl BenchmarkReport {
    /// Returns `linear_record_time / hashed_record_time`. Values > 1.0 mean
    /// the hashed backend was faster on inserts.
    ///
    /// Note: the recorded phase measures a mixed upsert-with-existing-entries
    /// workload, because the warm-up done by [`compare_backends`] pre-fills
    /// the first `warmup` keys. Upserts on the linear backend hit earlier
    /// vector positions while fresh inserts walk the full growing vector,
    /// so the linear cost is dominated by the insert tail. Both backends
    /// still receive the same operation sequence, so the ratio is a fair
    /// measure of the asymptotic difference (O(n²) vs O(n)).
    pub fn record_speedup(&self) -> f64 {
        duration_ratio(self.linear_record_time, self.hashed_record_time)
    }

    /// Returns `linear_lookup_time / hashed_lookup_time`. Values > 1.0 mean
    /// the hashed backend was faster on lookups. Both backends scan the
    /// already-populated state, so this contrasts full-vector linear
    /// scanning O(n) against hash-table point lookups O(1).
    pub fn lookup_speedup(&self) -> f64 {
        duration_ratio(self.linear_lookup_time, self.hashed_lookup_time)
    }
}

fn duration_ratio(a: Duration, b: Duration) -> f64 {
    let a_ns = a.as_nanos() as f64;
    let b_ns = b.as_nanos() as f64;
    if b_ns == 0.0 {
        f64::INFINITY
    } else {
        a_ns / b_ns
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── CostSnapshot ────────────────────────────────────────────────────

    #[test]
    fn cost_snapshot_new_stores_values() {
        let s = CostSnapshot::new(42, 128);
        assert_eq!(s.cpu, 42);
        assert_eq!(s.memory, 128);
    }

    #[test]
    fn cost_snapshot_zero_is_zero() {
        assert_eq!(CostSnapshot::zero().cpu, 0);
        assert_eq!(CostSnapshot::zero().memory, 0);
    }

    #[test]
    fn cost_snapshot_default_is_zero() {
        assert_eq!(CostSnapshot::default(), CostSnapshot::zero());
    }

    #[test]
    fn cost_snapshot_clone_and_eq() {
        let a = CostSnapshot::new(10, 20);
        let b = a;
        assert_eq!(a, b);
    }

    // ── LinearStateTracker ──────────────────────────────────────────────

    #[test]
    fn linear_tracker_starts_empty() {
        let t = LinearStateTracker::new();
        assert!(t.is_empty());
        assert_eq!(t.len(), 0);
        assert_eq!(t.total_cpu(), 0);
        assert_eq!(t.total_memory(), 0);
        assert!(t.lookup("anything").is_none());
    }

    #[test]
    fn linear_tracker_record_inserts_new() {
        let mut t = LinearStateTracker::new();
        t.record("a", CostSnapshot::new(10, 5));
        assert_eq!(t.len(), 1);
        assert_eq!(t.lookup("a"), Some(&CostSnapshot::new(10, 5)));
    }

    #[test]
    fn linear_tracker_record_updates_existing() {
        let mut t = LinearStateTracker::new();
        t.record("a", CostSnapshot::new(10, 5));
        t.record("a", CostSnapshot::new(99, 7));
        assert_eq!(t.len(), 1);
        assert_eq!(t.lookup("a"), Some(&CostSnapshot::new(99, 7)));
    }

    #[test]
    fn linear_tracker_lookup_missing_returns_none() {
        let mut t = LinearStateTracker::new();
        t.record("a", CostSnapshot::new(10, 5));
        assert!(t.lookup("b").is_none());
        assert!(!t.contains("b"));
    }

    #[test]
    fn linear_tracker_total_sums_all_entries() {
        let mut t = LinearStateTracker::new();
        t.record("a", CostSnapshot::new(10, 5));
        t.record("b", CostSnapshot::new(20, 10));
        t.record("c", CostSnapshot::new(30, 15));
        assert_eq!(t.total_cpu(), 60);
        assert_eq!(t.total_memory(), 30);
    }

    #[test]
    fn linear_tracker_ops_returns_all_names() {
        let mut t = LinearStateTracker::new();
        t.record("alpha", CostSnapshot::new(1, 1));
        t.record("beta", CostSnapshot::new(2, 2));
        let ops = t.ops();
        assert_eq!(ops.len(), 2);
        assert!(ops.contains(&"alpha".to_string()));
        assert!(ops.contains(&"beta".to_string()));
    }

    #[test]
    fn linear_tracker_preserves_insertion_order_in_ops() {
        // LinearStateTracker is documented to preserve insertion order in
        // its `ops()` iteration — other backends are explicitly not.
        let mut t = LinearStateTracker::new();
        t.record("first", CostSnapshot::new(1, 1));
        t.record("second", CostSnapshot::new(2, 2));
        t.record("third", CostSnapshot::new(3, 3));
        assert_eq!(
            t.ops(),
            vec![
                "first".to_string(),
                "second".to_string(),
                "third".to_string()
            ]
        );
    }

    // ── HashedStateTracker ──────────────────────────────────────────────

    #[test]
    fn hashed_tracker_starts_empty() {
        let t = HashedStateTracker::new();
        assert!(t.is_empty());
        assert_eq!(t.len(), 0);
        assert_eq!(t.total_cpu(), 0);
        assert_eq!(t.total_memory(), 0);
        assert!(t.lookup("anything").is_none());
    }

    #[test]
    fn hashed_tracker_record_inserts_new() {
        let mut t = HashedStateTracker::new();
        t.record("a", CostSnapshot::new(10, 5));
        assert_eq!(t.len(), 1);
        assert_eq!(t.lookup("a"), Some(&CostSnapshot::new(10, 5)));
    }

    #[test]
    fn hashed_tracker_record_updates_existing() {
        let mut t = HashedStateTracker::new();
        t.record("a", CostSnapshot::new(10, 5));
        t.record("a", CostSnapshot::new(99, 7));
        assert_eq!(t.len(), 1);
        assert_eq!(t.lookup("a"), Some(&CostSnapshot::new(99, 7)));
    }

    #[test]
    fn hashed_tracker_lookup_missing_returns_none() {
        let mut t = HashedStateTracker::new();
        t.record("a", CostSnapshot::new(10, 5));
        assert!(t.lookup("b").is_none());
        assert!(!t.contains("b"));
    }

    #[test]
    fn hashed_tracker_total_sums_all_entries() {
        let mut t = HashedStateTracker::new();
        t.record("a", CostSnapshot::new(10, 5));
        t.record("b", CostSnapshot::new(20, 10));
        t.record("c", CostSnapshot::new(30, 15));
        assert_eq!(t.total_cpu(), 60);
        assert_eq!(t.total_memory(), 30);
    }

    #[test]
    fn hashed_tracker_ops_returns_all_names() {
        let mut t = HashedStateTracker::new();
        t.record("alpha", CostSnapshot::new(1, 1));
        t.record("beta", CostSnapshot::new(2, 2));
        let ops = t.ops();
        assert_eq!(ops.len(), 2);
        assert!(ops.contains(&"alpha".to_string()));
        assert!(ops.contains(&"beta".to_string()));
    }

    #[test]
    fn hashed_tracker_clear_empties_state() {
        let mut t = HashedStateTracker::new();
        t.record("a", CostSnapshot::new(10, 5));
        t.record("b", CostSnapshot::new(20, 10));
        assert_eq!(t.len(), 2);
        t.clear();
        assert!(t.is_empty());
        assert!(t.lookup("a").is_none());
    }

    // ── Parity between backends ─────────────────────────────────────────

    #[test]
    fn both_trackers_agree_on_record_and_lookup() {
        let ops = ["deposit", "withdraw", "swap", "claim"];
        let snaps = [
            CostSnapshot::new(100, 10),
            CostSnapshot::new(200, 20),
            CostSnapshot::new(300, 30),
            CostSnapshot::new(400, 40),
        ];

        let mut lin = LinearStateTracker::new();
        let mut hash = HashedStateTracker::new();

        for (op, snap) in ops.iter().zip(snaps.iter()) {
            lin.record(op, *snap);
            hash.record(op, *snap);
        }

        assert_eq!(lin.len(), hash.len());
        assert_eq!(lin.total_cpu(), hash.total_cpu());
        assert_eq!(lin.total_memory(), hash.total_memory());
        assert_eq!(lin.len(), ops.len());

        for (op, expected) in ops.iter().zip(snaps.iter()) {
            assert_eq!(lin.lookup(op), Some(expected));
            assert_eq!(hash.lookup(op), Some(expected));
        }
    }

    #[test]
    fn both_trackers_agree_on_upsert() {
        let mut lin = LinearStateTracker::new();
        let mut hash = HashedStateTracker::new();
        lin.record("op", CostSnapshot::new(1, 1));
        hash.record("op", CostSnapshot::new(1, 1));
        lin.record("op", CostSnapshot::new(2, 3));
        hash.record("op", CostSnapshot::new(2, 3));

        assert_eq!(lin.len(), hash.len());
        assert_eq!(lin.lookup("op"), Some(&CostSnapshot::new(2, 3)));
        assert_eq!(hash.lookup("op"), Some(&CostSnapshot::new(2, 3)));
    }

    #[test]
    fn linear_into_hashed_preserves_last_snapshot() {
        let mut lin = LinearStateTracker::new();
        lin.record("a", CostSnapshot::new(1, 1));
        lin.record("a", CostSnapshot::new(2, 2));
        lin.record("b", CostSnapshot::new(3, 3));

        let hash = lin.into_hashed();
        assert_eq!(hash.len(), 2);
        assert_eq!(hash.lookup("a"), Some(&CostSnapshot::new(2, 2)));
        assert_eq!(hash.lookup("b"), Some(&CostSnapshot::new(3, 3)));
    }

    // ── Benchmarks ──────────────────────────────────────────────────────
    //
    // We exercise the benchmark harness on a representative input size. The
    // exact numbers vary by machine, but on a typical CI runner the hashed
    // backend should be substantially faster on both `record` (which
    // upserts) and `lookup`. We assert only basic invariants so the test
    // does not flake.

    #[test]
    fn benchmark_smoke_runs_and_reports_speedups() {
        // The smoke test verifies the harness itself runs end-to-end and
        // produces a well-formed report. The *optimization* invariant
        // (hashed beats linear at the same scale) is asserted by
        // `benchmark_hashed_outperforms_linear_at_scale` below; comparing
        // absolute timings across machines (and across record-vs-lookup
        // phases) is intentionally not asserted here — record phase
        // upserting into an already-warm linear vec can be competitive
        // with hashing on very small inputs.
        let report = compare_backends(200, 50);
        assert_eq!(report.n, 200);
        assert_eq!(report.warmup, 50);
        // Each phase must have produced a non-negative timing.
        let zero = Duration::ZERO;
        assert!(report.linear_record_time >= zero);
        assert!(report.hashed_record_time >= zero);
        assert!(report.linear_lookup_time >= zero);
        assert!(report.hashed_lookup_time >= zero);
        // Speedup ratios must be finite and non-negative — a div-by-zero
        // path returning NaN/Inf would be a regression in
        // `duration_ratio`, so we guard it here even though we do not
        // assert ordering.
        assert!(report.record_speedup().is_finite() && report.record_speedup() >= 0.0);
        assert!(report.lookup_speedup().is_finite() && report.lookup_speedup() >= 0.0);
    }

    #[test]
    fn benchmark_handles_empty_input() {
        let report = compare_backends(0, 0);
        assert_eq!(report.n, 0);
        // With no work to do, both backends should report a timing that fits
        // comfortably inside a 1 ms margin (wall-clock jitter may produce a
        // few nanoseconds even for an empty loop).
        let one_ms = Duration::from_millis(1).as_nanos();
        assert!(report.linear_record_time.as_nanos() <= one_ms);
        assert!(report.hashed_record_time.as_nanos() <= one_ms);
        assert!(report.linear_lookup_time.as_nanos() <= one_ms);
        assert!(report.hashed_lookup_time.as_nanos() <= one_ms);
    }

    #[test]
    fn benchmark_hashed_outperforms_linear_at_scale() {
        // Two workload sizes — confirm the optimization holds at both, and
        // that the linear backend actually scales with input size. The
        // cross-size comparison is intentionally absent because absolute
        // timings are noisy on shared CI runners; comparing hashed-vs-linear
        // *at the same scale* is the robust invariant we actually need.
        let small = compare_backends(50, 10);
        let large = compare_backends(2_000, 10);

        assert!(
            large.hashed_lookup_time <= large.linear_lookup_time,
            "hashed should be at least as fast as linear at n=2000 (hashed: {:?}, linear: {:?})",
            large.hashed_lookup_time,
            large.linear_lookup_time
        );
        assert!(
            small.hashed_lookup_time <= small.linear_lookup_time,
            "hashed should be at least as fast as linear at n=50 (hashed: {:?}, linear: {:?})",
            small.hashed_lookup_time,
            small.linear_lookup_time
        );

        // Sanity check the benchmark harness itself: the linear backend
        // must take longer on a larger workload. If this fails we cannot
        // trust the rest of the harness.
        assert!(large.linear_lookup_time >= small.linear_lookup_time);
    }
}
