//! # Soroban Budget Assert — Core Module
//!
//! This crate provides the foundational traits and types for cost measurement,
//! budget assertion, and resource reporting. See the [`traits`] module for the full
//! API documentation and usage examples.
//!
//! The implementation is split by responsibility:
//! - [`traits`] contains the core measurement, assertion, and reporting contracts.
//! - [`impls`] contains the standalone cost and budget abstractions.
//! - [`state_tracking`] contains the linear and hashed tracking backends.
//!
//! The commonly used tracking and trait APIs are re-exported at the crate root;
//! use the module paths when selecting a specific implementation abstraction.
//!
//! ## Crate status: internal groundwork, deliberately kept
//!
//! This package is not published to crates.io (`publish = false`) and no shipped
//! artifact links it: neither the macros nor the contracts reference these items,
//! and the CLI names it only under `[dev-dependencies]`, for a single test. The
//! remaining consumers are the integration tests under `tests/`.
//!
//! It is kept rather than retired because it is the home for behaviour those
//! artifacts are expected to need. It becomes load-bearing — and this note must
//! be revised — as soon as any of the following happens:
//!
//! 1. the macros emit assertions against [`BudgetAssert`] / [`impls::BudgetBounds`];
//! 2. the CLI moves a measurement or reporting path onto [`CostMeasurer`] /
//!    [`ResourceReportable`];
//! 3. any shipped crate takes this crate as a normal dependency.
//!
//! `tests/core_crate_contract.rs` pins the packaging side of that decision: the
//! package stays unpublished, and every workspace manifest that names it does so
//! only under `[dev-dependencies]`. Promoting it to a normal dependency fails
//! that test until this note and the manifest comment are updated to match.

pub mod impls;
pub mod state_tracking;
pub mod traits;

pub use state_tracking::*;
pub use traits::*;
