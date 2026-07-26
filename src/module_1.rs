//! # Core Module: Cost Measurement & Budget Assertion Traits
//!
//! This module defines the foundational traits and types used throughout the
//! Soroban budget-assert ecosystem. Implementors of these traits provide the
//! concrete logic for measuring smart-contract resource consumption, asserting
//! budget limits during testing, and generating structured cost reports.
//!
//! ## Architecture
//!
//! The module is organized around three primary responsibilities:
//!
//! * **Measurement** — Capturing raw resource usage (CPU instructions, memory
//!   bytes, ledger read/write bytes) from a contract execution environment.
//! * **Assertion** — Comparing measured costs against configured limits and
//!   producing pass/fail outcomes for regression detection.
//! * **Reporting** — Aggregating per-function metrics into a structured format
//!   suitable for table, JSON, or CSV output.
//!
//! ## Trait Overview
//!
//! | Trait | Purpose |
//! |-------|---------|
//! | [`CostMeasurer`] | Defines the interface for extracting cost metrics from an environment. |
//! | [`BudgetAssert`] | Provides budget-limit checking logic built on top of a measurer. |
//! | [`ResourceReport`] | Serializes measured metrics into a reportable structure. |
//!
//! ## Example
//!
//! A typical workflow chains all three traits together:
//!
//! ```rust
//! # use soroban_budget_assert_core::{CostMeasurer, BudgetAssert, ResourceReport, ResourceReportable};
//!
//! /// A minimal contract invocation that tracks its own CPU and memory usage.
//! struct MyContract {
//!     cpu_instructions: u64,
//!     memory_bytes: u64,
//! }
//!
//! impl MyContract {
//!     fn new() -> Self {
//!         MyContract {
//!             cpu_instructions: 0,
//!             memory_bytes: 0,
//!         }
//!     }
//!
//!     fn invoke(&mut self, n: u64) {
//!         // Simulate some work that consumes resources.
//!         let mut acc: u64 = 0;
//!         for i in 0..n {
//!             acc = acc.wrapping_add(i);
//!         }
//!         self.cpu_instructions = n * 10;
//!         self.memory_bytes = n * 4;
//!         let _ = acc;
//!     }
//! }
//!
//! impl CostMeasurer for MyContract {
//!     fn cpu_instructions(&self) -> u64 { self.cpu_instructions }
//!     fn memory_bytes(&self) -> u64 { self.memory_bytes }
//! }
//!
//! impl BudgetAssert for MyContract {}
//!
//! let mut contract = MyContract::new();
//! contract.invoke(100);
//!
//! assert!(contract.cpu_within(2_000).is_ok());
//! assert!(contract.memory_within(1_000).is_ok());
//!
//! let report = contract.to_report("my_contract", "do_work");
//! assert_eq!(report.package, "my_contract");
//! assert_eq!(report.function, "do_work");
//! assert_eq!(report.cpu_instructions, 1000);
//! assert_eq!(report.memory_bytes, 400);
//! ```

use std::fmt;

// ---------------------------------------------------------------------------
// CostMeasurer trait
// ---------------------------------------------------------------------------

