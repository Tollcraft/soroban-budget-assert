//! Canonical error handling for `cargo-budget-report`.
//!
//! Provides the [`Error`] enum that captures every failure mode of the
//! reporting pipeline, a convenience [`Result`] alias, and a [`Context`]
//! extension trait that mirrors `anyhow::Context` so callers can attach
//! human-readable context to errors without pulling in `anyhow`.
//!
//! # Relationship to `module_10`
//!
//! This module subsumes and extends `module_10`.  `module_10` is kept as a
//! thin compatibility shim that re-exports the canonical types from here.
//!
//! # Integration
//!
//! The [`Error`] type implements [`std::error::Error`] (and therefore
//! `Send + Sync`), so it can be converted to `anyhow::Error` via the `?`
//! operator when the caller returns `anyhow::Result`.  The `main()`
//! function is the natural place to keep that outer `anyhow::Result` return
//! type; all intermediate functions use [`crate::module_30::Result`].

use std::fmt;

// ── Error enum ────────────────────────────────────────────────────────────

/// The error type for `cargo-budget-report` operations.
///
/// Each variant corresponds to a class of failures that can occur during
/// budget simulation and reporting: I/O, (de)serialisation, CLI execution,
/// RPC communication, contract simulation, CSV output, and catch-all
/// messages.
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
    /// CSV serialisation or I/O failure.
    Csv(String),
    /// Generic error message (replaces ad-hoc `anyhow::bail!` call-sites).
    Message(String),
}

/// Convenience alias for `std::result::Result<T, Error>`.
pub type Result<T> = std::result::Result<T, Error>;

// ── Display ────────────────────────────────────────────────────────────────

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
            Error::Csv(msg) => write!(f, "CSV error: {}", msg),
            Error::Message(msg) => write!(f, "{}", msg),
        }
    }
}

// ── std::error::Error ──────────────────────────────────────────────────────

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

// ── Context extension trait ────────────────────────────────────────────────

/// Extension trait that provides `context` and `with_context` methods on
/// [`Result`] values, mirroring `anyhow::Context`.
///
/// # Examples
///
/// ```ignore
/// use crate::module_30::Context;
///
/// let contents = std::fs::read_to_string("budget.toml")
///     .context("failed to read budget.toml")?;
/// ```
pub trait Context<T> {
    /// Wrap the error value with additional context.
    fn context<C: fmt::Display>(self, ctx: C) -> Result<T>;

    /// Wrap the error value with additional context that is evaluated
    /// lazily only once an error does occur.
    fn with_context<C, F>(self, f: F) -> Result<T>
    where
        C: fmt::Display,
        F: FnOnce() -> C;
}

impl<T> Context<T> for Result<T> {
    fn context<C: fmt::Display>(self, ctx: C) -> Result<T> {
        self.map_err(|e| Error::Message(format!("{}: {:#}", ctx, e)))
    }

    fn with_context<C, F>(self, f: F) -> Result<T>
    where
        C: fmt::Display,
        F: FnOnce() -> C,
    {
        self.map_err(|e| Error::Message(format!("{}: {:#}", f(), e)))
    }
}

/// Also implement `Context` for `std::result::Result<T, E>` where `E:
/// std::error::Error + Send + Sync + 'static`, so callers can use
/// `.context()` on any error type that can convert into our `Error`.
impl<T, E> Context<T> for std::result::Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn context<C: fmt::Display>(self, ctx: C) -> Result<T> {
        self.map_err(|e| Error::Message(format!("{}: {:#}", ctx, e)))
    }

    fn with_context<C, F>(self, f: F) -> Result<T>
    where
        C: fmt::Display,
        F: FnOnce() -> C,
    {
        self.map_err(|e| Error::Message(format!("{}: {:#}", f(), e)))
    }
}

// ── From impls (external error types → our Error) ─────────────────────────

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

impl From<wasmparser::BinaryReaderError> for Error {
    fn from(e: wasmparser::BinaryReaderError) -> Self {
        Error::Message(e.message().to_string())
    }
}

impl From<stellar_xdr::curr::Error> for Error {
    fn from(e: stellar_xdr::curr::Error) -> Self {
        Error::Xdr(e.to_string())
    }
}

impl From<csv::Error> for Error {
    fn from(e: csv::Error) -> Self {
        Error::Csv(e.to_string())
    }
}

impl From<csv::IntoInnerError<csv::Writer<Vec<u8>>>> for Error {
    fn from(e: csv::IntoInnerError<csv::Writer<Vec<u8>>>) -> Self {
        Error::Csv(e.to_string())
    }
}

