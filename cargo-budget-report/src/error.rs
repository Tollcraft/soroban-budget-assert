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
//! type; all intermediate functions use [`crate::error::Result`].

use std::fmt;

/// Exit codes surfaced by `cargo-budget-report` so that CI can tell failure
/// modes apart and respond differently.
///
/// Every value other than [`EXIT_SUCCESS`] means "the run failed", so scripts
/// that only care about pass/fail keep working (`$? -ne 0`). The codes are
/// chosen to avoid the values with reserved meaning in POSIX shells
/// (126, 127, and 128 + N for fatal signals), and to avoid clashing with
/// `clap`'s own exit code for argument-parse errors (2, used before `main`
/// runs). See `docs/src/ci_cd_integration.md` for the full table and an
/// example workflow.
pub const EXIT_SUCCESS: i32 = 0;
/// A failure that is neither a budget/regression result nor a network fault
/// (build failure, unexpected I/O, etc.). Kept at `1` so it reads as the
/// generic "something went wrong".
pub const EXIT_GENERIC_FAILURE: i32 = 1;
/// `budget.toml` is malformed, a required configuration value is missing, or
/// another configuration-level mistake was detected.
pub const EXIT_CONFIG_ERROR: i32 = 3;
/// A measured resource breached a configured `--check` limit (the "budget
/// exceeded" outcome).
pub const EXIT_BUDGET_EXCEEDED: i32 = 4;
/// A regression was detected beyond the configured tolerance when comparing
/// against a baseline.
pub const EXIT_REGRESSION: i32 = 5;
/// A network or infrastructure failure (RPC error, `stellar` CLI could not be
/// spawned, a simulation failed to produce metrics). Results from these runs
/// are unreliable and safe to retry.
pub const EXIT_NETWORK_FAILURE: i32 = 6;

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

/// Allow `?` to convert from `stellar_xdr::Error` into ours.
impl From<stellar_xdr::Error> for Error {
    fn from(e: stellar_xdr::Error) -> Self {
        Error::Xdr(e.to_string())
    }
}

/// Convert an `anyhow::Error` (e.g. one produced by `.context(...)`) into our
/// own error type. The original message is preserved; the concrete variant is
/// lost, so callers that need a precise exit code should surface a typed
/// [`Error`] before wrapping with context.
impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        Error::Message(e.to_string())
    }
}

impl Error {
    /// Map this error to the exit code a CI job should see.
    ///
    /// Configuration mistakes (`budget.toml` / required-field errors) become
    /// [`EXIT_CONFIG_ERROR`]; network or infrastructure faults (RPC, CLI
    /// command failures) become [`EXIT_NETWORK_FAILURE`]; everything else is
    /// the generic [`EXIT_GENERIC_FAILURE`]. Regression and budget-exceeded
    /// outcomes are *not* errors — they are computed from the report and
    /// handled separately by the caller.
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::Toml(_) | Error::MissingField(_) | Error::Json(_) => EXIT_CONFIG_ERROR,
            Error::Rpc(_) | Error::CommandFailed(_) => EXIT_NETWORK_FAILURE,
            Error::Io(_) | Error::Xdr(_) | Error::Message(_) => EXIT_GENERIC_FAILURE,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_errors_map_to_config_exit_code() {
        let toml_err = toml::from_str::<toml::Value>("not = valid toml !!").unwrap_err();
        assert_eq!(Error::Toml(toml_err).exit_code(), EXIT_CONFIG_ERROR);
        assert_eq!(
            Error::MissingField("field".into()).exit_code(),
            EXIT_CONFIG_ERROR
        );
        let json_err = serde_json::from_str::<serde_json::Value>("").unwrap_err();
        assert_eq!(Error::Json(json_err).exit_code(), EXIT_CONFIG_ERROR);
    }

    #[test]
    fn network_errors_map_to_network_exit_code() {
        assert_eq!(Error::Rpc("boom".into()).exit_code(), EXIT_NETWORK_FAILURE);
        assert_eq!(
            Error::CommandFailed("boom".into()).exit_code(),
            EXIT_NETWORK_FAILURE
        );
    }

    #[test]
    fn other_errors_map_to_generic_exit_code() {
        assert_eq!(
            Error::Io(std::io::Error::other("x")).exit_code(),
            EXIT_GENERIC_FAILURE
        );
        assert_eq!(Error::Xdr("x".into()).exit_code(), EXIT_GENERIC_FAILURE);
        assert_eq!(Error::Message("x".into()).exit_code(), EXIT_GENERIC_FAILURE);
    }

    #[test]
    fn exit_codes_avoid_shell_reserved_and_clap_values() {
        // Shells reserve 126, 127, and 128 + N (fatal signals). Keep our codes
        // below that range so they carry their intended meaning.
        for code in [
            EXIT_SUCCESS,
            EXIT_GENERIC_FAILURE,
            EXIT_CONFIG_ERROR,
            EXIT_BUDGET_EXCEEDED,
            EXIT_REGRESSION,
            EXIT_NETWORK_FAILURE,
        ] {
            assert!(
                code < 126,
                "exit code {code} collides with a shell-reserved value"
            );
        }
        // `clap` exits with code 2 on argument-parse errors, before `main`
        // runs. Keep our configuration-error code distinct from it.
        assert_ne!(EXIT_CONFIG_ERROR, 2);
        // Success must be exactly 0.
        assert_eq!(EXIT_SUCCESS, 0);
    }
}
