//! # Module: Consolidated Error Handling (#20)
//!
//! Demonstrates and enforces consistent error handling across the budget
//! report pipeline by using the canonical [`crate::module_10::Result`] and
//! [`crate::module_10::Error`] types defined in module_10.
//!
//! All fallible operations in this module return the custom `Result<T>`
//! alias rather than panicking with `unwrap()`, `expect()`, or ad-hoc
//! error strings. Each error case is mapped to a semantically appropriate
//! variant of [`Error`].

// `pub` items in this module are referenced only by its own `#[cfg(test)] mod tests`
// and demo callers (issue #20 was a documentation/demonstration contribution), so
// the binary target does not link them through `fn main`. Allow dead code at file
// scope rather than auditing per-item; the public API is still exercisable from
// `cargo test`.
#![allow(dead_code)]

use crate::module_10::{Error, Result};
use std::path::Path;

/// Parses a margin string (e.g. `"0.15"` or `"1.25"`) into a multiplier.
///
/// # Errors
///
/// Returns `Error::Message` if the string cannot be parsed as a valid `f64`.
pub fn parse_margin_multiplier(raw: &str) -> Result<f64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(Error::Message("margin multiplier string is empty".into()));
    }
    trimmed
        .parse::<f64>()
        .map_err(|e| Error::Message(format!("invalid margin multiplier '{}': {}", raw, e)))
}

/// Validates that a margin multiplier is within the expected range.
///
/// The multiplier must be strictly positive and not exceed a reasonable upper
/// bound (currently 100.0).
///
/// # Errors
///
/// Returns `Error::Message` if the multiplier is zero, negative, or exceeds
/// the maximum allowed value.
pub fn validate_margin_multiplier(multiplier: f64) -> Result<f64> {
    if multiplier <= 0.0 {
        return Err(Error::Message(format!(
            "margin multiplier must be positive, got {}",
            multiplier
        )));
    }
    if multiplier > 100.0 {
        return Err(Error::Message(format!(
            "margin multiplier {} exceeds maximum allowed value of 100.0",
            multiplier
        )));
    }
    Ok(multiplier)
}

/// Supported metric kinds for budget limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricKind {
    CpuInstructions,
    ReadBytes,
    WriteBytes,
}

impl MetricKind {
    /// Returns the display label for this metric.
    pub fn label(&self) -> &'static str {
        match self {
            MetricKind::CpuInstructions => "CPU Instructions",
            MetricKind::ReadBytes => "Read Bytes",
            MetricKind::WriteBytes => "Write Bytes",
        }
    }
}

/// Reads an integer limit value from a configuration source.
///
/// This wraps the common pattern of reading a raw string from an environment
/// variable or file, parsing it as `u64`, and mapping the result into the
/// canonical error type — avoiding raw `unwrap()` / `expect()` calls.
///
/// # Arguments
///
/// * `raw` - The raw string value to parse (e.g. from an env var or `.env` file).
/// * `metric` - The metric kind, used in error messages.
/// * `source_label` - A human-readable label identifying the source of the
///   value (e.g. `"env VAR_NAME"` or `"file path:key"`).
///
/// # Errors
///
/// Returns `Error::Message` if the string cannot be parsed as a valid `u64`.
pub fn parse_limit_value(raw: &str, metric: MetricKind, source_label: &str) -> Result<u64> {
    raw.trim().parse::<u64>().map_err(|e| {
        Error::Message(format!(
            "cannot parse limit for {} from {}: '{}' is not a valid u64 ({})",
            metric.label(),
            source_label,
            raw.trim(),
            e
        ))
    })
}

/// Reads a `KEY=VALUE` entry from an `.env`-shaped file content.
///
/// # Arguments
///
/// * `content` - The full text content of the `.env` file.
/// * `key` - The key to look up.
///
/// # Returns
///
/// `Ok(Some(value))` if the key is found, `Ok(None)` if the key is absent,
/// or `Err` if the file content has malformed lines.
///
/// # Errors
///
/// Returns `Error::Message` if a line contains a malformed `KEY=VALUE` pair
/// (e.g. multiple `=` signs in a non-quoted context).
pub fn lookup_env_file_value(content: &str, key: &str) -> Result<Option<String>> {
    for (line_number, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let (lhs, rhs) = trimmed.split_once('=').ok_or_else(|| {
            Error::Message(format!(
                "line {}: malformed entry '{}' (expected KEY=VALUE)",
                line_number + 1,
                trimmed
            ))
        })?;
        if lhs.trim() == key {
            let raw_value = rhs.trim();
            let unquoted_value = raw_value
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .or_else(|| {
                    raw_value
                        .strip_prefix('\'')
                        .and_then(|s| s.strip_suffix('\''))
                })
                .unwrap_or(raw_value);
            return Ok(Some(unquoted_value.to_string()));
        }
    }
    Ok(None)
}

