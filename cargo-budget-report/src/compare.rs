//! Pure comparison logic for the `cargo budget-report` baseline/snapshot mode.
//!
//! This module is intentionally side-effect free (no I/O outside reading/writing
//! the baseline TOML file) and contains no CLI parsing. The same functions are
//! used by `--record-baseline`, `--check-baseline`, and the JSON report output.
//!
//! ## Baseline file format
//!
//! ```toml
//! [package.function]
//! cpu_instructions = 1234
//! read_bytes = 100
//! write_bytes = 200
//! ```
//!
//! Sections are sorted alphabetically by `BTreeMap` iteration; together with
//! the explicit metric ordering inside each section, this produces a stable
//! diff in PRs.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

/// One resource measurement for a single function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Measurement {
    pub cpu_instructions: u64,
    pub read_bytes: u64,
    pub write_bytes: u64,
}

impl Measurement {
    /// Placeholder used in tests; not part of the public API.
    #[allow(dead_code)]
    pub const fn zero() -> Self {
        Self {
            cpu_instructions: 0,
            read_bytes: 0,
            write_bytes: 0,
        }
    }
}

/// Default tolerance applied to a metric when no override is supplied.
///
/// 10 % matches the project's stated intuition: testnet simulations drift
/// with ledger state, so a small headroom absorbs that noise without masking
/// real regressions.
pub const DEFAULT_TOLERANCE: f64 = 0.10;

/// Keys in the baseline file use `<package>::<function>` because `::`
/// cannot appear in a Cargo package or export name.
pub const KEY_SEPARATOR: &str = "::";

pub fn function_key(package: &str, function: &str) -> String {
    format!("{package}{KEY_SEPARATOR}{function}")
}

/// Configurable tolerance, supplied via CLI (`--tolerance`) or `budget.toml`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tolerance {
    /// Fractional tolerance. `0.10` allows a 10 % increase before failing.
    pub value: f64,
}

impl Tolerance {
    pub const fn new(value: f64) -> Self {
        Self { value }
    }

    /// Returns `true` when `current` is allowed given `baseline` and tolerance.
    ///
    /// Boundary policy: `current == max_allowed` passes; `current > max_allowed`
    /// fails. The check is `current <= baseline * (1 + tol)` in integer space
    /// to avoid floating-point off-by-one near the boundary.
    pub fn allows(self, baseline: u64, current: u64) -> bool {
        max_allowed(baseline, self.value) >= current
    }
}

impl Default for Tolerance {
    fn default() -> Self {
        Self {
            value: DEFAULT_TOLERANCE,
        }
    }
}

/// Parse a tolerance value as either a fraction (`"0.05"`) or a percentage
/// (`"5%"` — equivalent to `"0.05"`). Rejects negative values; zero is
/// allowed.
pub fn parse_tolerance(s: &str) -> Result<Tolerance> {
    let trimmed = s.trim();
    let (raw, is_percent) = match trimmed.strip_suffix('%') {
        Some(stripped) => (stripped.trim(), true),
        None => (trimmed, false),
    };
    let parsed: f64 = raw
        .parse()
        .with_context(|| format!("tolerance must be a number (e.g. '5%' or '0.05'), got '{s}'"))?;
    let value = if is_percent { parsed / 100.0 } else { parsed };
    if !value.is_finite() {
        anyhow::bail!("tolerance must be a finite number, got '{s}'");
    }
    if value < 0.0 {
        anyhow::bail!("tolerance must be non-negative, got '{value}'");
    }
    Ok(Tolerance::new(value))
}

/// Compute the largest allowed value for a given baseline + tolerance.
/// Returns `u64::MAX` if the multiplication would overflow (effectively an
/// unlimited ceiling) — this is also what `Tolerance::allows` checks against.
pub fn max_allowed(baseline: u64, tolerance: f64) -> u64 {
    let rhs = 1.0_f64 + tolerance;
    if rhs <= 0.0 {
        return baseline.saturating_sub(baseline); // == 0
    }
    let scaled = (baseline as f64) * rhs;
    if scaled >= u64::MAX as f64 {
        return u64::MAX;
    }
    if scaled < 0.0 {
        return 0;
    }
    scaled as u64
}

/// On-disk baseline, keyed by `<package>::<function>`.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Baseline {
    /// Section headers to write, sorted alphabetically (`BTreeMap`).
    pub entries: BTreeMap<String, BaselineEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaselineEntry {
    pub cpu_instructions: u64,
    pub read_bytes: u64,
    pub write_bytes: u64,
}

