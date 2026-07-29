//! # Budget Assertion Traits
//!
//! This module defines the public trait abstractions that underpin the
//! `soroban-budget-assert` tooling. These traits capture the essential
//! interfaces for resolving budget limits and measuring resource consumption,
//! enabling new contributors to understand the contract without tracing
//! through macro-generated code.
//!
//! # Overview
//!
//! The budget assertion system works in two phases:
//!
//! 1. **Resolution** — a limit is resolved to a concrete `u64` value using one
//!    of several strategies (static literal, environment variable, or JSON
//!    configuration file). See [`BudgetLimit`] and [`BudgetLimitResolver`].
//! 2. **Measurement** — the Soroban host environment reports current resource
//!    consumption (CPU instructions and memory bytes). See
//!    [`ResourceMeasurer`].
//!
//! Both phases feed into an assertion that the measured cost is strictly less
//! than the resolved limit.

/// A strategy for resolving a budget limit to a concrete `u64` value at
/// runtime.
///
/// Implementations of this trait encapsulate the three resolution strategies
/// supported by the `budget_cpu_lt` and `budget_mem_lt` attribute macros:
///
/// * **Static** — a hard-coded literal (e.g. `500_000`).
/// * **Environment variable** — the limit is read from `std::env::var` and
///   falls back to `u64::MAX` when the variable is absent.
/// * **JSON configuration** — the limit is parsed from a `budget.json` file
///   in the current working directory.
///
/// # Examples
///
/// ## Implementing a custom resolver
///
/// ```rust,ignore
/// use cargo_budget_report::module_11::BudgetLimitResolver;
///
/// /// Wraps a file path to use as a budget limit source.
/// struct FileBasedLimit {
///     path: String,
/// }
///
/// impl BudgetLimitResolver for FileBasedLimit {
///     fn resolve(&self, _metric_label: &str) -> u64 {
///         std::fs::read_to_string(&self.path)
///             .ok()
///             .and_then(|s| s.trim().parse().ok())
///             .unwrap_or(u64::MAX)
///     }
/// }
/// ```
pub trait BudgetLimitResolver {
    /// Resolves the budget limit to a concrete `u64` value.
    ///
    /// # Parameters
    ///
    /// * `metric_label` — a human-readable name for the metric being resolved
    ///   (e.g. `"budget_cpu_lt"` or `"budget_mem_lt"`). This is used to
    ///   produce descriptive panic messages when resolution fails.
    ///
    /// # Returns
    ///
    /// A `u64` value representing the maximum allowed cost. Implementations
    /// may return `u64::MAX` to indicate "no limit" (the assertion will
    /// always pass).
    fn resolve(&self, metric_label: &str) -> u64;
}

/// A type that can report resource consumption from a Soroban host
/// environment.
///
/// This trait abstracts over `soroban_sdk::Env`'s budget measurement
/// capabilities, providing a narrow interface that the assertion macros
/// can use without coupling to the full SDK.
///
/// # Examples
///
/// ## Implementing `ResourceMeasurer` for a newtype wrapper
///
/// ```rust,ignore
/// use cargo_budget_report::module_11::{BudgetMetricKind, ResourceMeasurer};
/// use soroban_sdk::Env;
///
/// /// Newtype wrapper to implement `ResourceMeasurer` for `Env`.
/// struct MeasuredEnv(Env);
///
/// impl ResourceMeasurer for MeasuredEnv {
///     fn cpu_instructions(&self) -> u64 {
///         self.0.cost_estimate().budget().cpu_instruction_cost()
///     }
///
///     fn memory_bytes(&self) -> u64 {
///         self.0.cost_estimate().budget().memory_bytes_cost()
///     }
///
///     fn metric_name_for(&self, metric: BudgetMetricKind) -> &'static str {
///         match metric {
///             BudgetMetricKind::Cpu => "budget_cpu_lt",
///             BudgetMetricKind::Memory => "budget_mem_lt",
///         }
///     }
/// }
/// ```
pub trait ResourceMeasurer {
    /// Returns the current CPU instruction count consumed by the environment.
    fn cpu_instructions(&self) -> u64;

    /// Returns the current memory byte count consumed by the environment.
    fn memory_bytes(&self) -> u64;

