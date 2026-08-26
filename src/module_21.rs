//! # Core Module
//!
//! This module defines the core trait abstractions for budget cost measurement,
//! estimation, and assertion used throughout the `soroban-budget-assert`
//! ecosystem.
//!
//! ## Overview
//!
//! The traits in this module form the contract between:
//!
//! * **Budget macros** (`#[budget_cpu_lt]`, `#[budget_mem_lt]`) that assert
//!   resource limits at test time.
//! * **Measurement providers** that supply CPU instruction and memory byte costs
//!   from various sources (local WASM estimates, network-simulated figures from
//!   `cargo budget-report`).
//! * **Report formatters** that serialize cost data for display, CSV, or JSON
//!   consumption in CI pipelines.
//!
//! ## Traits
//!
//! | Trait | Purpose |
//! |---|---|---|
//! | [`CpuCost`] | Types that report a CPU instruction cost. |
//! | [`MemoryCost`] | Types that report a memory byte cost. |
//! | [`BudgetBounds`] | Types that provide CPU and memory costs together with optional upper limits. |
//! | [`CostReportable`] | Types that can be formatted into a single cost-report entry. |
//!
//! ## Usage
//!
//! Implement these traits on your own measurement or report types to integrate
//! with the assertion and reporting machinery:
//!
//! ```rust
//! use soroban_budget_assert_core::module_21::{CpuCost, MemoryCost, BudgetBounds};
//!
//! /// A pair of measured costs for a contract function.
//! struct FunctionMetrics {
//!     name: &'static str,
//!     cpu: u64,
//!     mem: u64,
//! }
//!
//! impl CpuCost for FunctionMetrics {
//!     fn cpu_instruction_cost(&self) -> u64 {
//!         self.cpu
//!     }
//! }
//!
//! impl MemoryCost for FunctionMetrics {
//!     fn memory_bytes_cost(&self) -> u64 {
//!         self.mem
//!     }
//! }
//!
//! impl BudgetBounds for FunctionMetrics {
//!     fn label(&self) -> &str {
//!         self.name
//!     }
//!
//!     fn cpu_limit(&self) -> Option<u64> {
//!         Some(1_000_000)
//!     }
//!
//!     fn memory_limit(&self) -> Option<u64> {
//!         Some(500_000)
//!     }
//! }
//!
//! let metrics = FunctionMetrics { name: "do_expensive_work", cpu: 750_000, mem: 300_000 };
//! assert!(metrics.is_within_bounds().unwrap());
//! ```
//!
//! ## Design notes
//!
//! * All traits follow the **single-responsibility principle** — each trait
//!   captures exactly one capability, and they compose via supertrait bounds
//!   (e.g., `BudgetBounds: CpuCost + MemoryCost`).
//! * `BudgetBounds` provides a **default method** (`is_within_bounds`) so that
//!   implementors only need to supply the limit accessors and the label.
//! * The traits are deliberately **no-`std` compatible** — none of them depend on
//!   `std` or `alloc`, so they can be used inside Soroban contract code compiled
//!   with `#![no_std]`.

/// A type that can report its CPU instruction cost.
///
/// This trait provides a uniform interface for querying the CPU instruction
/// consumption of a contract execution. Implementations can source the value
/// from:
///
/// * A Soroban `Env` cost estimate
///   (`env.cost_estimate().budget().cpu_instruction_cost()`).
/// * A deserialized [`SorobanTransactionData`] resource field returned by
///   `simulateTransaction`.
/// * A user-provided constant or configuration value (e.g., from
///   `budget.toml` or `budget.json`).
///
/// # Usage
///
/// Implement `CpuCost` on any struct that carries or can compute a CPU
/// instruction count:
///
/// ```rust
/// use soroban_budget_assert_core::module_21::CpuCost;
///
/// /// A fixed-cost wrapper used in unit tests.
/// struct FixedCpu(u64);
///
/// impl CpuCost for FixedCpu {
///     fn cpu_instruction_cost(&self) -> u64 {
///         self.0
///     }
/// }
///
/// let cost = FixedCpu(42);
/// assert_eq!(cost.cpu_instruction_cost(), 42);
/// ```
///
/// [`SorobanTransactionData`]: https://docs.rs/stellar-xdr/22.1.0/stellar_xdr/curr/struct.SorobanTransactionData.html
pub trait CpuCost {
    /// Returns the CPU instruction cost.
    ///
    /// The returned value represents the number of CPU instructions consumed
    /// during contract execution. This is a non-refundable resource cost that
    /// the Soroban host uses to meter execution. The value is typically
    /// obtained from:
    ///
    /// * Local test estimates: `env.cost_estimate().budget().cpu_instruction_cost()`
    /// * Network simulation: the `instructions` field of
    ///   [`SorobanResources`](https://docs.rs/stellar-xdr/22.1.0/stellar_xdr/curr/struct.SorobanResources.html).
    ///
    /// # Returns
    ///
    /// The number of CPU instructions as a `u64`.
    fn cpu_instruction_cost(&self) -> u64;
}