impl BaselineEntry {
    pub fn from_measurement(m: Measurement) -> Self {
        Self {
            cpu_instructions: m.cpu_instructions,
            read_bytes: m.read_bytes,
            write_bytes: m.write_bytes,
        }
    }
}

impl Baseline {
    /// Read a baseline file from disk. A missing file is treated as an error
    /// so that callers (notably `cargo budget-report --check-baseline`) can
    /// surface a clear "run --record-baseline first" message instead of
    /// silently comparing against an empty baseline.
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(contents) => Self::parse(&contents)
                .with_context(|| format!("failed to parse baseline {}", path.display())),
            Err(err) => {
                Err(err).with_context(|| format!("failed to read baseline {}", path.display()))
            }
        }
    }

    /// Parse TOML content into a `Baseline`. Splitting it from `load` keeps
    /// the parser testable without filesystem setup.
    pub fn parse(content: &str) -> Result<Self> {
        let value: toml::Value =
            toml::from_str(content).context("baseline TOML could not be parsed")?;
        let root_table = value
            .as_table()
            .context("baseline top-level must be a TOML table")?;

        let mut entries: BTreeMap<String, BaselineEntry> = BTreeMap::new();
        for (key, val) in root_table {
            if !val.is_table() {
                anyhow::bail!("expected a table for key '{key}', got {}", value_kind(val));
            }
            let inner = val.as_table().expect("checked above").clone();
            let entry = BaselineEntry::parse(&inner)
                .with_context(|| format!("failed to parse entry '{key}'"))?;
            entries.insert(key.clone(), entry);
        }
        Ok(Self { entries })
    }

    /// Serialize the baseline to a deterministic TOML string. Iterating over
    /// `BTreeMap` provides alphabetical key ordering; metric lines inside a
    /// block also have a fixed order so PR diffs stay minimal.
    pub fn to_toml(&self) -> Result<String> {
        let mut out = String::new();
        out.push_str(
            "# budget-baseline.toml\n\
             # Recorded by `cargo budget-report --record-baseline`. Each section\n\
             # `[package.function]` holds the measured CPU instructions, read\n\
             # bytes, and write bytes from `simulateTransaction`. Compare against\n\
             # this file with `cargo budget-report --check-baseline`. Tolerance\n\
             # for regressions is configured in `budget.toml` (top-level plus\n\
             # per-function overrides) and defaults to 10%.\n",
        );

        for (key, entry) in &self.entries {
            writeln!(out)?;
            // Keys contain `<package>::<function>`, and bare TOML table
            // headers only accept alphanumeric, `-`, and `_`. Quoting the
            // header eschews the bare-form restrictions and keeps the `::`
            // verbatim.
            writeln!(out, "[\"{key}\"]")?;
            writeln!(out, "cpu_instructions = {}", entry.cpu_instructions)?;
            writeln!(out, "read_bytes = {}", entry.read_bytes)?;
            writeln!(out, "write_bytes = {}", entry.write_bytes)?;
        }
        if self.entries.is_empty() {
            writeln!(out)?;
            writeln!(out, "# (no entries)")?;
        }
        Ok(out)
    }

    /// Atomically write the baseline to disk. Writes to `<path>.tmp` first and
    /// renames, so a partial write never replaces a valid baseline.
    pub fn save(&self, path: &Path) -> Result<()> {
        let contents = self.to_toml()?;
        let tmp = sibling_tmp_path(path);
        std::fs::write(&tmp, contents).with_context(|| {
            format!("failed to write baseline temporary file {}", tmp.display())
        })?;
        std::fs::rename(&tmp, path)
            .with_context(|| format!("failed to rename {} to {}", tmp.display(), path.display()))?;
        Ok(())
    }
}

impl BaselineEntry {
    fn parse(table: &toml::value::Table) -> Result<Self> {
        let get_unsigned = |key: &str| -> Result<u64> {
            let value = table
                .get(key)
                .with_context(|| format!("missing field '{key}'"))?;
            match value {
                toml::Value::Integer(n) if *n >= 0 => Ok(*n as u64),
                toml::Value::Integer(_) => {
                    anyhow::bail!("field '{key}' must be non-negative")
                }
                other => anyhow::bail!(
                    "field '{key}' must be a non-negative integer, got {}",
                    value_kind(other)
                ),
            }
        };
        Ok(Self {
            cpu_instructions: get_unsigned("cpu_instructions")?,
            read_bytes: get_unsigned("read_bytes")?,
            write_bytes: get_unsigned("write_bytes")?,
        })
    }
}

