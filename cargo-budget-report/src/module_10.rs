//! Backward-compatibility shim that re-exports the canonical error types
//! from [`crate::module_30`].
//!
//! New code should import directly from `crate::module_30`; this module
//! exists so that existing callers don't break while they are being
//! migrated.

pub use crate::module_30::{Context, Error, Result};
pub use crate::module_30::{SimulationFailure, SimulationOutcome};