/// A type that can report its memory byte cost.
///
/// This trait provides a uniform interface for querying the memory byte
/// consumption of a contract execution. Implementations can source the value
/// from:
///
/// * A Soroban `Env` cost estimate
///   (`env.cost_estimate().budget().memory_bytes_cost()`).
/// * A deserialized [`SorobanTransactionData`] resource field returned by
///   `simulateTransaction`.
/// * A user-provided constant or configuration value.
///
/// # Usage
///
/// Implement `MemoryCost` on any struct that carries or can compute a memory
/// byte count:
///
/// ```rust
/// use soroban_budget_assert_core::module_21::MemoryCost;
///
/// /// A fixed-cost wrapper used in unit tests.
/// struct FixedMem(u64);
///
/// impl MemoryCost for FixedMem {
///     fn memory_bytes_cost(&self) -> u64 {
///         self.0
///     }
/// }
///
/// let cost = FixedMem(2048);
/// assert_eq!(cost.memory_bytes_cost(), 2048);
/// ```
///
/// [`SorobanTransactionData`]: https://docs.rs/stellar-xdr/22.1.0/stellar_xdr/curr/struct.SorobanTransactionData.html
pub trait MemoryCost {
    /// Returns the memory byte cost.
    ///
    /// The returned value represents the number of memory bytes consumed
    /// during contract execution. This is a non-refundable resource cost that
    /// the Soroban host uses to meter heap and stack allocation. The value is
    /// typically obtained from:
    ///
    /// * Local test estimates:
    ///   `env.cost_estimate().budget().memory_bytes_cost()`
    /// * Network simulation: the `read_bytes` or `write_bytes` fields of
    ///   [`SorobanResources`](https://docs.rs/stellar-xdr/22.1.0/stellar_xdr/curr/struct.SorobanResources.html).
    ///
    /// # Returns
    ///
    /// The number of memory bytes as a `u64`.
    fn memory_bytes_cost(&self) -> u64;
}

/// A type that pairs CPU and memory cost information with optional upper
/// bounds.
///
/// `BudgetBounds` extends [`CpuCost`] and [`MemoryCost`] with a human-readable
/// label and configurable upper limits, enabling the consumer to determine
/// whether a measured cost is within acceptable bounds.
///
/// # Provided methods
///
/// * [`is_within_bounds`](BudgetBounds::is_within_bounds) — checks both costs
///   against their respective limits and returns a `Result<bool, String>`.
///
/// # Usage
///
/// ```rust
/// use soroban_budget_assert_core::module_21::{CpuCost, MemoryCost, BudgetBounds};
///
/// /// Metrics for a swap operation.
/// struct SwapMetrics {
///     cpu: u64,
///     mem: u64,
/// }
///
/// impl CpuCost for SwapMetrics {
///     fn cpu_instruction_cost(&self) -> u64 { self.cpu }
/// }
///
/// impl MemoryCost for SwapMetrics {
///     fn memory_bytes_cost(&self) -> u64 { self.mem }
/// }
///
/// impl BudgetBounds for SwapMetrics {
///     fn label(&self) -> &str { "swap" }
///     fn cpu_limit(&self) -> Option<u64> { Some(2_000_000) }
///     fn memory_limit(&self) -> Option<u64> { None }
/// }
///
/// let metrics = SwapMetrics { cpu: 1_500_000, mem: 100_000 };
/// // CPU is below limit (1_500_000 < 2_000_000) and memory has no limit.
/// assert!(metrics.is_within_bounds().unwrap());
///
/// let exceeded = SwapMetrics { cpu: 3_000_000, mem: 100_000 };
/// assert!(!exceeded.is_within_bounds().unwrap());
/// ```
pub trait BudgetBounds: CpuCost + MemoryCost {
    /// A human-readable label identifying this budget measurement.
    ///
    /// Typically this is the name of the contract function being measured
    /// (e.g., `"do_expensive_work"` or `"amm-pool-contract::swap"`). The
    /// label appears in report output and error messages to help users
    /// identify which function caused a budget breach.
    fn label(&self) -> &str;