fn value_kind(v: &toml::Value) -> &'static str {
    match v {
        toml::Value::String(_) => "a string",
        toml::Value::Integer(_) => "an integer",
        toml::Value::Float(_) => "a float",
        toml::Value::Boolean(_) => "a boolean",
        toml::Value::Datetime(_) => "a datetime",
        toml::Value::Array(_) => "an array",
        toml::Value::Table(_) => "a table",
    }
}

/// Build a sibling path with `.tmp` appended to the file name. `with_extension`
/// would replace the existing extension (turning `budget-baseline.toml` into
/// `budget-baseline.tmp`), so we have to add the suffix by hand.
fn sibling_tmp_path(path: &Path) -> std::path::PathBuf {
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("budget-baseline.toml");
    path.with_file_name(format!("{file_name}.tmp"))
}

// -----------------------------------------------------------------------------
// Comparison report
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricKind {
    CpuInstructions,
    ReadBytes,
    WriteBytes,
}

impl MetricKind {
    const ALL: &'static [MetricKind] = &[
        MetricKind::CpuInstructions,
        MetricKind::ReadBytes,
        MetricKind::WriteBytes,
    ];

    pub fn label(self) -> &'static str {
        match self {
            MetricKind::CpuInstructions => "cpu_instructions",
            MetricKind::ReadBytes => "read_bytes",
            MetricKind::WriteBytes => "write_bytes",
        }
    }

    fn baseline_value(self, entry: BaselineEntry) -> u64 {
        match self {
            MetricKind::CpuInstructions => entry.cpu_instructions,
            MetricKind::ReadBytes => entry.read_bytes,
            MetricKind::WriteBytes => entry.write_bytes,
        }
    }

    fn current_value(self, m: Measurement) -> u64 {
        match self {
            MetricKind::CpuInstructions => m.cpu_instructions,
            MetricKind::ReadBytes => m.read_bytes,
            MetricKind::WriteBytes => m.write_bytes,
        }
    }
}

/// One measured metric's verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Within tolerance.
    Pass,
    /// `current` strictly below baseline — informational improvement.
    Improvement,
    /// `current` strictly above `baseline * (1 + tolerance)`.
    Regression,
}

/// Result for a single `(metric, current)` comparison.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetricComparison {
    pub metric: MetricKind,
    pub baseline: u64,
    pub current: u64,
    pub tolerance: Tolerance,
    pub verdict: Verdict,
}

impl MetricComparison {
    pub fn limits_label(&self) -> String {
        let max = max_allowed(self.baseline, self.tolerance.value);
        format!(
            "baseline={}, max_allowed={} (baseline * {:.4})",
            self.baseline, max, self.tolerance.value
        )
    }
}

/// One function's full set of metric findings.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionComparison {
    pub package: String,
    pub function: String,
    pub metrics: Vec<MetricComparison>,
}

impl FunctionComparison {
    pub fn has_regressions(&self) -> bool {
        self.metrics
            .iter()
            .any(|m| matches!(m.verdict, Verdict::Regression))
    }
}

/// A function in the baseline that was not measured in the current run —
/// it may have been renamed or removed from the WASM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaleEntry {
    pub package: String,
    pub function: String,
}

/// A function measured in the current run but not present in the baseline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEntry {
    pub package: String,
    pub function: String,
}

/// Full output of `check_against_baseline`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CheckReport {
    pub compared: Vec<FunctionComparison>,
    pub stale: Vec<StaleEntry>,
    pub new: Vec<NewEntry>,
}

impl CheckReport {
    /// True if any metric regressed beyond its tolerance.
    pub fn has_regressions(&self) -> bool {
        self.compared
            .iter()
            .any(FunctionComparison::has_regressions)
    }

    /// Number of metric-level regressions (can exceed `compared.len()` when a
    /// single function regresses on multiple metrics).
    pub fn regression_count(&self) -> usize {
        self.compared
            .iter()
            .flat_map(|f| f.metrics.iter())
            .filter(|m| matches!(m.verdict, Verdict::Regression))
            .count()
    }
}

