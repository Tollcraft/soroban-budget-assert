//! Integration tests pinning the two behaviours of `BudgetAssert::within_budget`
//! and `BudgetExceededError` that downstream reporting tools treat as a
//! contract: the order the metrics are evaluated in, and the exact `Display`
//! rendering of a breach.
//!
//! `within_budget` short-circuits (`src/traits.rs`): CPU is checked first and
//! `?` returns before `memory_within` runs. A simultaneous breach therefore
//! always reports CPU, and a report that shows "Memory Bytes" for a run that
//! also blew its CPU limit is a regression rather than a tie-break.
//!
//! The ordering is proven by *counting calls* rather than only by reading the
//! error text. `Probe` records how many times each metric was consulted, so a
//! change that evaluated memory first, or evaluated both and then picked CPU,
//! fails here instead of continuing to pass on the strength of the message.

use std::cell::Cell;

use soroban_budget_assert_core::traits::{
    BudgetAssert, BudgetExceededError, CostMeasurer, METRIC_CPU_INSTRUCTIONS, METRIC_MEMORY_BYTES,
    METRIC_NAMES,
};

/// A `CostMeasurer` that reports fixed costs and counts how often each one was
/// read.
struct Probe {
    cpu: u64,
    memory: u64,
    cpu_reads: Cell<u32>,
    memory_reads: Cell<u32>,
}

impl Probe {
    fn new(cpu: u64, memory: u64) -> Self {
        Probe {
            cpu,
            memory,
            cpu_reads: Cell::new(0),
            memory_reads: Cell::new(0),
        }
    }

    fn cpu_breaches(memory_breaches: bool) -> Self {
        // 1_500 / 2_048 against the 1_000 / 1_024 limits used throughout this
        // file: both values sit above their limit when the respective flag is
        // set and well below it otherwise.
        Probe::new(1_500, if memory_breaches { 2_048 } else { 10 })
    }
}

impl CostMeasurer for Probe {
    fn cpu_instructions(&self) -> u64 {
        self.cpu_reads.set(self.cpu_reads.get() + 1);
        self.cpu
    }

    fn memory_bytes(&self) -> u64 {
        self.memory_reads.set(self.memory_reads.get() + 1);
        self.memory
    }
}

/// Runs `within_budget(1_000, 1_024)` and returns the error, asserting the call
/// was expected to fail.
fn breach(probe: &Probe) -> BudgetExceededError {
    probe
        .within_budget(1_000, 1_024)
        .expect_err("the configured costs should breach at least one limit")
}

// ---------------------------------------------------------------------------
// Evaluation order
// ---------------------------------------------------------------------------

#[test]
fn only_cpu_breaching_reports_the_cpu_error() {
    let probe = Probe::cpu_breaches(false);
    let error = breach(&probe);

    assert_eq!(error.metric, METRIC_CPU_INSTRUCTIONS);
    assert_eq!(error.actual, 1_500);
    assert_eq!(error.limit, 1_000);
    assert_eq!(
        error.to_string(),
        "CPU Instructions exceeded limit: value=1500, limit=1000"
    );
}

#[test]
fn only_memory_breaching_reports_the_memory_error() {
    let probe = Probe::new(10, 2_048);
    let error = breach(&probe);

    assert_eq!(error.metric, METRIC_MEMORY_BYTES);
    assert_eq!(error.actual, 2_048);
    assert_eq!(error.limit, 1_024);
    assert_eq!(
        error.to_string(),
        "Memory Bytes exceeded limit: value=2048, limit=1024"
    );
}

#[test]
fn simultaneous_breach_reports_cpu() {
    let probe = Probe::cpu_breaches(true);
    let error = breach(&probe);

    assert_eq!(
        error.metric, METRIC_CPU_INSTRUCTIONS,
        "a run that breaches both limits must be attributed to CPU, not memory"
    );
    assert_eq!(
        error.to_string(),
        "CPU Instructions exceeded limit: value=1500, limit=1000"
    );
    assert_ne!(error.metric, METRIC_MEMORY_BYTES);
}

#[test]
fn cpu_breach_short_circuits_before_memory_is_measured() {
    let probe = Probe::cpu_breaches(true);
    let _error = breach(&probe);

    assert_eq!(
        probe.cpu_reads.get(),
        1,
        "CPU should be measured exactly once"
    );
    assert_eq!(
        probe.memory_reads.get(),
        0,
        "memory must not be consulted at all once CPU has failed; a non-zero \
         count means both metrics are measured and CPU is only a display preference"
    );
}