    /// Optional upper bound for CPU instruction cost.
    ///
    /// Returns `None` when no CPU limit has been configured. When `None`,
    /// the [`is_within_bounds`] method reports the CPU metric as within
    /// bounds regardless of its value — the metric is reported but not
    /// enforced.
    ///
    /// [`is_within_bounds`]: BudgetBounds::is_within_bounds
    fn cpu_limit(&self) -> Option<u64>;

    /// Optional upper bound for memory byte cost.
    ///
    /// Returns `None` when no memory limit has been configured. When `None`,
    /// the [`is_within_bounds`] method reports the memory metric as within
    /// bounds regardless of its value — the metric is reported but not
    /// enforced.
    ///
    /// [`is_within_bounds`]: BudgetBounds::is_within_bounds
    fn memory_limit(&self) -> Option<u64>;

    /// Checks whether both measured costs are within their configured bounds.
    ///
    /// For each metric (CPU instructions, memory bytes):
    ///
    /// * If the corresponding limit is `Some(n)` and the measured cost is
    ///   **greater than or equal to** `n`, the check **fails** for that
    ///   metric.
    /// * If the limit is `None`, the metric is always considered within
    ///   bounds (reported but not enforced).
    ///
    /// All metrics must pass for the overall result to be `Ok(true)`.
    ///
    /// # Returns
    ///
    /// * `Ok(true)` — both costs are within their configured limits (or have
    ///   no configured limit).
    /// * `Ok(false)` — at least one cost exceeds its configured limit.
    ///
    /// # Errors
    ///
    /// Returns `Err(message)` when an implementation encounters an internal
    /// inconsistency that prevents the check from completing (e.g., a limit
    /// value that cannot be meaningfully compared). Concrete implementations
    /// that always return `None` from both limit accessors will never
    /// produce an error.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use soroban_budget_assert_core::module_21::{CpuCost, MemoryCost, BudgetBounds};
    ///
    /// struct NoLimits;
    ///
    /// impl CpuCost for NoLimits {
    ///     fn cpu_instruction_cost(&self) -> u64 { 9_999_999 }
    /// }
    /// impl MemoryCost for NoLimits {
    ///     fn memory_bytes_cost(&self) -> u64 { 9_999_999 }
    /// }
    /// impl BudgetBounds for NoLimits {
    ///     fn label(&self) -> &str { "no_limits" }
    ///     fn cpu_limit(&self) -> Option<u64> { None }
    ///     fn memory_limit(&self) -> Option<u64> { None }
    /// }
    ///
    /// // No limits configured → always within bounds.
    /// assert!(NoLimits.is_within_bounds().unwrap());
    /// ```
    fn is_within_bounds(&self) -> Result<bool, String> {
        let cpu_ok = match self.cpu_limit() {
            Some(limit) => self.cpu_instruction_cost() < limit,
            None => true,
        };
        let mem_ok = match self.memory_limit() {
            Some(limit) => self.memory_bytes_cost() < limit,
            None => true,
        };
        Ok(cpu_ok && mem_ok)
    }
}