/// Compare the current measurements against a baseline using the supplied
/// per-function tolerance overrides.
///
/// `tolerance_overrides` is keyed by `function` only (same tolerance applies
/// regardless of which package the function lives in). Entries in the
/// baseline with no current counterpart go into `CheckReport::stale`;
/// entries in `current` with no baseline counterpart go into `new`.
pub fn check_against_baseline(
    baseline: &Baseline,
    current: &BTreeMap<String, BTreeMap<String, Measurement>>,
    default_tolerance: Tolerance,
    tolerance_overrides: &BTreeMap<String, Tolerance>,
) -> CheckReport {
    let mut report = CheckReport::default();

    for (key, entry) in &baseline.entries {
        let (package, function) = match split_key(key) {
            Some(pair) => pair,
            None => continue, // malformed, ignored
        };
        let current_pkg = match current.get(&package) {
            Some(map) => map,
            None => {
                report.stale.push(StaleEntry {
                    package: package.clone(),
                    function: function.clone(),
                });
                continue;
            }
        };
        let measured = match current_pkg.get(&function) {
            Some(m) => *m,
            None => {
                report.stale.push(StaleEntry {
                    package: package.clone(),
                    function: function.clone(),
                });
                continue;
            }
        };

        let tolerance = tolerance_overrides
            .get(&function)
            .copied()
            .unwrap_or(default_tolerance);

        let metrics = MetricKind::ALL
            .iter()
            .map(|kind| {
                let baseline_value = kind.baseline_value(*entry);
                let current_value = kind.current_value(measured);
                let verdict = classify(baseline_value, current_value, tolerance);
                MetricComparison {
                    metric: *kind,
                    baseline: baseline_value,
                    current: current_value,
                    tolerance,
                    verdict,
                }
            })
            .collect();

        report.compared.push(FunctionComparison {
            package,
            function,
            metrics,
        });
    }

    for (package, fns) in current {
        for function in fns.keys() {
            let key = function_key(package, function);
            if !baseline.entries.contains_key(&key) {
                report.new.push(NewEntry {
                    package: package.clone(),
                    function: function.clone(),
                });
            }
        }
    }

    report
}

/// Classify a single `(baseline, current, tolerance)` triple.
///
/// Boundary policy: `current == max_allowed` passes; only strictly greater
/// triggers a regression. `current < baseline` is reported as Improvement.
fn classify(baseline: u64, current: u64, tolerance: Tolerance) -> Verdict {
    if current <= baseline {
        return if current == baseline {
            Verdict::Pass
        } else {
            Verdict::Improvement
        };
    }
    // current > baseline here; max_allowed == current => Pass, else Regression.
    if tolerance.allows(baseline, current) {
        Verdict::Pass
    } else {
        Verdict::Regression
    }
}

fn split_key(key: &str) -> Option<(String, String)> {
    let (pkg, fn_) = key.split_once(KEY_SEPARATOR)?;
    Some((pkg.to_string(), fn_.to_string()))
}

/// Format a `CheckReport` for human display (terminal table-style output).
pub fn render_report_text(report: &CheckReport) -> String {
    let mut out = String::new();
    out.push_str("\n=== BASELINE CHECK REPORT ===\n");

    if report.compared.is_empty() && report.new.is_empty() && report.stale.is_empty() {
        out.push_str("\nNo overlap between baseline and current measurements.\n");
        return out;
    }

    if !report.compared.is_empty() {
        out.push_str("\nComparisons:\n");
        out.push_str(&render_comparison_table(&report.compared));
    }

    if !report.new.is_empty() {
        out.push_str("\nNew functions (no baseline entry):\n");
        for entry in &report.new {
            out.push_str(&format!("  + {}::{}\n", entry.package, entry.function));
        }
        out.push_str("  Suggestion: re-run with `--record-baseline` to capture them.\n");
    }

    if !report.stale.is_empty() {
        out.push_str("\nStale baseline entries (function not in current WASM):\n");
        for entry in &report.stale {
            out.push_str(&format!("  - {}::{}\n", entry.package, entry.function));
        }
        out.push_str("  Suggestion: re-run with `--record-baseline` to clean them up.\n");
    }

    out.push_str("\nSummary:\n");
    let regressions = report.regression_count();
    let improvements = report
        .compared
        .iter()
        .flat_map(|f| f.metrics.iter())
        .filter(|m| matches!(m.verdict, Verdict::Improvement))
        .count();
    out.push_str(&format!("  regressions: {regressions}\n"));
    out.push_str(&format!("  improvements: {improvements}\n"));
    out.push_str(&format!("  new functions: {}\n", report.new.len()));
    out.push_str(&format!("  stale entries: {}\n", report.stale.len()));
    out
}

fn render_comparison_table(rows: &[FunctionComparison]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "  {:<24} {:<14} {:>14} {:>14} {:>12} verdict\n",
        "function", "metric", "baseline", "current", "tolerance"
    ));
    for row in rows {
        for m in &row.metrics {
            out.push_str(&format!(
                "  {:<24} {:<14} {:>14} {:>14} {:>11.2}% {}\n",
                format!("{}::{}", row.package, row.function),
                m.metric.label(),
                m.baseline,
                m.current,
                m.tolerance.value * 100.0,
                verdict_label(m.verdict),
            ));
        }
        if row.has_regressions() {
            for m in row
                .metrics
                .iter()
                .filter(|m| matches!(m.verdict, Verdict::Regression))
            {
                out.push_str(&format!(
                    "    ! regression: {} = {} ({})\n",
                    m.metric.label(),
                    m.current,
                    m.limits_label()
                ));
            }
        }
    }
    out
}