impl From<std::fmt::Error> for Error {
    fn from(e: std::fmt::Error) -> Self {
        Error::Message(format!("formatting error: {}", e))
    }
}

// ── Convenience constructors ───────────────────────────────────────────────

impl Error {
    /// Create an `Error::Message` from a format string and arguments.
    ///
    /// This mirrors `anyhow::anyhow!()` for call-sites that need to build
    /// an error message dynamically.
    #[allow(dead_code)]
    pub fn msg(args: std::fmt::Arguments<'_>) -> Self {
        Error::Message(args.to_string())
    }
}

/// Equivalent to `anyhow::bail!()` — returns an `Error::Message` with the
/// formatted string.
#[macro_export]
macro_rules! bail {
    ($($arg:tt)*) => {
        return Err($crate::module_30::Error::Message(format!($($arg)*)))
    };
}

/// Equivalent to `anyhow::anyhow!()` — creates an `Error::Message` with the
/// formatted string.
#[macro_export]
macro_rules! anyhow_err {
    ($($arg:tt)*) => {
        $crate::module_30::Error::Message(format!($($arg)*))
    };
}

// ── Simulation types ───────────────────────────────────────────────────────

/// Outcome of simulating one exported function.
pub enum SimulationOutcome {
    /// Successfully extracted resource metrics.
    Metrics {
        instructions: u32,
        read_bytes: u32,
        write_bytes: u32,
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

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    // ── Error Display ──────────────────────────────────────────────────

    #[test]
    fn error_display_io() {
        let err = Error::Io(io::Error::new(io::ErrorKind::NotFound, "file not found"));
        let msg = err.to_string();
        assert!(msg.contains("I/O error"));
        assert!(msg.contains("file not found"));
    }

    #[test]
    fn error_display_xdr() {
        let err = Error::Xdr("invalid base64".into());
        assert!(err.to_string().contains("XDR decode error"));
        assert!(err.to_string().contains("invalid base64"));
    }

    #[test]
    fn error_display_json() {
        let err = Error::Json(serde_json::from_str::<serde_json::Value>("{").unwrap_err());
        assert!(err.to_string().contains("JSON error"));
    }

    #[test]
    fn error_display_toml() {
        let err = Error::Toml(toml::from_str::<toml::Value>("[").unwrap_err());
        assert!(err.to_string().contains("TOML error"));
    }

    #[test]
    fn error_display_missing_field() {
        let err = Error::MissingField("transactionData".into());
        assert!(err.to_string().contains("missing required field"));
        assert!(err.to_string().contains("transactionData"));
    }

    #[test]
    fn error_display_rpc() {
        let err = Error::Rpc("Invalid Request".into());
        assert!(err.to_string().contains("RPC error"));
        assert!(err.to_string().contains("Invalid Request"));
    }

    #[test]
    fn error_display_command_failed() {
        let err = Error::CommandFailed("stellar not found".into());
        assert!(err.to_string().contains("command failed"));
        assert!(err.to_string().contains("stellar not found"));
    }

    #[test]
    fn error_display_csv() {
        let err = Error::Csv("bad record".into());
        assert!(err.to_string().contains("CSV error"));
        assert!(err.to_string().contains("bad record"));
    }

    #[test]
    fn error_display_message() {
        let err = Error::Message("something went wrong".into());
        assert_eq!(err.to_string(), "something went wrong");
    }

    // ── Error source ───────────────────────────────────────────────────

    #[test]
    fn error_source_io() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "denied");
        let err = Error::Io(io_err);
        let source = std::error::Error::source(&err);
        assert!(source.is_some());
    }

    #[test]
    fn error_source_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
        let err = Error::Json(json_err);
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn error_source_toml() {
        let toml_err = toml::from_str::<toml::Value>("[").unwrap_err();
        let err = Error::Toml(toml_err);
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn error_source_message_is_none() {
        let err = Error::Message("plain".into());
        assert!(std::error::Error::source(&err).is_none());
    }

    #[test]
    fn error_source_rpc_is_none() {
        let err = Error::Rpc("something".into());
        assert!(std::error::Error::source(&err).is_none());
    }

    #[test]
    fn error_source_missing_field_is_none() {
        let err = Error::MissingField("x".into());
        assert!(std::error::Error::source(&err).is_none());
    }

    // ── From impls ─────────────────────────────────────────────────────

    #[test]
    fn from_io_error() {
        let io_err = io::Error::new(io::ErrorKind::Other, "test");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));
    }

    #[test]
    fn from_string() {
        let err: Error = String::from("test message").into();
        assert!(matches!(err, Error::Message(_)));
    }

    #[test]
    fn from_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
        let err: Error = json_err.into();
        assert!(matches!(err, Error::Json(_)));
    }

    #[test]
    fn from_toml_error() {
        let toml_err = toml::from_str::<toml::Value>("[").unwrap_err();
        let err: Error = toml_err.into();
        assert!(matches!(err, Error::Toml(_)));
    }

    // ── Context trait ──────────────────────────────────────────────────

    #[test]
    fn context_adds_message_on_err() {
        let result: Result<()> = Err(Error::Message("inner".into()));
        let wrapped = result.context("outer");
        assert!(wrapped.is_err());
        let msg = wrapped.unwrap_err().to_string();
        assert!(msg.contains("outer"));
        assert!(msg.contains("inner"));
    }

    #[test]
    fn context_passes_through_ok() {
        let result: Result<i32> = Ok(42);
        let wrapped = result.context("should not appear");
        assert_eq!(wrapped.unwrap(), 42);
    }

    #[test]
    fn with_context_adds_message_on_err() {
        let result: Result<()> = Err(Error::Message("inner".into()));
        let wrapped = result.with_context(|| format!("ctx {}", 1));
        assert!(wrapped.is_err());
        let msg = wrapped.unwrap_err().to_string();
        assert!(msg.contains("ctx 1"));
        assert!(msg.contains("inner"));
    }

    #[test]
    fn with_context_is_lazy() {
        let mut called = false;
        let result: Result<i32> = Ok(42);
        let _ = result.with_context(|| {
            called = true;
            "should not be evaluated"
        });
        assert!(!called, "with_context closure should not run on Ok");
    }

    #[test]
    fn context_on_std_result_io_error() {
        let result: std::result::Result<(), io::Error> =
            Err(io::Error::new(io::ErrorKind::NotFound, "nope"));
        let wrapped: Result<()> = result.context("file op failed");
        assert!(wrapped.is_err());
        let msg = wrapped.unwrap_err().to_string();
        assert!(msg.contains("file op failed"));
        assert!(msg.contains("nope"));
    }

    #[test]
    fn context_on_std_result_ok() {
        let result: std::result::Result<i32, io::Error> = Ok(99);
        let wrapped: Result<i32> = result.context("ignored");
        assert_eq!(wrapped.unwrap(), 99);
    }

    // ── bail! macro ────────────────────────────────────────────────────

    #[test]
    fn bail_macro_returns_err() {
        fn inner() -> Result<()> {
            bail!("something {}", "bad");
        }
        let result = inner();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("something bad"));
    }

    // ── anyhow_err! macro ──────────────────────────────────────────────

    #[test]
    fn anyhow_err_macro_creates_error() {
        let err = anyhow_err!("failed at line {}", 42);
        assert!(matches!(err, Error::Message(_)));
        assert!(err.to_string().contains("failed at line 42"));
    }

    // ── SimulationFailure debug display ─────────────────────────────────

    #[test]
    fn simulation_failure_invoke_debug() {
        let sf = SimulationFailure::Invoke("exit code 1".into());
        let dbg = format!("{:?}", sf);
        assert!(dbg.contains("Invoke"));
        assert!(dbg.contains("exit code 1"));
    }

    #[test]
    fn simulation_failure_rpc_debug() {
        let sf = SimulationFailure::Rpc("timeout".into());
        let dbg = format!("{:?}", sf);
        assert!(dbg.contains("Rpc"));
        assert!(dbg.contains("timeout"));
    }

    #[test]
    fn simulation_failure_metrics_extraction_debug() {
        let sf = SimulationFailure::MetricsExtraction("decode failed".into());
        let dbg = format!("{:?}", sf);
        assert!(dbg.contains("MetricsExtraction"));
        assert!(dbg.contains("decode failed"));
    }

    // ── Send + Sync bounds ─────────────────────────────────────────────

    /// Compile-time assertion that `Error` is `Send + Sync`.
    #[test]
    fn error_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Error>();
    }

    // ── Round-trip: std error → our Error → anyhow ─────────────────────

    #[test]
    fn error_converts_to_anyhow() {
        let err = Error::MissingField("x".into());
        let anyhow_err: anyhow::Error = err.into();
        let msg = format!("{:#}", anyhow_err);
        assert!(msg.contains("missing required field"));
    }
}
