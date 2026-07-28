//! # Soroban Budget Assert — Core Module
//!
//! This crate provides the foundational traits and types for cost measurement,
//! budget assertion, and resource reporting. See [`module_1`] for the full
//! trait documentation and usage examples.
//!
//! ## Modules
//!
//! - [`module_1`] — Core cost-measurement, budget-assertion, and
//!   reporting traits and types.
//! - [`module_25`] — Optimized per-operation state-tracking backends
//!   (linear vs. hash-based) with paired benchmarks.

pub mod module_1;
pub mod module_25;

pub use module_1::*;
pub use module_25::*;