    /// Returns the macro label associated with the given metric kind.
    ///
    /// This is used in panic messages to identify which assertion failed,
    /// e.g. `"budget_cpu_lt"` or `"budget_mem_lt"`.
    fn metric_name_for(&self, metric: BudgetMetricKind) -> &'static str;
}

/// Enumerates the kinds of budget metrics that can be asserted against.
///
/// Corresponds to the two measurements exposed by the Soroban host:
/// CPU instruction count and memory byte count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetMetricKind {
    /// CPU instruction cost, asserted by `#[budget_cpu_lt(N)]`.
    Cpu,
    /// Memory byte cost, asserted by `#[budget_mem_lt(N)]`.
    Memory,
}

impl BudgetMetricKind {
    /// Returns the attribute macro name that corresponds to this metric kind.
    ///
    /// # Examples
    ///
    /// ```
    /// use cargo_budget_report::module_11::BudgetMetricKind;
    ///
    /// assert_eq!(BudgetMetricKind::Cpu.macro_name(), "budget_cpu_lt");
    /// assert_eq!(BudgetMetricKind::Memory.macro_name(), "budget_mem_lt");
    /// ```
    pub fn macro_name(self) -> &'static str {
        match self {
            BudgetMetricKind::Cpu => "budget_cpu_lt",
            BudgetMetricKind::Memory => "budget_mem_lt",
        }
    }

    /// Returns the Soroban budget method name used to read this metric.
    ///
    /// # Examples
    ///
    /// ```
    /// use cargo_budget_report::module_11::BudgetMetricKind;
    ///
    /// assert_eq!(BudgetMetricKind::Cpu.method_name(), "cpu_instruction_cost");
    /// assert_eq!(BudgetMetricKind::Memory.method_name(), "memory_bytes_cost");
    /// ```
    pub fn method_name(self) -> &'static str {
        match self {
            BudgetMetricKind::Cpu => "cpu_instruction_cost",
            BudgetMetricKind::Memory => "memory_bytes_cost",
        }
    }
}

/// A composable budget assertion that combines a [`BudgetLimitResolver`] with
/// a [`ResourceMeasurer`] to enforce cost ceilings.
///
/// This trait provides the `assert_within_budget` method, which is the core
/// assertion logic used by both `budget_cpu_lt` and `budget_mem_lt`.
///
/// # Safety
///
/// Local estimates from raw Rust or unoptimised WASM may differ significantly
/// from real network costs. This trait is designed for fast local regression
/// gating; use `cargo budget-report` for network-verified numbers.
///
/// # Examples
///
/// ```rust,ignore
/// use cargo_budget_report::module_11::{BudgetAssertion, BudgetMetricKind, BudgetLimitResolver};
///
/// // Assume `measured_env` is a newtype wrapper around `soroban_sdk::Env`
/// // that implements `ResourceMeasurer`.
/// # struct MeasuredEnv;
/// # impl MeasuredEnv {
/// #     fn new() -> Self { MeasuredEnv }
/// # }
///
/// // Resolve a limit from an environment variable:
/// struct EnvLimit;
/// impl BudgetLimitResolver for EnvLimit {
///     fn resolve(&self, _label: &str) -> u64 {
///         std::env::var("MAX_CPU")
///             .ok()
///             .and_then(|s| s.parse().ok())
///             .unwrap_or(u64::MAX)
///     }
/// }
///
/// // Assert that the environment is within budget:
/// let assertion = BudgetAssertion::new(EnvLimit, BudgetMetricKind::Cpu);
/// // assertion.assert_within_budget(&measured_env);
/// ```
pub struct BudgetAssertion<R: BudgetLimitResolver> {
    resolver: R,
    metric: BudgetMetricKind,
}

impl<R: BudgetLimitResolver> BudgetAssertion<R> {
    /// Creates a new budget assertion with the given resolver and metric kind.
    ///
    /// # Parameters
    ///
    /// * `resolver` — the strategy used to obtain the upper bound.
    /// * `metric` — which resource dimension to measure.
    pub fn new(resolver: R, metric: BudgetMetricKind) -> Self {
        BudgetAssertion { resolver, metric }
    }

