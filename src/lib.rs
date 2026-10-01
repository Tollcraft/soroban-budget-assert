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

pub mod impls;
pub mod state_tracking;
pub mod traits;

pub use state_tracking::*;
pub use traits::*;