/// Trait for types that can report their resource consumption.
///
/// Implementors provide access to the two primary cost metrics tracked by the
/// Soroban budget system: **CPU instructions** (computation) and **memory
/// bytes** (heap / stack allocation). A third metric, *ledger read/write
/// bytes*, is intentionally omitted from this trait because it is only
/// available via on-chain RPC simulation (`simulateTransaction`) and cannot be
/// derived from local execution alone. Use [`cargo-budget-report`] for
/// on-network metrics.
///
/// [`cargo-budget-report`]: https://crates.io/crates/cargo-budget-report
///
/// # Implementing
///
/// The simplest implementation delegates to the Soroban SDK's [`Budget`]
/// type:
///
/// ```rust
/// # use soroban_budget_assert_core::CostMeasurer;
/// use soroban_sdk::{Env, budget::Budget};
///
/// struct MyContract { env: Env }
///
/// impl CostMeasurer for MyContract {
///     fn cpu_instructions(&self) -> u64 {
///         self.env.cost_estimate().budget().cpu_instruction_cost()
///     }
///
///     fn memory_bytes(&self) -> u64 {
///         self.env.cost_estimate().budget().memory_bytes_cost()
///     }
/// }
/// ```
///
/// For unit tests that do not use the Soroban SDK, a mock implementation can
/// return hard-coded or accumulated values.
///
/// [`Budget`]: https://docs.rs/soroban-sdk/latest/soroban_sdk/budget/struct.Budget.html
pub trait CostMeasurer {
    /// Returns the number of CPU instructions consumed by the most recent
    /// operation.
    ///
    /// This value represents the *local estimate* of instruction execution
    /// and includes VM metering overhead when run against compiled WASM.
    /// It may differ from the on-network figure returned by
    /// `simulateTransaction`.
    fn cpu_instructions(&self) -> u64;

    /// Returns the number of memory bytes consumed by the most recent
    /// operation.
    ///
    /// This value represents the *local estimate* of heap / stack allocation
    /// and may differ from the on-network write-bytes figure.
    fn memory_bytes(&self) -> u64;

