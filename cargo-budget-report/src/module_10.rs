//! Consolidated error handling for `cargo-budget-report`.
//!
//! Provides the canonical [`Error`] enum that captures every failure mode
//! of the reporting pipeline, along with a convenience [`Result`] alias so
//! internal functions always speak the same error language.
//!
//! # Integration
//!
//! The [`Error`] type implements [`std::error::Error`] (and therefore
//! `Send + Sync`), so it can be converted to `anyhow::Error` via the `?`
//! operator when the caller returns `anyhow::Result`.  The `main()`
//! function is the natural place to keep that outer `anyhow::Result` return
//! type; all intermediate functions use [`crate::module_10::Result`].

use std::fmt;

/// The error type for `cargo-budget-report` operations.
///
/// Each variant corresponds to a class of failures that can occur during
/// budget simulation and reporting: I/O, (de)serialisation, CLI execution,
/// RPC communication, contract simulation, and catch-all messages.
#[derive(Debug)]
pub enum Error {
    /// An I/O operation failed (file read/write, etc.).
    Io(std::io::Error),
    /// Stellar XDR base64 decode / encode failure.
    Xdr(String),
    /// JSON parse or deserialise failure.
    Json(serde_json::Error),
    /// TOML parse failure.
    Toml(toml::de::Error),
    /// A required field was missing from a response or configuration file.
    MissingField(String),
    /// The RPC endpoint returned an error response.
    Rpc(String),
    /// A CLI command (`stellar`, `curl`) could not be spawned or exited
    /// with a non-zero status.
    CommandFailed(String),
    /// Generic error message (replaces ad-hoc `anyhow::bail!` call-sites).
    Message(String),
}

/// Convenience alias for `std::result::Result<T, Error>`.
pub type Result<T> = std::result::Result<T, Error>;

// ── Display ────────────────────────────────────────────────────────────

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O error: {}", e),
            Error::Xdr(msg) => write!(f, "XDR decode error: {}", msg),
            Error::Json(e) => write!(f, "JSON error: {}", e),
            Error::Toml(e) => write!(f, "TOML error: {}", e),
            Error::MissingField(field) => {
                write!(f, "missing required field: {}", field)
            }
            Error::Rpc(msg) => write!(f, "RPC error: {}", msg),
            Error::CommandFailed(msg) => write!(f, "command failed: {}", msg),
            Error::Message(msg) => write!(f, "{}", msg),
        }
    }
}

// ── std::error::Error ──────────────────────────────────────────────────

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Json(e) => Some(e),
            Error::Toml(e) => Some(e),
            _ => None,
        }
    }
}

// ── From impls (own error types → custom Error) ────────────────────────

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e)
    }
}

impl From<toml::de::Error> for Error {
    fn from(e: toml::de::Error) -> Self {
        Error::Toml(e)
    }
}

impl From<String> for Error {
    fn from(msg: String) -> Self {
        Error::Message(msg)
    }
}

/// Make a boxed `std::error::Error` (from `wasmparser`) wrapable.
impl From<wasmparser::BinaryReaderError> for Error {
    fn from(e: wasmparser::BinaryReaderError) -> Self {
        Error::Message(e.message().to_string())
    }
}

/// Allow `?` to convert from `stellar_xdr::curr::Error` into ours.
impl From<stellar_xdr::curr::Error> for Error {
    fn from(e: stellar_xdr::curr::Error) -> Self {
        Error::Xdr(e.to_string())
    }
}

// ── Simulation types ───────────────────────────────────────────────────

/// Outcome of simulating one exported function.
///
/// On success, `transaction_data_xdr` carries the base64-encoded
/// `SorobanTransactionData` from the RPC response so that optional validation
/// (via `--validate`) can re-decode it through the Stellar CLI's own XDR
/// decoder without a second RPC call.
pub enum SimulationOutcome {
    /// Successfully extracted resource metrics.
    Metrics {
        instructions: u32,
        read_bytes: u32,
        write_bytes: u32,
        /// Base64-encoded `SorobanTransactionData` from the RPC response.
        transaction_data_xdr: String,
    },
    /// Simulation did not produce metrics (recoverable).
    Failed(SimulationFailure),
}

/// Single reason why a function simulation failed to produce metrics.
///
/// This is *not* an error variant of [`Error`] because these failures are
/// recoverable — the caller can move on to the next function instead of
/// aborting the whole report.
#[derive(Debug)]
pub enum SimulationFailure {
    /// `stellar contract invoke --build-only` exited non-zero.
    Invoke(String),
    /// The RPC `simulateTransaction` response contained an `"error"` field.
    Rpc(String),
    /// The RPC response didn't contain a decodable `SorobanTransactionData`.
    MetricsExtraction(String),
}