fn verdict_label(v: Verdict) -> &'static str {
    match v {
        Verdict::Pass => "ok",
        Verdict::Improvement => "improved",
        Verdict::Regression => "REGRESSED",
    }
}

/// Build a `Baseline` from the current run's measurements, ready to write to
/// disk with `Baseline::save`. Stale entries in the existing baseline are
/// dropped; new ones replace older values for the same `(package, function)`.
///
/// Splitting this from `save` keeps the merge logic unit-testable.
pub fn build_baseline(current: &BTreeMap<String, BTreeMap<String, Measurement>>) -> Baseline {
    let mut entries = BTreeMap::new();
    for (package, fns) in current {
        for (function, m) in fns {
            let key = function_key(package, function);
            entries.insert(key, BaselineEntry::from_measurement(*m));
        }
    }
    Baseline { entries }
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn m(cpu: u64, read: u64, write: u64) -> Measurement {
        Measurement {
            cpu_instructions: cpu,
            read_bytes: read,
            write_bytes: write,
        }
    }

    fn current_map() -> BTreeMap<String, BTreeMap<String, Measurement>> {
        let mut pkg = BTreeMap::new();
        pkg.insert("do_expensive_work".to_string(), m(1100, 200, 300));
        pkg.insert("transfer".to_string(), m(900, 100, 200));
        let mut root = BTreeMap::new();
        root.insert("amm-pool-contract".to_string(), pkg);

        let mut other = BTreeMap::new();
        other.insert("ping".to_string(), m(50, 0, 0));
        root.insert("other-crate".to_string(), other);
        root
    }

    fn committed_baseline() -> Baseline {
        let mut entries = BTreeMap::new();
        entries.insert(
            function_key("amm-pool-contract", "do_expensive_work"),
            BaselineEntry::from_measurement(m(1000, 200, 300)),
        );
        entries.insert(
            function_key("amm-pool-contract", "removed_fn"),
            BaselineEntry::from_measurement(m(80, 0, 0)),
        );
        Baseline { entries }
    }

    // -- parse_tolerance / Tolerance ------------------------------------------

    #[test]
    fn tolerance_parses_fraction() {
        assert_eq!(parse_tolerance("0.05").unwrap().value, 0.05);
        assert_eq!(parse_tolerance("0.0").unwrap().value, 0.0);
        assert_eq!(parse_tolerance("0.10").unwrap().value, 0.10);
    }

    #[test]
    fn tolerance_parses_percentage() {
        assert_eq!(parse_tolerance("5%").unwrap().value, 0.05);
        assert_eq!(parse_tolerance("10 %").unwrap().value, 0.10);
    }

    #[test]
    fn tolerance_rejects_negative() {
        let err = parse_tolerance("-0.05").unwrap_err().to_string();
        assert!(err.contains("non-negative"), "got: {err}");
    }

    #[test]
    fn tolerance_rejects_garbage() {
        let err = parse_tolerance("hello").unwrap_err().to_string();
        assert!(err.contains("tolerance must be a number"), "got: {err}");
    }

    #[test]
    fn tolerance_rejects_nan() {
        let err = parse_tolerance("NaN").unwrap_err().to_string();
        assert!(err.contains("finite"), "got: {err}");
    }

    #[test]
    fn tolerance_default_is_ten_percent() {
        let t = Tolerance::default();
        assert!((t.value - 0.10).abs() < f64::EPSILON);
    }

    // -- Tolerance::allows / classify boundary math --------------------------

    #[test]
    fn tolerance_allows_value_at_exact_boundary() {
        // baseline = 1000, tol = 0.10 -> max_allowed = 1100
        let t = Tolerance::new(0.10);
        assert!(t.allows(1000, 1099));
        assert!(t.allows(1000, 1100), "== max_allowed must pass");
        assert!(!t.allows(1000, 1101), "one above must fail");
    }

    #[test]
    fn zero_tolerance_allows_only_exact_match() {
        let t = Tolerance::new(0.0);
        assert!(t.allows(1000, 1000));
        assert!(!t.allows(1000, 1001));
        // `current < baseline` is an improvement, not a regression; `allows`
        // (and the `classify` flow) report it as accepted. The math boundary
        // is `current > max_allowed`, not `current != baseline`.
        assert!(t.allows(1000, 999));
    }

    #[test]
    fn classify_improvement_when_below_baseline() {
        let v = classify(1000, 900, Tolerance::new(0.10));
        assert!(matches!(v, Verdict::Improvement));
    }

    #[test]
    fn classify_pass_when_within_tolerance() {
        let v = classify(1000, 1100, Tolerance::new(0.10));
        assert!(matches!(v, Verdict::Pass));
    }

    #[test]
    fn classify_regression_when_above_tolerance() {
        let v = classify(1000, 1101, Tolerance::new(0.10));
        assert!(matches!(v, Verdict::Regression));
    }

    #[test]
    fn classify_pass_when_exactly_equal() {
        let v = classify(1000, 1000, Tolerance::new(0.10));
        assert!(matches!(v, Verdict::Pass));
    }

    // -- check_against_baseline ----------------------------------------------

    #[test]
    fn check_no_regressions_no_stale_no_new() {
        let baseline = Baseline {
            entries: BTreeMap::from([(
                function_key("amm-pool-contract", "do_expensive_work"),
                BaselineEntry::from_measurement(m(1000, 200, 300)),
            )]),
        };
        let mut pkg = BTreeMap::new();
        pkg.insert("do_expensive_work".to_string(), m(1100, 200, 300));
        let mut current = BTreeMap::new();
        current.insert("amm-pool-contract".to_string(), pkg);

        let report =
            check_against_baseline(&baseline, &current, Tolerance::new(0.10), &BTreeMap::new());
        assert_eq!(report.regression_count(), 0);
        assert!(report.stale.is_empty());
        assert!(report.new.is_empty());
    }

    #[test]
    fn check_clear_dropkick_regression() {
        // Anchors the boundary contract the opposite way: a value so far above
        // `max_allowed` that there is no room for floating-point argument.
        let baseline = Baseline {
            entries: BTreeMap::from([(
                function_key("amm-pool-contract", "do_expensive_work"),
                BaselineEntry::from_measurement(m(1000, 200, 300)),
            )]),
        };
        let mut pkg = BTreeMap::new();
        pkg.insert("do_expensive_work".to_string(), m(2000, 1000, 1000));
        let mut current = BTreeMap::new();
        current.insert("amm-pool-contract".to_string(), pkg);
        let report =
            check_against_baseline(&baseline, &current, Tolerance::new(0.10), &BTreeMap::new());
        assert!(report.has_regressions());
        assert_eq!(report.regression_count(), 3);
    }

    #[test]
    fn baseline_load_missing_file_returns_error() {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "cargo_budget_report_compare_missing_{}.toml",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_file(&path);
        let err = Baseline::load(&path).unwrap_err().to_string();
        assert!(err.contains("failed to read baseline"), "got: {err}");
    }

    #[test]
    fn check_passes_at_exact_tolerance_boundary() {
        let baseline = committed_baseline();
        let current = current_map();
        let report =
            check_against_baseline(&baseline, &current, Tolerance::new(0.10), &BTreeMap::new());

        // do_expensive_work cpu jumped 1000 -> 1100; baseline * 1.10 == 1100,
        // so 1100 must be Pass, not Regression.
        let cpu = find_metric(
            &report,
            "amm-pool-contract",
            "do_expensive_work",
            MetricKind::CpuInstructions,
        );
        assert_eq!(
            cpu.baseline, 1000,
            "test setup: baseline value must be 1000"
        );
        assert_eq!(cpu.current, 1100, "test setup: current value must be 1100");
        assert!(matches!(cpu.verdict, Verdict::Pass));
    }

    #[test]
    fn check_reports_regression_just_above_tolerance() {
        let baseline = committed_baseline();
        // Override the cpu value to 1101 (1 above max_allowed) without touching
        // read_bytes/write_bytes, so we only assert the cpu metric regresses.
        let mut pkg = BTreeMap::new();
        pkg.insert("do_expensive_work".to_string(), m(1101, 200, 300));
        let mut current = BTreeMap::new();
        current.insert("amm-pool-contract".to_string(), pkg);

        let report =
            check_against_baseline(&baseline, &current, Tolerance::new(0.10), &BTreeMap::new());
        let cpu = find_metric(
            &report,
            "amm-pool-contract",
            "do_expensive_work",
            MetricKind::CpuInstructions,
        );
        assert!(matches!(cpu.verdict, Verdict::Regression));
    }

    #[test]
    fn check_reports_stale_entries() {
        let baseline = committed_baseline();
        let current = current_map();
        let report =
            check_against_baseline(&baseline, &current, Tolerance::new(0.10), &BTreeMap::new());
        assert!(
            report
                .stale
                .iter()
                .any(|s| s.package == "amm-pool-contract" && s.function == "removed_fn"),
            "expected stale entry for removed_fn"
        );
    }

    #[test]
    fn check_reports_new_entries() {
        let baseline = committed_baseline();
        let current = current_map();
        let report =
            check_against_baseline(&baseline, &current, Tolerance::new(0.10), &BTreeMap::new());
        assert!(
            report
                .new
                .iter()
                .any(|n| n.package == "other-crate" && n.function == "ping"),
            "expected new entry for ping"
        );
    }

    #[test]
    fn check_applies_per_function_tolerance_override() {
        let baseline = committed_baseline();
        let current = current_map();
        // Tight override on do_expensive_work at 0.05 makes 1100 regress.
        let mut overrides = BTreeMap::new();
        overrides.insert("do_expensive_work".to_string(), Tolerance::new(0.05));
        let report = check_against_baseline(&baseline, &current, Tolerance::new(0.10), &overrides);
        let cpu = find_metric(
            &report,
            "amm-pool-contract",
            "do_expensive_work",
            MetricKind::CpuInstructions,
        );
        assert!(matches!(cpu.verdict, Verdict::Regression));
    }

    #[test]
    fn check_per_function_override_can_relax_global_tolerance() {
        let baseline = committed_baseline();
        let current = current_map();
        let mut overrides = BTreeMap::new();
        overrides.insert("do_expensive_work".to_string(), Tolerance::new(0.50));
        let report = check_against_baseline(&baseline, &current, Tolerance::new(0.0), &overrides);
        let cpu = find_metric(
            &report,
            "amm-pool-contract",
            "do_expensive_work",
            MetricKind::CpuInstructions,
        );
        // Global is 0% which would regress on 1100 vs 1000; override 50% saves it.
        assert!(matches!(cpu.verdict, Verdict::Pass));
    }

    #[test]
    fn check_improvement_does_not_count_as_regression() {
        let baseline = Baseline {
            entries: BTreeMap::from([(
                function_key("amm-pool-contract", "do_expensive_work"),
                BaselineEntry::from_measurement(m(1000, 200, 300)),
            )]),
        };
        let mut pkg = BTreeMap::new();
        pkg.insert("do_expensive_work".to_string(), m(700, 100, 50));
        let mut current = BTreeMap::new();
        current.insert("amm-pool-contract".to_string(), pkg);
        let report =
            check_against_baseline(&baseline, &current, Tolerance::new(0.10), &BTreeMap::new());
        assert!(!report.has_regressions());
        assert_eq!(report.regression_count(), 0);
        let improvements: Vec<_> = report
            .compared
            .iter()
            .flat_map(|f| f.metrics.iter())
            .filter(|m| matches!(m.verdict, Verdict::Improvement))
            .collect();
        assert_eq!(improvements.len(), 3);
    }

    #[test]
    fn check_stale_and_new_entries_never_fail_run() {
        let baseline = committed_baseline();
        let current = current_map();
        let report =
            check_against_baseline(&baseline, &current, Tolerance::new(0.10), &BTreeMap::new());
        // Stale and new entries are informational; they must not flip
        // has_regressions() on their own.
        assert!(
            !report.has_regressions(),
            "stale/new entries caused false regression"
        );
    }

    #[test]
    fn check_regression_count_sums_metric_level_findings() {
        let baseline = Baseline {
            entries: BTreeMap::from([(
                function_key("amm-pool-contract", "do_expensive_work"),
                BaselineEntry::from_measurement(m(1000, 200, 300)),
            )]),
        };
        let mut pkg = BTreeMap::new();
        // All three metrics regress (current > max_allowed for each).
        pkg.insert("do_expensive_work".to_string(), m(2000, 1000, 1000));
        let mut current = BTreeMap::new();
        current.insert("amm-pool-contract".to_string(), pkg);
        let report =
            check_against_baseline(&baseline, &current, Tolerance::new(0.10), &BTreeMap::new());
        assert!(report.has_regressions());
        assert_eq!(report.regression_count(), 3);
    }

    // -- build_baseline / save / parse / to_toml round-trip -----------------

    #[test]
    fn build_baseline_replaces_entries_for_known_functions() {
        let current = current_map();
        // Simplest merge: build_baseline is "current wins, no merging with old".
        let new = build_baseline(&current);
        assert_eq!(
            new.entries
                .get(&function_key("amm-pool-contract", "do_expensive_work"))
                .map(|e| e.cpu_instructions),
            Some(1100)
        );
        // removed_fn is dropped (not in current).
        assert!(!new
            .entries
            .contains_key(&function_key("amm-pool-contract", "removed_fn")));
    }

    #[test]
    fn baseline_round_trip_preserves_all_values() {
        let entries = BTreeMap::from([
            (
                function_key("amm-pool-contract", "do_expensive_work"),
                BaselineEntry::from_measurement(m(123456, 789, 1023)),
            ),
            (
                function_key("amm-pool-contract", "transfer"),
                BaselineEntry::from_measurement(m(1000, 200, 300)),
            ),
            (
                function_key("other-crate", "ping"),
                BaselineEntry::from_measurement(m(7, 0, 0)),
            ),
        ]);
        let baseline = Baseline { entries };
        let serialized = baseline.to_toml().unwrap();
        let parsed = Baseline::parse(&serialized).unwrap();
        assert_eq!(parsed, baseline);

        // Writing -> --record-baseline should be a no-op for unchanged data:
        let again = parsed.to_toml().unwrap();
        assert_eq!(serialized, again, "round-trip must be deterministic");
    }

    #[test]
    fn baseline_toml_emits_sorted_sections() {
        let entries = BTreeMap::from([
            (
                function_key("zeta", "fn"),
                BaselineEntry::from_measurement(m(1, 1, 1)),
            ),
            (
                function_key("alpha", "gn"),
                BaselineEntry::from_measurement(m(2, 2, 2)),
            ),
            (
                function_key("alpha", "fn"),
                BaselineEntry::from_measurement(m(3, 3, 3)),
            ),
        ]);
        let baseline = Baseline { entries };
        let toml = baseline.to_toml().unwrap();
        // BTreeMap iterates alphabetically; lowercase `f` precedes `g` after
        // the shared `alpha::` prefix, so `alpha::fn` comes before
        // `alpha::gn`. Headers are emitted as `["<key>"]` to keep `::` valid
        // in TOML 1.0.
        let alpha_fn = toml.find("[\"alpha::fn\"]").expect("alpha::fn present");
        let alpha_gn = toml.find("[\"alpha::gn\"]").expect("alpha::gn present");
        let zeta = toml.find("[\"zeta::fn\"]").expect("zeta::fn present");
        assert!(alpha_fn < alpha_gn, "alpha::fn must come before alpha::gn");
        assert!(alpha_gn < zeta, "alpha::gn must come before zeta::fn");
    }

    #[test]
    fn baseline_parse_rejects_missing_field() {
        let toml = "[\"a::b\"]\ncpu_instructions = 1\nread_bytes = 1\n";
        let err = format!("{:#}", Baseline::parse(toml).unwrap_err());
        assert!(
            err.contains("write_bytes"),
            "expected missing-field error, got: {err}"
        );
    }

    #[test]
    fn baseline_parse_rejects_non_table_top_level_key() {
        let toml = "\"a::b\" = 42\n";
        let err = Baseline::parse(toml).unwrap_err().to_string();
        assert!(
            err.contains("expected a table"),
            "expected table-required error, got: {err}"
        );
    }

    #[test]
    fn baseline_parse_rejects_negative_metric() {
        let toml = "[\"a::b\"]\ncpu_instructions = -1\nread_bytes = 1\nwrite_bytes = 1\n";
        let err = format!("{:#}", Baseline::parse(toml).unwrap_err());
        assert!(err.contains("non-negative"), "got: {err}");
    }

    #[test]
    fn baseline_save_writes_atomically() {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "cargo_budget_report_compare_{}.toml",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let baseline = Baseline {
            entries: BTreeMap::from([(
                function_key("p", "f"),
                BaselineEntry::from_measurement(m(100, 200, 300)),
            )]),
        };
        baseline.save(&path).expect("save should succeed");
        let on_disk = std::fs::read_to_string(&path).expect("file exists");
        assert!(on_disk.contains("cpu_instructions = 100"));
        // The `.tmp` sibling is renamed away; it must not exist after save.
        let tmp = sibling_tmp_path(&path);
        assert!(
            !tmp.exists(),
            "tmp sibling {} should not survive",
            tmp.display()
        );
        let _ = std::fs::remove_file(&path);
    }

    // -- helpers -------------------------------------------------------------

    fn find_metric<'r>(
        report: &'r CheckReport,
        pkg: &str,
        function: &str,
        metric: MetricKind,
    ) -> &'r MetricComparison {
        report
            .compared
            .iter()
            .find(|f| f.package == pkg && f.function == function)
            .expect("function present in report")
            .metrics
            .iter()
            .find(|m| m.metric == metric)
            .expect("metric present in function")
    }
}