    /// Returns the measured value for the given named metric.
    ///
    /// The default implementation matches against the standard metric names
    /// used throughout the project:
    ///
    /// | Name | Method | Unit |
    /// |------|--------|------|
    /// | `"CPU Instructions"` | [`cpu_instructions`](Self::cpu_instructions) | instructions |
    /// | `"Read Bytes"` | [`memory_bytes`](Self::memory_bytes) | bytes |
    /// | `"Write Bytes"` | [`memory_bytes`](Self::memory_bytes) | bytes |
    /// | `"WASM Bytes"` | returns `0` | bytes |
    ///
    /// Unknown metric names return `None`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use soroban_budget_assert_core::CostMeasurer;
    ///
    /// struct MockMeasurer;
    ///
    /// impl CostMeasurer for MockMeasurer {
    ///     fn cpu_instructions(&self) -> u64 { 42 }
    ///     fn memory_bytes(&self) -> u64 { 128 }
    /// }
    ///
    /// assert_eq!(MockMeasurer.value_for("CPU Instructions"), Some(42));
    /// assert_eq!(MockMeasurer.value_for("Read Bytes"), Some(128));
    /// assert_eq!(MockMeasurer.value_for("Unknown"), None);
    /// ```
    fn value_for(&self, metric: &str) -> Option<u64> {
        match metric {
            "CPU Instructions" => Some(self.cpu_instructions()),
            "Read Bytes" | "Write Bytes" => Some(self.memory_bytes()),
            "WASM Bytes" => Some(0),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// BudgetAssert trait
// ---------------------------------------------------------------------------

/// Trait for types that can assert their resource usage stays within budget.
///
/// This trait builds on [`CostMeasurer`] to provide ergonomic assertion
/// methods that compare measured costs against configured limits. It is the
/// trait-level equivalent of the `#[budget_cpu_lt(N)]` and
/// `#[budget_mem_lt(N)]` proc macros defined in `budget-macros`.
///
/// # Default Implementation
///
/// All methods have default implementations that delegate to
/// [`CostMeasurer::cpu_instructions`] and [`CostMeasurer::memory_bytes`].
/// A blanket impl provides [`BudgetExceededError`] as the default error type
/// for all [`CostMeasurer`] implementors, so `impl BudgetAssert for MyType {}`
/// is sufficient for most use cases.
///
/// # Example
///
/// ```rust
/// # use soroban_budget_assert_core::{CostMeasurer, BudgetAssert};
///
/// struct MyContract { cpu: u64, mem: u64 }
///
/// impl CostMeasurer for MyContract {
///     fn cpu_instructions(&self) -> u64 { self.cpu }
///     fn memory_bytes(&self) -> u64 { self.mem }
/// }
///
/// impl BudgetAssert for MyContract {}
///
/// let contract = MyContract { cpu: 100, mem: 200 };
///
/// // These should pass.
/// assert!(contract.cpu_within(200).is_ok());
/// assert!(contract.memory_within(300).is_ok());
///
/// // These should fail.
/// assert!(contract.cpu_within(50).is_err());
/// assert!(contract.memory_within(100).is_err());
/// ```
pub trait BudgetAssert: CostMeasurer {
    /// The error type returned by assertion methods.
    ///
    /// The default [`BudgetExceededError`] type (available via the blanket
    /// impl) carries the metric name, the measured value, and the limit that
    /// was exceeded.
    type Error: fmt::Debug + From<BudgetExceededError>;

    /// Asserts that the measured CPU instructions are strictly less than
    /// `limit`.
    ///
    /// Returns `Ok(())` if the assertion passes, or `Err(Self::Error)` with
    /// details about the breach.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use soroban_budget_assert_core::{CostMeasurer, BudgetAssert};
    ///
    /// struct CheapOp;
    ///
    /// impl CostMeasurer for CheapOp {
    ///     fn cpu_instructions(&self) -> u64 { 5 }
    ///     fn memory_bytes(&self) -> u64 { 0 }
    /// }
    ///
    /// impl BudgetAssert for CheapOp {}
    ///
    /// assert!(CheapOp.cpu_within(10).is_ok());
    /// assert!(CheapOp.cpu_within(1).is_err());
    /// ```
    fn cpu_within(&self, limit: u64) -> Result<(), Self::Error> {
        let actual = self.cpu_instructions();
        if actual < limit {
            Ok(())
        } else {
            Err(BudgetExceededError::new("CPU Instructions", actual, limit).into())
        }
    }

    /// Asserts that the measured memory bytes are strictly less than
    /// `limit`.
    ///
    /// Returns `Ok(())` if the assertion passes, or `Err(Self::Error)` with
    /// details about the breach.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use soroban_budget_assert_core::{CostMeasurer, BudgetAssert};
    ///
    /// struct LightOp;
    ///
    /// impl CostMeasurer for LightOp {
    ///     fn cpu_instructions(&self) -> u64 { 0 }
    ///     fn memory_bytes(&self) -> u64 { 512 }
    /// }
    ///
    /// impl BudgetAssert for LightOp {}
    ///
    /// assert!(LightOp.memory_within(1024).is_ok());
    /// assert!(LightOp.memory_within(256).is_err());
    /// ```
    fn memory_within(&self, limit: u64) -> Result<(), Self::Error> {
        let actual = self.memory_bytes();
        if actual < limit {
            Ok(())
        } else {
            Err(BudgetExceededError::new("Memory Bytes", actual, limit).into())
        }
    }

    /// Asserts that BOTH CPU instructions AND memory bytes are within their
    /// respective limits.
    ///
    /// Short-circuits on the first failure: if CPU exceeds its limit, the
    /// memory check is skipped and the CPU error is returned.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use soroban_budget_assert_core::{CostMeasurer, BudgetAssert};
    ///
    /// struct BalancedOp;
    ///
    /// impl CostMeasurer for BalancedOp {
    ///     fn cpu_instructions(&self) -> u64 { 100 }
    ///     fn memory_bytes(&self) -> u64 { 200 }
    /// }
    ///
    /// impl BudgetAssert for BalancedOp {}
    ///
    /// assert!(BalancedOp.within_budget(500, 500).is_ok());
    /// assert!(BalancedOp.within_budget(50, 500).is_err());
    /// assert!(BalancedOp.within_budget(500, 50).is_err());
    /// ```
    fn within_budget(&self, cpu_limit: u64, mem_limit: u64) -> Result<(), Self::Error> {
        self.cpu_within(cpu_limit)?;
        self.memory_within(mem_limit)
    }
}

// Blanket impl: every CostMeasurer automatically gets BudgetAssert with
// BudgetExceededError as the default error type.
impl<T: CostMeasurer> BudgetAssert for T {
    type Error = BudgetExceededError;
}

// ---------------------------------------------------------------------------
// BudgetExceededError
// ---------------------------------------------------------------------------

/// Error returned when a budget limit is exceeded.
///
/// Carries the name of the metric that failed, the measured value, and the
/// limit that was exceeded. This type is the default [`BudgetAssert::Error`]
/// provided by the blanket `BudgetAssert` impl.
///
/// # Formatting
///
/// The [`Display`](fmt::Display) implementation produces a message consistent
/// with `cargo-budget-report`'s check output:
///
/// ```text
/// CPU Instructions exceeded limit: value=1500, limit=1000
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetExceededError {
    /// The name of the metric that exceeded its limit (e.g. `"CPU Instructions"`).
    pub metric: &'static str,
    /// The measured value that breached the limit.
    pub actual: u64,
    /// The configured upper bound that was violated.
    pub limit: u64,
}

impl BudgetExceededError {
    /// Creates a new budget-exceeded error.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use soroban_budget_assert_core::BudgetExceededError;
    ///
    /// let err = BudgetExceededError::new("CPU Instructions", 1500, 1000);
    /// assert_eq!(err.metric, "CPU Instructions");
    /// assert_eq!(err.actual, 1500);
    /// assert_eq!(err.limit, 1000);
    /// ```
    pub fn new(metric: &'static str, actual: u64, limit: u64) -> Self {
        BudgetExceededError {
            metric,
            actual,
            limit,
        }
    }
}

impl fmt::Display for BudgetExceededError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} exceeded limit: value={}, limit={}",
            self.metric, self.actual, self.limit
        )
    }
}