/// Reads a limit value from an `.env` file at the given path.
///
/// # Arguments
///
/// * `path` - Path to the `.env` file.
/// * `key` - The key to look up.
/// * `metric` - The metric kind, used in error messages.
///
/// # Errors
///
/// Returns `Error::Io` if the file cannot be read, or `Error::Message` if the
/// key is missing or the value cannot be parsed.
pub fn read_limit_from_env_file(path: &Path, key: &str, metric: MetricKind) -> Result<u64> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!("cannot read limit file '{}': {}", path.display(), e),
        ))
    })?;

    let raw_value = lookup_env_file_value(&content, key)?.ok_or_else(|| {
        Error::MissingField(format!(
            "key '{}' not found in limit file '{}'",
            key,
            path.display()
        ))
    })?;

    let source_label = format!("env_file {}:{}", path.display(), key);
    parse_limit_value(&raw_value, metric, &source_label)
}

/// Aggregates multiple limit sources into a single effective limit.
///
/// If `env_file_path` is provided, the limit is read from that file.
/// Otherwise, if `env_var` is provided, it is parsed directly.
/// If neither is provided, `Ok(default)` is returned.
///
/// This function avoids `unwrap()` / `expect()` throughout by chaining
/// the custom `Result` type through every fallible step.
///
/// # Arguments
///
/// * `env_file_path` - Optional path to an `.env` file.
/// * `env_var` - Optional raw string from an environment variable.
/// * `metric` - The metric kind, used in error messages.
/// * `default` - The default limit if no source is provided.
///
/// # Errors
///
/// Returns `Error::MissingField` if both sources are absent and no default
/// is relevant, or delegates to the underlying parse/read errors.
pub fn resolve_limit(
    env_file_path: Option<&Path>,
    env_var: Option<&str>,
    metric: MetricKind,
    default: u64,
) -> Result<u64> {
    match (env_file_path, env_var) {
        (Some(file_path), Some(var_raw)) => read_limit_from_env_file(file_path, var_raw, metric),
        (None, Some(var_raw)) => {
            let source_label = format!("env {}", var_raw);
            parse_limit_value(var_raw, metric, &source_label)
        }
        (Some(_), None) => Err(Error::MissingField(format!(
            "env_file path provided but no env var key for metric '{}'",
            metric.label(),
        ))),
        (None, None) => Ok(default),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_margin_multiplier tests ──────────────────────────────────

    #[test]
    fn parse_margin_multiplier_valid() {
        let result = parse_margin_multiplier("1.15");
        assert!(result.is_ok());
        assert!((result.unwrap() - 1.15).abs() < 1e-10);
    }

    #[test]
    fn parse_margin_multiplier_empty_string() {
        let result = parse_margin_multiplier("");
        assert!(result.is_err());
    }

    #[test]
    fn parse_margin_multiplier_whitespace_only() {
        let result = parse_margin_multiplier("  ");
        assert!(result.is_err());
    }

    #[test]
    fn parse_margin_multiplier_invalid_format() {
        let result = parse_margin_multiplier("not-a-number");
        assert!(result.is_err());
    }

    // ── validate_margin_multiplier tests ────────────────────────────────

    #[test]
    fn validate_margin_multiplier_positive() {
        let result = validate_margin_multiplier(1.0);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_margin_multiplier_zero() {
        let result = validate_margin_multiplier(0.0);
        assert!(result.is_err());
    }

    #[test]
    fn validate_margin_multiplier_negative() {
        let result = validate_margin_multiplier(-0.5);
        assert!(result.is_err());
    }

    #[test]
    fn validate_margin_multiplier_too_large() {
        let result = validate_margin_multiplier(150.0);
        assert!(result.is_err());
    }

    #[test]
    fn validate_margin_multiplier_at_maximum() {
        let result = validate_margin_multiplier(100.0);
        assert!(result.is_ok());
    }

    // ── MetricKind tests ───────────────────────────────────────────────

    #[test]
    fn metric_kind_labels() {
        assert_eq!(MetricKind::CpuInstructions.label(), "CPU Instructions");
        assert_eq!(MetricKind::ReadBytes.label(), "Read Bytes");
        assert_eq!(MetricKind::WriteBytes.label(), "Write Bytes");
    }

    // ── parse_limit_value tests ────────────────────────────────────────

    #[test]
    fn parse_limit_value_valid() {
        let result = parse_limit_value("5000000", MetricKind::CpuInstructions, "test");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 5_000_000);
    }

    #[test]
    fn parse_limit_value_zero() {
        let result = parse_limit_value("0", MetricKind::ReadBytes, "test");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn parse_limit_value_u64_max() {
        let result = parse_limit_value(&u64::MAX.to_string(), MetricKind::WriteBytes, "test");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), u64::MAX);
    }

    #[test]
    fn parse_limit_value_invalid() {
        let result = parse_limit_value("abc", MetricKind::CpuInstructions, "env MAX_CPU");
        assert!(result.is_err());
    }

    #[test]
    fn parse_limit_value_negative() {
        let result = parse_limit_value("-1", MetricKind::CpuInstructions, "test");
        assert!(result.is_err());
    }

    // ── lookup_env_file_value tests ────────────────────────────────────

    #[test]
    fn lookup_env_file_value_found() {
        let content = "KEY=123\nOTHER=456\n";
        let result = lookup_env_file_value(content, "KEY");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some("123".to_string()));
    }

    #[test]
    fn lookup_env_file_value_not_found() {
        let content = "KEY=123\n";
        let result = lookup_env_file_value(content, "MISSING");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn lookup_env_file_value_empty_content() {
        let result = lookup_env_file_value("", "KEY");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn lookup_env_file_value_comments_ignored() {
        let content = "# COMMENT=ignored\nKEY=value\n";
        let result = lookup_env_file_value(content, "COMMENT");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn lookup_env_file_value_quoted_value() {
        let content = "KEY=\"quoted value\"\n";
        let result = lookup_env_file_value(content, "KEY");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some("quoted value".to_string()));
    }

    #[test]
    fn lookup_env_file_value_malformed_line() {
        let content = "NO_EQUALS_SIGN\n";
        let result = lookup_env_file_value(content, "NO_EQUALS_SIGN");
        assert!(result.is_err());
    }

    // ── read_limit_from_env_file tests ─────────────────────────────────

    #[test]
    fn read_limit_from_env_file_valid() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let file_path = dir.path().join("limits.env");
        std::fs::write(&file_path, "CPU_LIMIT=5000000\n").expect("failed to write test file");

        let result = read_limit_from_env_file(&file_path, "CPU_LIMIT", MetricKind::CpuInstructions);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 5_000_000);
    }

    #[test]
    fn read_limit_from_env_file_missing_key() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let file_path = dir.path().join("limits.env");
        std::fs::write(&file_path, "OTHER=123\n").expect("failed to write test file");

        let result = read_limit_from_env_file(&file_path, "CPU_LIMIT", MetricKind::CpuInstructions);
        assert!(result.is_err());
    }

    #[test]
    fn read_limit_from_env_file_nonexistent_path() {
        let result = read_limit_from_env_file(
            Path::new("/tmp/nonexistent_file_xyz.env"),
            "KEY",
            MetricKind::CpuInstructions,
        );
        assert!(result.is_err());
    }

    // ── resolve_limit tests ────────────────────────────────────────────

    #[test]
    fn resolve_limit_with_env_file_and_key() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let file_path = dir.path().join("limits.env");
        std::fs::write(&file_path, "MY_KEY=2048\n").expect("failed to write test file");

        let result = resolve_limit(Some(&file_path), Some("MY_KEY"), MetricKind::ReadBytes, 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 2048);
    }

    #[test]
    fn resolve_limit_with_env_var_only() {
        let result = resolve_limit(None, Some("500000"), MetricKind::CpuInstructions, 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 500_000);
    }

    #[test]
    fn resolve_limit_with_default() {
        let result = resolve_limit(None, None, MetricKind::WriteBytes, 999);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 999);
    }

    #[test]
    fn resolve_limit_with_file_but_no_key() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let file_path = dir.path().join("limits.env");
        std::fs::write(&file_path, "A=1\n").expect("failed to write test file");

        let result = resolve_limit(Some(&file_path), None, MetricKind::CpuInstructions, 0);
        assert!(result.is_err());
    }

    #[test]
    fn resolve_limit_with_env_var_invalid() {
        let result = resolve_limit(None, Some("not-a-number"), MetricKind::CpuInstructions, 0);
        assert!(result.is_err());
    }
}