    /// Asserts that the measured cost reported by `env` is strictly less than
    /// the limit resolved by this assertion's [`BudgetLimitResolver`].
    ///
    /// # Parameters
    ///
    /// * `env` — any type implementing [`ResourceMeasurer`] (typically a
    ///   `soroban_sdk::Env`).
    ///
    /// # Panics
    ///
    /// Panics with a descriptive message if the measured cost equals or exceeds
    /// the resolved limit. The message includes the macro name, the measured
    /// value, and the limit.
    pub fn assert_within_budget(&self, env: &dyn ResourceMeasurer) {
        let label = env.metric_name_for(self.metric);
        let limit = self.resolver.resolve(label);
        let cost = match self.metric {
            BudgetMetricKind::Cpu => env.cpu_instructions(),
            BudgetMetricKind::Memory => env.memory_bytes(),
        };

        let metric_desc = match self.metric {
            BudgetMetricKind::Cpu => "CPU instruction cost",
            BudgetMetricKind::Memory => "Memory bytes cost",
        };

        assert!(
            cost < limit,
            "{} {} exceeded limit {} - local estimate, real network cost may differ significantly in either direction",
            metric_desc,
            cost,
            limit,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// A mock resource measurer that reports fixed, configurable values.
    struct MockMeasurer {
        cpu: Cell<u64>,
        mem: Cell<u64>,
    }

    impl MockMeasurer {
        fn new(cpu: u64, mem: u64) -> Self {
            MockMeasurer {
                cpu: Cell::new(cpu),
                mem: Cell::new(mem),
            }
        }
    }

    impl ResourceMeasurer for MockMeasurer {
        fn cpu_instructions(&self) -> u64 {
            self.cpu.get()
        }

        fn memory_bytes(&self) -> u64 {
            self.mem.get()
        }

        fn metric_name_for(&self, metric: BudgetMetricKind) -> &'static str {
            metric.macro_name()
        }
    }

    /// A resolver that always returns a fixed limit.
    struct FixedLimit(u64);

    impl BudgetLimitResolver for FixedLimit {
        fn resolve(&self, _label: &str) -> u64 {
            self.0
        }
    }

    #[test]
    fn budget_metric_kind_macro_names() {
        assert_eq!(BudgetMetricKind::Cpu.macro_name(), "budget_cpu_lt");
        assert_eq!(BudgetMetricKind::Memory.macro_name(), "budget_mem_lt");
    }

    #[test]
    fn budget_metric_kind_method_names() {
        assert_eq!(BudgetMetricKind::Cpu.method_name(), "cpu_instruction_cost");
        assert_eq!(BudgetMetricKind::Memory.method_name(), "memory_bytes_cost");
    }

    #[test]
    fn assertion_passes_when_cost_is_below_limit() {
        let env = MockMeasurer::new(100, 50);
        let assertion = BudgetAssertion::new(FixedLimit(500), BudgetMetricKind::Cpu);
        assertion.assert_within_budget(&env);
    }

    #[test]
    #[should_panic(
        expected = "local estimate, real network cost may differ significantly in either direction"
    )]
    fn assertion_panics_when_cost_exceeds_limit() {
        let env = MockMeasurer::new(1_000_000, 50);
        let assertion = BudgetAssertion::new(FixedLimit(1_000), BudgetMetricKind::Cpu);
        assertion.assert_within_budget(&env);
    }

    #[test]
    #[should_panic(
        expected = "local estimate, real network cost may differ significantly in either direction"
    )]
    fn assertion_panics_when_mem_cost_exceeds_limit() {
        let env = MockMeasurer::new(100, 500_000);
        let assertion = BudgetAssertion::new(FixedLimit(1_000), BudgetMetricKind::Memory);
        assertion.assert_within_budget(&env);
    }

    #[test]
    fn fixed_limit_resolver_returns_configured_value() {
        let resolver = FixedLimit(42);
        assert_eq!(resolver.resolve("test"), 42);
    }

    #[test]
    fn resolver_can_return_u64_max_for_no_limit() {
        let resolver = FixedLimit(u64::MAX);
        assert_eq!(resolver.resolve("test"), u64::MAX);
    }
}