/// A type that can be formatted as a single cost-report entry.
///
/// This trait provides a serialization-friendly representation of one metric
/// measurement, suitable for inclusion in a `CostReport` row or in
/// CSV/JSON output consumed by CI pipelines.
///
/// # Usage
///
/// ```rust
/// use soroban_budget_assert_core::module_21::CostReportable;
///
/// /// One row in a budget report.
/// struct ReportRow {
///     function: String,
///     metric: String,
///     value: u64,
/// }
///
/// impl CostReportable for ReportRow {
///     fn function_name(&self) -> &str { &self.function }
///     fn metric_name(&self) -> &str { &self.metric }
///     fn metric_value(&self) -> u64 { self.value }
/// }
///
/// let row = ReportRow {
///     function: "swap".into(),
///     metric: "CPU Instructions".into(),
///     value: 250_000,
/// };
/// assert_eq!(row.function_name(), "swap");
/// assert_eq!(row.metric_name(), "CPU Instructions");
/// assert_eq!(row.metric_value(), 250_000);
/// ```
pub trait CostReportable {
    /// Returns the function name associated with this report entry.
    ///
    /// This is the fully qualified name of the contract function, for example
    /// `"amm-pool-contract::swap"`.
    fn function_name(&self) -> &str;

    /// Returns the metric name (e.g., `"CPU Instructions"`, `"Read Bytes"`,
    /// `"Write Bytes"`, or `"WASM Bytes"`).
    ///
    /// The metric name is used as a column header in plain-text tables and as
    /// a key in JSON output.
    fn metric_name(&self) -> &str;

    /// Returns the measured metric value.
    ///
    /// The semantic meaning of this value depends on [`metric_name`]:
    ///
    /// * `"CPU Instructions"` — number of CPU instructions.
    /// * `"Read Bytes"` — number of bytes read from storage.
    /// * `"Write Bytes"` — number of bytes written to storage.
    /// * `"WASM Bytes"` — compiled WASM binary size in bytes.
    ///
    /// [`metric_name`]: CostReportable::metric_name
    fn metric_value(&self) -> u64;