impl std::error::Error for BudgetExceededError {}

// ---------------------------------------------------------------------------
// ResourceReport and ResourceReportable trait
// ---------------------------------------------------------------------------

/// A structured, serializable report of a single function's resource usage.
///
/// Each `ResourceReport` captures the package name, function name, and
/// measured cost metrics. This type mirrors the `CostReport` struct in
/// `cargo-budget-report/src/main.rs` and can be serialized to JSON or CSV.
///
/// # Example
///
/// ```rust
/// # use soroban_budget_assert_core::ResourceReport;
///
/// let report = ResourceReport {
///     package: "my-contract".to_string(),
///     function: "do_work".to_string(),
///     cpu_instructions: 1_000_000,
///     memory_bytes: 2_048,
///     wasm_bytes: 102_400,
/// };
///
/// assert_eq!(report.cpu_instructions, 1_000_000);
/// assert_eq!(report.memory_bytes, 2_048);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceReport {
    /// The Cargo package name (e.g. `"amm-pool-contract"`).
    pub package: String,
    /// The exported function name (e.g. `"deposit"`).
    pub function: String,
    /// Measured CPU instructions.
    pub cpu_instructions: u64,
    /// Measured memory bytes (proxy for ledger read/write bytes locally).
    pub memory_bytes: u64,
    /// Size of the compiled WASM binary in bytes.
    pub wasm_bytes: u64,
}