/// Guards the previous test against being vacuously true: when CPU is within
/// budget, memory *is* measured, so the zero above is about ordering rather
/// than about `within_budget` never reading memory at all.
#[test]
fn memory_is_measured_when_cpu_is_within_budget() {
    let probe = Probe::new(10, 2_048);
    let _error = breach(&probe);

    assert_eq!(probe.cpu_reads.get(), 1);
    assert_eq!(probe.memory_reads.get(), 1);
}

#[test]
fn both_within_budget_succeeds_and_measures_each_metric_once() {
    let probe = Probe::new(10, 20);

    probe
        .within_budget(1_000, 1_024)
        .expect("both costs sit below their limits");

    assert_eq!(probe.cpu_reads.get(), 1);
    assert_eq!(probe.memory_reads.get(), 1);
}

// ---------------------------------------------------------------------------
// Strictness at the boundary
// ---------------------------------------------------------------------------

/// `cpu_within` / `memory_within` compare with `<`, so a cost equal to the
/// limit is a breach. Pinned here because `>=` and `>` read the same in review
/// and differ in every reported figure at the edge.
#[test]
fn a_cost_equal_to_its_limit_is_a_breach() {
    let cpu_edge = Probe::new(1_000, 10);
    assert_eq!(
        cpu_edge
            .cpu_within(1_000)
            .expect_err("== limit breaches")
            .to_string(),
        "CPU Instructions exceeded limit: value=1000, limit=1000"
    );

    let memory_edge = Probe::new(10, 1_024);
    assert_eq!(
        memory_edge
            .memory_within(1_024)
            .expect_err("== limit breaches")
            .to_string(),
        "Memory Bytes exceeded limit: value=1024, limit=1024"
    );

    // One below the limit on both metrics is the only clean pass at the edge.
    Probe::new(999, 1_023)
        .within_budget(1_000, 1_024)
        .expect("strictly-below costs pass");
}

// ---------------------------------------------------------------------------
// Display formatting
// ---------------------------------------------------------------------------

/// `Display` is the serialization boundary for reports: `"{} exceeded limit:
/// value={}, limit={}"`. These are byte-exact, including the absence of the
/// thousands separators that `format_metric` applies elsewhere.
#[test]
fn display_format_is_byte_exact() {
    let cases: &[(BudgetExceededError, &str)] = &[
        (
            BudgetExceededError::new(METRIC_CPU_INSTRUCTIONS, 1_500, 1_000),
            "CPU Instructions exceeded limit: value=1500, limit=1000",
        ),
        (
            BudgetExceededError::new(METRIC_MEMORY_BYTES, 2_048, 1_024),
            "Memory Bytes exceeded limit: value=2048, limit=1024",
        ),
        (
            BudgetExceededError::new(METRIC_CPU_INSTRUCTIONS, 0, 0),
            "CPU Instructions exceeded limit: value=0, limit=0",
        ),
        (
            BudgetExceededError::new(METRIC_CPU_INSTRUCTIONS, 1_234_567, 1),
            "CPU Instructions exceeded limit: value=1234567, limit=1",
        ),
        (
            BudgetExceededError::new(METRIC_MEMORY_BYTES, u64::MAX, 0),
            "Memory Bytes exceeded limit: value=18446744073709551615, limit=0",
        ),
    ];

    for (error, expected) in cases {
        assert_eq!(&error.to_string(), expected);
        assert_eq!(&format!("{error}"), expected);
    }
}

/// The rendering is a function of the three fields alone, so every metric name
/// the crate publishes formats through the same template.
#[test]
fn display_uses_the_same_template_for_every_metric_name() {
    for &metric in METRIC_NAMES {
        let error = BudgetExceededError::new(metric, 42, 7);
        assert_eq!(
            error.to_string(),
            format!("{} exceeded limit: value=42, limit=7", metric)
        );
        assert_eq!(error.metric, metric);
        assert_eq!(error.actual, 42);
        assert_eq!(error.limit, 7);
    }
}

#[test]
fn error_round_trips_through_the_std_error_trait() {
    let error = BudgetExceededError::new(METRIC_CPU_INSTRUCTIONS, 1_500, 1_000);
    let as_trait: &dyn std::error::Error = &error;

    assert_eq!(as_trait.to_string(), error.to_string());
    assert!(as_trait.source().is_none());
    assert_eq!(
        format!("{error:?}"),
        "BudgetExceededError { metric: \"CPU Instructions\", actual: 1500, limit: 1000 }"
    );
}