    /// Serialize this report entry to a JSON string.
    ///
    /// The default implementation produces a lightweight JSON object with
    /// `function`, `metric`, and `value` keys:
    ///
    /// ```json
    /// {"function":"swap","metric":"CPU Instructions","value":250000}
    /// ```
    ///
    /// Implementors may override this method to include additional fields
    /// such as limit and pass/fail status.
    fn to_json(&self) -> String {
        format!(
            r#"{{"function":"{}","metric":"{}","value":{}}}"#,
            self.function_name(),
            self.metric_name(),
            self.metric_value()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── CpuCost tests ──────────────────────────────────────────────────────

    #[test]
    fn cpu_cost_returns_stored_value() {
        struct Fake {
            cpu: u64,
        }
        impl CpuCost for Fake {
            fn cpu_instruction_cost(&self) -> u64 {
                self.cpu
            }
        }
        assert_eq!(Fake { cpu: 0 }.cpu_instruction_cost(), 0);
        assert_eq!(Fake { cpu: 1 }.cpu_instruction_cost(), 1);
        assert_eq!(Fake { cpu: u64::MAX }.cpu_instruction_cost(), u64::MAX);
    }

    // ── MemoryCost tests ───────────────────────────────────────────────────

    #[test]
    fn memory_cost_returns_stored_value() {
        struct Fake {
            mem: u64,
        }
        impl MemoryCost for Fake {
            fn memory_bytes_cost(&self) -> u64 {
                self.mem
            }
        }
        assert_eq!(Fake { mem: 0 }.memory_bytes_cost(), 0);
        assert_eq!(Fake { mem: 512 }.memory_bytes_cost(), 512);
        assert_eq!(Fake { mem: u64::MAX }.memory_bytes_cost(), u64::MAX);
    }

    // ── BudgetBounds tests ─────────────────────────────────────────────────

    #[test]
    fn is_within_bounds_ok_when_below_limits() {
        struct Bounds {
            cpu: u64,
            mem: u64,
            cpu_limit: Option<u64>,
            mem_limit: Option<u64>,
        }
        impl CpuCost for Bounds {
            fn cpu_instruction_cost(&self) -> u64 {
                self.cpu
            }
        }
        impl MemoryCost for Bounds {
            fn memory_bytes_cost(&self) -> u64 {
                self.mem
            }
        }
        impl BudgetBounds for Bounds {
            fn label(&self) -> &str {
                "test"
            }
            fn cpu_limit(&self) -> Option<u64> {
                self.cpu_limit
            }
            fn memory_limit(&self) -> Option<u64> {
                self.mem_limit
            }
        }

        let b = Bounds {
            cpu: 500,
            mem: 200,
            cpu_limit: Some(1000),
            mem_limit: Some(500),
        };
        assert!(b.is_within_bounds().unwrap());
    }

    #[test]
    fn is_within_bounds_fails_when_cpu_exceeds() {
        struct Bounds {
            cpu: u64,
            mem: u64,
            cpu_limit: Option<u64>,
            mem_limit: Option<u64>,
        }
        impl CpuCost for Bounds {
            fn cpu_instruction_cost(&self) -> u64 {
                self.cpu
            }
        }
        impl MemoryCost for Bounds {
            fn memory_bytes_cost(&self) -> u64 {
                self.mem
            }
        }
        impl BudgetBounds for Bounds {
            fn label(&self) -> &str {
                "test"
            }
            fn cpu_limit(&self) -> Option<u64> {
                self.cpu_limit
            }
            fn memory_limit(&self) -> Option<u64> {
                self.mem_limit
            }
        }

        let b = Bounds {
            cpu: 1500,
            mem: 200,
            cpu_limit: Some(1000),
            mem_limit: Some(500),
        };
        assert!(!b.is_within_bounds().unwrap());
    }

    #[test]
    fn is_within_bounds_passes_when_no_limits() {
        struct Bounds {
            cpu: u64,
            mem: u64,
        }
        impl CpuCost for Bounds {
            fn cpu_instruction_cost(&self) -> u64 {
                self.cpu
            }
        }
        impl MemoryCost for Bounds {
            fn memory_bytes_cost(&self) -> u64 {
                self.mem
            }
        }
        impl BudgetBounds for Bounds {
            fn label(&self) -> &str {
                "test"
            }
            fn cpu_limit(&self) -> Option<u64> {
                None
            }
            fn memory_limit(&self) -> Option<u64> {
                None
            }
        }

        assert!(Bounds {
            cpu: u64::MAX,
            mem: u64::MAX
        }
        .is_within_bounds()
        .unwrap());
    }

    // ── CostReportable tests ───────────────────────────────────────────────

    #[test]
    fn cost_reportable_default_to_json() {
        struct Row {
            function: String,
            metric: String,
            value: u64,
        }
        impl CostReportable for Row {
            fn function_name(&self) -> &str {
                &self.function
            }
            fn metric_name(&self) -> &str {
                &self.metric
            }
            fn metric_value(&self) -> u64 {
                self.value
            }
        }

        let r = Row {
            function: "swap".into(),
            metric: "CPU Instructions".into(),
            value: 250_000,
        };
        let json = r.to_json();
        assert!(json.contains(r#""function":"swap""#));
        assert!(json.contains(r#""metric":"CPU Instructions""#));
        assert!(json.contains(r#""value":250000"#));
    }

    #[test]
    fn cost_reportable_custom_to_json() {
        struct CustomRow {
            function: String,
            metric: String,
            value: u64,
            limit: Option<u64>,
            pass: bool,
        }
        impl CostReportable for CustomRow {
            fn function_name(&self) -> &str {
                &self.function
            }
            fn metric_name(&self) -> &str {
                &self.metric
            }
            fn metric_value(&self) -> u64 {
                self.value
            }

            fn to_json(&self) -> String {
                let limit = self
                    .limit
                    .map(|l| l.to_string())
                    .unwrap_or_else(|| "null".into());
                format!(
                    r#"{{"function":"{}","metric":"{}","value":{},"limit":{},"pass":{}}}"#,
                    self.function_name(),
                    self.metric_name(),
                    self.metric_value(),
                    limit,
                    self.pass,
                )
            }
        }

        let r = CustomRow {
            function: "deposit".into(),
            metric: "Read Bytes".into(),
            value: 2048,
            limit: Some(5000),
            pass: true,
        };
        let json = r.to_json();
        assert!(json.contains(r#""function":"deposit""#));
        assert!(json.contains(r#""metric":"Read Bytes""#));
        assert!(json.contains(r#""value":2048"#));
        assert!(json.contains(r#""limit":5000"#));
        assert!(json.contains(r#""pass":true"#));
    }
}