/// Trait for types that can produce a [`ResourceReport`] describing their
/// resource consumption.
///
/// This trait is the reporting counterpart of [`CostMeasurer`]: instead of
/// returning individual metrics, it aggregates them into a single structured
/// value suitable for display or serialization.
///
/// # Example
///
/// ```rust
/// # use soroban_budget_assert_core::{CostMeasurer, ResourceReport, ResourceReportable};
///
/// struct MyOp { cpu: u64, mem: u64 }
///
/// impl CostMeasurer for MyOp {
///     fn cpu_instructions(&self) -> u64 { self.cpu }
///     fn memory_bytes(&self) -> u64 { self.mem }
/// }
///
/// impl ResourceReportable for MyOp {
///     fn to_report(&self, package: &str, function: &str) -> ResourceReport {
///         ResourceReport {
///             package: package.to_string(),
///             function: function.to_string(),
///             cpu_instructions: self.cpu_instructions(),
///             memory_bytes: self.memory_bytes(),
///             wasm_bytes: 0,
///         }
///     }
/// }
///
/// let op = MyOp { cpu: 500, mem: 256 };
/// let report = op.to_report("my-crate", "do_work");
/// assert_eq!(report.cpu_instructions, 500);
/// ```
pub trait ResourceReportable {
    /// Build a [`ResourceReport`] for the given package and function.
    fn to_report(&self, package: &str, function: &str) -> ResourceReport;
}

// ---------------------------------------------------------------------------
// Utility function
// ---------------------------------------------------------------------------

/// Formats a numeric value with thousands-separator commas and a metric unit
/// suffix, consistent with `cargo-budget-report`'s display output.
///
/// # Example
///
/// ```rust
/// # use soroban_budget_assert_core::format_metric;
///
/// assert_eq!(format_metric(1_000_000, "CPU Instructions"), "1,000,000 inst.");
/// assert_eq!(format_metric(2_048, "Read Bytes"), "2,048 B");
/// assert_eq!(format_metric(0, "Write Bytes"), "0 B");
/// ```
pub fn format_metric(value: u64, metric: &str) -> String {
    let s = value.to_string();
    let mut result = String::new();
    let mut count = 0;

    for c in s.chars().rev() {
        if count == 3 {
            result.push(',');
            count = 0;
        }
        result.push(c);
        count += 1;
    }

    let formatted: String = result.chars().rev().collect();

    if metric.contains("Bytes") {
        format!("{} B", formatted)
    } else {
        format!("{} inst.", formatted)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── CostMeasurer tests ──────────────────────────────────────────────

    #[test]
    fn cost_measurer_returns_cpu_and_memory() {
        struct Dummy;
        impl CostMeasurer for Dummy {
            fn cpu_instructions(&self) -> u64 {
                42
            }
            fn memory_bytes(&self) -> u64 {
                128
            }
        }
        assert_eq!(Dummy.cpu_instructions(), 42);
        assert_eq!(Dummy.memory_bytes(), 128);
    }

    #[test]
    fn cost_measurer_value_for_known_metrics() {
        struct Dummy;
        impl CostMeasurer for Dummy {
            fn cpu_instructions(&self) -> u64 {
                99
            }
            fn memory_bytes(&self) -> u64 {
                200
            }
        }
        assert_eq!(Dummy.value_for("CPU Instructions"), Some(99));
        assert_eq!(Dummy.value_for("Read Bytes"), Some(200));
        assert_eq!(Dummy.value_for("Write Bytes"), Some(200));
        assert_eq!(Dummy.value_for("WASM Bytes"), Some(0));
    }

    #[test]
    fn cost_measurer_value_for_unknown_metric() {
        struct Dummy;
        impl CostMeasurer for Dummy {
            fn cpu_instructions(&self) -> u64 {
                0
            }
            fn memory_bytes(&self) -> u64 {
                0
            }
        }
        assert_eq!(Dummy.value_for("Unknown Metric"), None);
        assert_eq!(Dummy.value_for(""), None);
    }

    // ── BudgetAssert tests ──────────────────────────────────────────────

    /// A test type that receives the blanket `BudgetAssert` impl.
    struct WithinBudget(u64, u64);

    impl CostMeasurer for WithinBudget {
        fn cpu_instructions(&self) -> u64 {
            self.0
        }
        fn memory_bytes(&self) -> u64 {
            self.1
        }
    }

    // The blanket impl provides BudgetAssert for all CostMeasurer impls,
    // so WithinBudget automatically gets BudgetAssert<Error = BudgetExceededError>.

    #[test]
    fn cpu_within_passes_when_below_limit() {
        assert!(WithinBudget(100, 0).cpu_within(200).is_ok());
    }

    #[test]
    fn cpu_within_fails_when_at_limit() {
        // Limit is _strictly less than_, so equality should fail.
        let result = WithinBudget(200, 0).cpu_within(200);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.metric, "CPU Instructions");
        assert_eq!(err.actual, 200);
        assert_eq!(err.limit, 200);
    }

    #[test]
    fn cpu_within_fails_when_above_limit() {
        assert!(WithinBudget(300, 0).cpu_within(200).is_err());
    }

    #[test]
    fn memory_within_passes_when_below_limit() {
        assert!(WithinBudget(0, 100).memory_within(200).is_ok());
    }

    #[test]
    fn memory_within_fails_when_at_limit() {
        assert!(WithinBudget(0, 200).memory_within(200).is_err());
    }

    #[test]
    fn memory_within_fails_when_above_limit() {
        assert!(WithinBudget(0, 300).memory_within(200).is_err());
    }

    #[test]
    fn within_budget_passes_when_both_within() {
        assert!(WithinBudget(100, 200).within_budget(500, 500).is_ok());
    }

    #[test]
    fn within_budget_fails_on_cpu_first() {
        let result = WithinBudget(600, 100).within_budget(500, 500);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.metric, "CPU Instructions");
    }

    #[test]
    fn within_budget_fails_on_memory() {
        let result = WithinBudget(100, 600).within_budget(500, 500);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.metric, "Memory Bytes");
    }

    // ── BudgetExceededError tests ───────────────────────────────────────

    #[test]
    fn budget_exceeded_error_creation() {
        let err = BudgetExceededError::new("CPU Instructions", 1500, 1000);
        assert_eq!(err.metric, "CPU Instructions");
        assert_eq!(err.actual, 1500);
        assert_eq!(err.limit, 1000);
    }

    #[test]
    fn budget_exceeded_error_display() {
        let err = BudgetExceededError::new("CPU Instructions", 1500, 1000);
        let msg = format!("{}", err);
        assert!(msg.contains("CPU Instructions"));
        assert!(msg.contains("1500"));
        assert!(msg.contains("1000"));
    }

    #[test]
    fn budget_exceeded_error_is_error() {
        use std::error::Error;
        let err = BudgetExceededError::new("Read Bytes", 500, 256);
        assert!(err.source().is_none());
    }

    // ── ResourceReport tests ────────────────────────────────────────────

    #[test]
    fn resource_report_construction() {
        let report = ResourceReport {
            package: "my-contract".to_string(),
            function: "do_work".to_string(),
            cpu_instructions: 1_000_000,
            memory_bytes: 2_048,
            wasm_bytes: 102_400,
        };
        assert_eq!(report.package, "my-contract");
        assert_eq!(report.function, "do_work");
        assert_eq!(report.cpu_instructions, 1_000_000);
        assert_eq!(report.memory_bytes, 2_048);
        assert_eq!(report.wasm_bytes, 102_400);
    }

    // ── format_metric tests ─────────────────────────────────────────────

    #[test]
    fn format_metric_zero_cpu() {
        assert_eq!(format_metric(0, "CPU Instructions"), "0 inst.");
    }

    #[test]
    fn format_metric_zero_bytes() {
        assert_eq!(format_metric(0, "Read Bytes"), "0 B");
    }

    #[test]
    fn format_metric_with_commas() {
        assert_eq!(
            format_metric(1_000_000, "CPU Instructions"),
            "1,000,000 inst."
        );
    }

    #[test]
    fn format_metric_bytes_unit() {
        assert_eq!(format_metric(4_096, "Write Bytes"), "4,096 B");
    }
}
