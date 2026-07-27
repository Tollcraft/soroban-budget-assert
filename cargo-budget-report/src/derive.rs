//! Tier A limit derivation from a Tier B JSON report.
//!
//! This module is the engine behind `cargo budget-report --derive-limits`.
//! Given a network-verified `simulateTransaction` report — the same JSON
//! shape produced by `cargo budget-report --json` — and a per-metric
//! margin, it emits the `KEY=VALUE` lines that a `#[budget_cpu_lt(...)]`
//! test annotation can consume via the macro's
//! `env_file = "PATH"` + `env = "VAR"` form.
//!
//! ## Design rationale
//!
//! A single global margin is known to be wrong across operation types
//! (see issue #45): a host-function call and a tight VM loop have different
//! local-vs-network deltas. This module therefore treats the margin as a
//! four-component input (`cpu`, `memory`, `read`, `write`) and never picks
//! one for the caller: if any component is missing, the derivation errors
//! out with the missing field named. The Margin type also powers the
//! `budget.toml` `[margin]` section so contributors can persist a margin
//! alongside their per-function configuration.
//!
//! ## Output format
//!
//! The emitted env file is a deterministic, alphabetically keyed
//! `KEY=VALUE` list preceded by a provenance header that records:
//!
//! - the source Tier B JSON path or `<stdin>`,
//! - the four margin multipliers used,
//! - the derivation timestamp (UTC ISO-8601),
//! - the wasm32 build profile that produced the source where known, and
//! - a per-line ledger summarising `tier_b_value × margin = tier_a_limit`.
//!
//! The provenance block is the audit trail a reviewer needs to see which
//! figure produced which limit. The `.env` format is chosen over a Rust
//! `pub const` module because:
//!
//! - the macro's existing `env_file` form reads `.env` directly, so no
//!   additional plumbing is required;
//! - it diffs cleanly in PRs (one line per value);
//! - non-Rust tooling (CI scripts, dashboards, the cost-over-time
//!   consumer) can read the same file.

use crate::module_10::{Error, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

/// Per-metric margin multipliers applied to Tier B values.
///
/// Every margin is a positive `f64`. `cpu = 1.25` means
/// `tier_a_limit = ceil(tier_b_cpu * 1.25)`. A margin that is too small
/// produces tight limits that fail when WASM local costs exceed them; a
/// margin that is too large masks real regressions. The user must supply
/// all four components; defaults are intentionally omitted so a missing
/// input cannot silently degrade to "no margin".
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Margin {
    pub cpu: f64,
    pub memory: f64,
    pub read: f64,
    pub write: f64,
}

impl Margin {
    /// Construct a margin from four non-negative, finite `f64`s.
    /// Rejects NaN, infinity, and any negative value so callers can
    /// reuse this for both CLI- and TOML-supplied margins.
    pub fn new(cpu: f64, memory: f64, read: f64, write: f64) -> Result<Self> {
        for (name, value) in [
            ("cpu", cpu),
            ("memory", memory),
            ("read", read),
            ("write", write),
        ] {
            if !value.is_finite() {
                return Err(Error::Message(format!(
                    "margin.{name} must be a finite number, got {value}"
                )));
            }
            if value < 1.0 {
                // A sub-1.0 margin would tighten the limit below the
                // Tier B measurement — almost never what a user means
                // by "margin", and the source of subtle under-limit
                // failures that masquerade as regressions.
                return Err(Error::Message(format!(
                    "margin.{name} must be >= 1.0 (a margin below 1 would \
                     tighten the limit below the measured Tier B value; \
                     pass --margin <name>=1.0 to write a limit equal to \
                     the Tier B value), got {value}"
                )));
            }
        }
        Ok(Self {
            cpu,
            memory,
            read,
            write,
        })
    }

    /// Return the margin multiplier for the canonical `metric` label
    /// used in `cargo budget-report --json`. Memory maps to the
    /// `memory_bytes` field via the `Memory` arm; the report's literal
    /// `CPU Instructions` label maps to `cpu`; "Read Bytes" / "Write
    /// Bytes" map to `read` / `write` respectively.
    pub fn for_metric(&self, metric: &str) -> Option<f64> {
        match metric {
            "CPU Instructions" => Some(self.cpu),
            "Memory Bytes" | "Memory bytes" | "memory_bytes" => Some(self.memory),
            "Read Bytes" => Some(self.read),
            "Write Bytes" => Some(self.write),
            _ => None,
        }
    }
}

/// Resolved configuration for the derivation command: margins + the set
/// of "scenarios" — Tier A test groupings that span multiple Tier B
/// functions. A scenario limit is the ceiling of the sum of its
/// component Tier B values times the relevant margin.
#[derive(Clone, Debug)]
pub struct DerivationConfig {
    pub margin: Margin,
    /// Map of `scenario -> components` (function names) keyed by
    /// `<package>::<scenario_name>`. Empty by default; populated from
    /// `[[scenarios]]` blocks in `budget.toml`.
    pub scenarios: BTreeMap<String, Vec<String>>,
}

impl DerivationConfig {
    /// Construct a derivation config with no scenarios.
    #[allow(dead_code)]
    pub fn margin_only(margin: Margin) -> Self {
        Self {
            margin,
            scenarios: BTreeMap::new(),
        }
    }
}

/// One measurement record lifted out of a Tier B JSON report.
#[derive(Clone, Debug, Deserialize)]
pub struct TierBMeasurement {
    pub package: String,
    pub function: String,
    pub metric: String,
    pub value: u64,
}

/// One row of the source Tier B JSON after we accept both the bare
/// array and the `{schema_version, snapshots}` wrapped forms.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum TierBReportShape {
    Wrapped {
        #[serde(default = "default_schema_version")]
        #[allow(dead_code)]
        schema_version: u32,
        snapshots: Vec<TierBMeasurement>,
    },
    Bare(Vec<TierBMeasurement>),
}

fn default_schema_version() -> u32 {
    1
}

/// Load and parse a Tier B JSON file. Stdin is supported by passing
/// `-` as `path`. The function accepts both the bare array shape and
/// the wrapped `{schema_version, snapshots}` shape — `cargo
/// budget-report --json` currently emits an array, but the
/// `json_output::render_json` helper wraps it; either is consumed
/// without further massaging.
pub fn load_tier_b_report(path: &Path) -> Result<Vec<TierBMeasurement>> {
    let contents = if path == Path::new("-") {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| Error::Message(format!("failed to read JSON from stdin: {e}")))?;
        buf
    } else {
        std::fs::read_to_string(path)
            .map_err(|e| Error::Message(format!("failed to read {}: {e}", path.display())))?
    };
    let parsed: TierBReportShape = serde_json::from_str(&contents).map_err(|e| {
        Error::Message(format!(
            "failed to parse Tier B JSON as either {{\"schema_version\":..., \"snapshots\":[...]}} or a bare array: {e}"
        ))
    })?;
    Ok(match parsed {
        TierBReportShape::Wrapped { snapshots, .. } => snapshots,
        TierBReportShape::Bare(items) => items,
    })
}

/// One row of the output env file. Each row records both the
/// `tier_a_limit` and the inputs that produced it, so a reviewer can
/// trace any line in the output back to its origin in the Tier B
/// report and budget.toml scenarios.
#[derive(Clone, Debug)]
pub struct DerivedLimit {
    pub key: String,
    pub tier_b_value: u64,
    pub margin: f64,
    pub tier_a_limit: u64,
    pub provenance: String,
}

/// The full result of a derivation: the env-file rows plus the
/// scenario composition table that feeds scenario rows.
#[derive(Clone, Debug, Default)]
pub struct Derivation {
    pub limits: Vec<DerivedLimit>,
}

/// Compute a Tier A limit from a Tier B value + a margin.
///
/// The math is `(value as f64 * margin).ceil()`, then bounded by
/// `u64::MAX`. The bound catches pathological margins (e.g.
/// > `u64::MAX / value`) gracefully rather than panicking on `as`.
///
/// A `value == 0` input yields `0`. Tier A test annotations on that
/// limit will then assert `cost < 0`, which is never true and so
/// always panics — this is intentional: a Tier B report that records
/// zero for a metric (e.g. `extend_instance_ttl - Write Bytes`)
/// means "no charge recorded" and the test author should either drop
/// the macro on that assertion or use `env = "VAR"` and leave the
/// variable unset (which the macro falls back to `u64::MAX`).
fn ceil_apply(value: u64, margin: f64) -> u64 {
    let scaled = (value as f64) * margin;
    if !scaled.is_finite() || scaled < 0.0 {
        return value; // margin < 0 or non-finite; preserve value rather than overflow
    }
    if scaled >= u64::MAX as f64 {
        return u64::MAX;
    }
    scaled.ceil() as u64
}

impl Derivation {
    /// Derive per-function and per-scenario Tier A limits.
    ///
    /// The result is sorted by `key` (via the `BTreeMap` accumulator)
    /// so the emitted env file has stable line ordering across runs.
    pub fn from_report(
        measurements: &[TierBMeasurement],
        config: &DerivationConfig,
    ) -> Result<Self> {
        // First pass: bucket measurements by `(package, function)` and
        // compute per-function Tier A limits. The bucket is keyed
        // `<package>::<function>` to mimic the existing Tier B
        // baseline key style in `compare.rs`.
        let mut per_function: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
        // Track the raw Tier B value per (package, function, metric) so
        // the provenance ledger can cite it. Keys mirror the limit keys
        // but include the metric.
        let mut raw_measurements: BTreeMap<(String, String, String), u64> = BTreeMap::new();

        for m in measurements {
            let margin = config.margin.for_metric(&m.metric).ok_or_else(|| {
                Error::Message(format!(
                    "Tier B metric {:?} has no configured margin; \
                     supply --margin-cpu / --margin-memory / --margin-read / \
                     --margin-write or the [margin.*] section in budget.toml",
                    m.metric
                ))
            })?;
            let limit_key = format!("{}::{}", m.package, m.function);
            let scaled = ceil_apply(m.value, margin);
            // When two metrics in the same function both contribute to a
            // single Tier A limit, the macro emits them as separate
            // (cpu, mem, read, write) values keyed separately. We
            // therefore emit one row per `(package, function, metric)`
            // and avoid silently accumulating unrelated limits under
            // the same key.
            per_function
                .entry(limit_key)
                .or_default()
                .insert(m.metric.clone(), scaled);
            raw_measurements.insert(
                (m.package.clone(), m.function.clone(), m.metric.clone()),
                m.value,
            );
        }

        let mut limits: Vec<DerivedLimit> = Vec::new();

        for (func_key, by_metric) in &per_function {
            for (metric, tier_a_limit) in by_metric {
                let (pkg, fn_name) = func_key
                    .split_once("::")
                    .map(|(p, f)| (p.to_string(), f.to_string()))
                    .unwrap_or_else(|| ("".to_string(), func_key.clone()));
                let margin = config
                    .margin
                    .for_metric(metric)
                    .expect("metric must be a known label");
                let tier_b_value = raw_measurements
                    .get(&(pkg.clone(), fn_name.clone(), metric.clone()))
                    .copied()
                    .unwrap_or(0);
                limits.push(DerivedLimit {
                    key: env_var_key(&pkg, &fn_name, metric),
                    tier_b_value,
                    margin,
                    tier_a_limit: *tier_a_limit,
                    provenance: format!(
                        "{pkg}::{fn_name} [{metric}] tier_b={tier_b_value} × margin={margin:.4} = {tier_a_limit}"
                    ),
                });
            }
        }

        // Second pass: per-scenario sums. A scenario's CPU limit is
        // `ceil(sum(component_cpu_values) * cpu_margin)` and likewise
        // for mem/read/write. Scenarios with no components at all are
        // skipped; scenarios whose components are partially missing
        // return an error so the user sees the broken mapping instead
        // of a half-correct limit. Scenarios prefixed with a package
        // key that matches a row already in `per_function` are left
        // alone (no double-emit).
        for (scenario_full_key, components) in &config.scenarios {
            if components.is_empty() {
                continue;
            }
            let (scenario_pkg, scenario_name) = scenario_full_key
                .split_once("::")
                .map(|(p, n)| (p.to_string(), n.to_string()))
                .unwrap_or_else(|| ("".to_string(), scenario_full_key.clone()));

            // For each metric, sum the Tier B values across components
            // and apply the margin. The missing-component check fires
            // *before* the zero-sum check so a fully-absent scenario
            // produces a clear error rather than a silent skip.
            for metric_label in [
                "CPU Instructions",
                "Memory Bytes",
                "Read Bytes",
                "Write Bytes",
            ] {
                let margin = match config.margin.for_metric(metric_label) {
                    Some(m) => m,
                    None => continue,
                };
                let mut total_tier_b: u64 = 0;
                let mut missing: Vec<String> = Vec::new();
                for component in components {
                    match raw_measurements.get(&(
                        scenario_pkg.clone(),
                        component.clone(),
                        metric_label.to_string(),
                    )) {
                        Some(v) => total_tier_b = total_tier_b.saturating_add(*v),
                        None => missing.push(component.clone()),
                    }
                }
                if !missing.is_empty() {
                    return Err(Error::Message(format!(
                        "scenario {scenario_full_key} includes component(s) {missing:?} \
                         for metric {metric_label:?} but the Tier B report has no value \
                         for {scenario_pkg}::{:?} [{metric_label:?}]; \
                         run `cargo budget-report` to refresh the report",
                        missing
                    )));
                }
                if total_tier_b == 0 {
                    continue;
                }
                let tier_a_limit = ceil_apply(total_tier_b, margin);
                limits.push(DerivedLimit {
                    key: env_var_scenario_key(&scenario_pkg, &scenario_name, metric_label),
                    tier_b_value: total_tier_b,
                    margin,
                    tier_a_limit,
                    provenance: format!(
                        "scenario {scenario_full_key} [{metric_label}] \
                         tier_b={total_tier_b} (sum of {components:?}) \
                         × margin={margin:.4} = {tier_a_limit}"
                    ),
                });
            }
        }

        // Stable sort by key so emitted diffs stay bounded.
        limits.sort_by(|a, b| a.key.cmp(&b.key));

        Ok(Self { limits })
    }

    /// Render the limits as a `.env`-shaped string with a provenanced
    /// header. The output uses `# ...` and `# KEY=value` comment lines
    /// for human-readable auditability; tooling that ignores shell-style
    /// comments (`parse_env_file_value` included) sees only `KEY=value`
    /// pairs.
    pub fn render_env_file(
        &self,
        source_label: &str,
        margin: &Margin,
        build_profile: Option<&str>,
        timestamp_utc: &str,
    ) -> String {
        let mut out = String::new();
        out.push_str(
            "# tier-a-limits.env\n\
             # Auto-generated by `cargo budget-report --derive-limits`.\n\
             # Do not edit by hand: re-run the derivation command to update.\n\
             # Source Tier B JSON: ",
        );
        out.push_str(source_label);
        out.push('\n');
        out.push_str(&format!(
            "# Margins (cpu, memory, read, write): {:.4}, {:.4}, {:.4}, {:.4}\n",
            margin.cpu, margin.memory, margin.read, margin.write
        ));
        if let Some(profile) = build_profile {
            out.push_str(&format!("# Build profile of source WASM: {profile}\n"));
        }
        out.push_str(&format!("# Generated at (UTC): {timestamp_utc}\n"));
        out.push_str(
            "#\n\
             # Per-line provenance (tier_b_value × margin = tier_a_limit):\n",
        );

        for limit in &self.limits {
            out.push_str(&format!("# {}\n", limit.provenance));
        }
        out.push('\n');

        for limit in &self.limits {
            // The `# ` comment follows the `KEY=value` line because
            // `.env` consumers that don't tolerate same-line comments
            // (POSIX shell, plain text) still see a usable value.
            writeln!(
                out,
                "{key}={value}  # {prov}",
                key = limit.key,
                value = limit.tier_a_limit,
                prov = limit.provenance
            )
            .expect("writing to String never fails");
        }
        out
    }

    /// Render the limits as a Markdown table — the artifact one commits
    /// to `tier-a-limits.provenance.md` next to the `.env` file so the
    /// audit trail is visible in PRs without having to opening the env
    /// file with a parser.
    pub fn render_provenance_markdown(
        &self,
        source_label: &str,
        margin: &Margin,
        build_profile: Option<&str>,
        timestamp_utc: &str,
    ) -> String {
        let mut out = String::new();
        out.push_str("# tier-a-limits provenance\n\n");
        out.push_str(&format!("- Source Tier B JSON: `{source_label}`\n"));
        out.push_str(&format!(
            "- Margins (cpu, memory, read, write): `{:.4}`, `{:.4}`, `{:.4}`, `{:.4}`\n",
            margin.cpu, margin.memory, margin.read, margin.write
        ));
        if let Some(profile) = build_profile {
            out.push_str(&format!("- Build profile of source WASM: `{profile}`\n"));
        }
        out.push_str(&format!("- Generated at (UTC): `{timestamp_utc}`\n"));
        out.push_str(
            "\nThis file is auto-generated. Re-run `cargo budget-report --derive-limits` \
             to refresh. The columns are the inputs and result of every Tier A limit; \
             `tier_a_limit = ceil(tier_b_value × margin_metric)`.\n\n",
        );
        out.push_str("| Key | Tier B value | Margin | Tier A limit |\n");
        out.push_str("|---|---:|---:|---:|\n");
        for limit in &self.limits {
            out.push_str(&format!(
                "| `{}` | {} | {:.4} | {} |\n",
                limit.key, limit.tier_b_value, limit.margin, limit.tier_a_limit
            ));
        }
        out.push('\n');
        out
    }
}

/// Convert a `(package, function, metric_label)` triple to the env-var
/// key the test annotations will use. The double-underscore separator
/// survives both shell- and `.env`-style parsing cleanly, while a
/// single-colon (`::`) separator collides with shell PATH semantics.
fn env_var_key(package: &str, function: &str, metric_label: &str) -> String {
    let metric_segment = metric_to_env_segment(metric_label);
    let pkg = package.replace('-', "_").to_ascii_uppercase();
    let fn_ = function.to_ascii_uppercase();
    // Scenario keys use the same prefix with `SCENARIO__<name>`; for
    // ordinary function-level keys we mirror that pattern with the
    // function name in place of `<name>` so consumers can grep
    // uniformly.
    if fn_.is_empty() {
        format!("TIER_A__{pkg}__{metric_segment}")
    } else {
        format!("TIER_A__{pkg}__{fn_}__{metric_segment}")
    }
}

fn env_var_scenario_key(package: &str, scenario: &str, metric_label: &str) -> String {
    let metric_segment = metric_to_env_segment(metric_label);
    let pkg = package.replace('-', "_").to_ascii_uppercase();
    let scenario_u = scenario.to_ascii_uppercase();
    format!("TIER_A__{pkg}__SCENARIO__{scenario_u}__{metric_segment}")
}

fn metric_to_env_segment(metric_label: &str) -> &'static str {
    match metric_label {
        "CPU Instructions" => "CPU",
        "Memory Bytes" | "Memory bytes" | "memory_bytes" => "MEM",
        "Read Bytes" => "READ",
        "Write Bytes" => "WRITE",
        _ => "UNKNOWN",
    }
}

/// Persist the derivation's two outputs.
pub fn write_outputs(
    out_env: &Path,
    out_provenance: Option<&Path>,
    derivation: &Derivation,
    source_label: &str,
    margin: &Margin,
    build_profile: Option<&str>,
    timestamp_utc: &str,
) -> Result<()> {
    let env_body = derivation.render_env_file(source_label, margin, build_profile, timestamp_utc);
    atomic_write(out_env, &env_body)?;
    if let Some(path) = out_provenance {
        let md_body = derivation.render_provenance_markdown(
            source_label,
            margin,
            build_profile,
            timestamp_utc,
        );
        atomic_write(path, &md_body)?;
    }
    Ok(())
}

fn atomic_write(path: &Path, body: &str) -> Result<()> {
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|s| s.to_str()).unwrap_or("")
    ));
    std::fs::write(&tmp, body)
        .map_err(|e| Error::Message(format!("failed to write {}: {e}", tmp.display())))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        Error::Message(format!(
            "failed to rename {} to {}: {e}",
            tmp.display(),
            path.display()
        ))
    })
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn margin() -> Margin {
        Margin::new(1.25, 1.10, 1.50, 2.00).expect("valid test margin")
    }

    #[test]
    fn margin_rejects_sub_one_multiplier() {
        // 1.25 is the realistic minimum the workflow expects; sub-1.0
        // would tighten below the Tier B measurement, which almost
        // never matches user intent and creates "phantom regressions".
        let err = Margin::new(0.5, 1.0, 1.0, 1.0).unwrap_err().to_string();
        assert!(err.contains("margin.cpu must be >= 1.0"), "got: {err}");
    }

    #[test]
    fn margin_rejects_non_finite() {
        let err = Margin::new(f64::NAN, 1.0, 1.0, 1.0)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("margin.cpu must be a finite number"),
            "got: {err}"
        );
    }

    #[test]
    fn margin_rejects_negative() {
        let err = Margin::new(-0.5, 1.0, 1.0, 1.0).unwrap_err().to_string();
        assert!(err.contains("margin.cpu must be >= 1.0"), "got: {err}");
    }

    #[test]
    fn ceil_apply_handles_overflow_safely() {
        // u64::MAX × any margin > 1 must cap rather than panic.
        assert_eq!(ceil_apply(u64::MAX, 2.0), u64::MAX);
        assert_eq!(ceil_apply(u64::MAX, 1.5), u64::MAX);
        assert_eq!(ceil_apply(1000, 1.25), 1250);
        assert_eq!(ceil_apply(101, 1.25), 127); // ceiling rounds up
    }

    #[test]
    fn wrapped_tier_b_report_parses() {
        let body = serde_json::json!({
            "schema_version": 1,
            "snapshots": [
                {"package": "amm-pool-contract", "function": "deposit", "metric": "CPU Instructions", "value": 100},
            ]
        })
        .to_string();
        let parsed: TierBReportShape = serde_json::from_str(&body).unwrap();
        match parsed {
            TierBReportShape::Wrapped { snapshots, .. } => {
                assert_eq!(snapshots.len(), 1);
                assert_eq!(snapshots[0].package, "amm-pool-contract");
                assert_eq!(snapshots[0].function, "deposit");
            }
            other => panic!("expected Wrapped, got {other:?}"),
        }
    }

    #[test]
    fn bare_tier_b_report_parses() {
        let body = serde_json::json!([
            {"package": "p", "function": "f", "metric": "Read Bytes", "value": 2048}
        ])
        .to_string();
        let parsed: TierBReportShape = serde_json::from_str(&body).unwrap();
        match parsed {
            TierBReportShape::Bare(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].value, 2048);
            }
            other => panic!("expected Bare, got {other:?}"),
        }
    }

    #[test]
    fn derive_emits_one_limit_per_metric_per_function() {
        let measurements = vec![
            TierBMeasurement {
                package: "amm-pool-contract".to_string(),
                function: "require_auth_only".to_string(),
                metric: "CPU Instructions".to_string(),
                value: 10_000,
            },
            TierBMeasurement {
                package: "amm-pool-contract".to_string(),
                function: "require_auth_only".to_string(),
                metric: "Memory Bytes".to_string(),
                value: 1_000_000,
            },
        ];
        let config = DerivationConfig::margin_only(margin());
        let derivation = Derivation::from_report(&measurements, &config).unwrap();

        assert_eq!(derivation.limits.len(), 2);
        // 10_000 × 1.25 = 12_500 exactly.
        let cpu = derivation
            .limits
            .iter()
            .find(|l| l.key.ends_with("CPU"))
            .expect("cpu row present");
        assert_eq!(cpu.tier_a_limit, 12_500);
        assert_eq!(cpu.tier_b_value, 10_000);
        // 1_000_000 × 1.10 = 1_100_000.
        let mem = derivation
            .limits
            .iter()
            .find(|l| l.key.ends_with("MEM"))
            .expect("mem row present");
        assert_eq!(mem.tier_a_limit, 1_100_000);
    }

    #[test]
    fn derive_unknown_metric_errors_with_actionable_message() {
        let measurements = vec![TierBMeasurement {
            package: "p".to_string(),
            function: "f".to_string(),
            metric: "WASM Bytes".to_string(),
            value: 12_345,
        }];
        // Margin::new validates the four required multipliers, not the
        // metric labels. The Tier B report row's metric is what we
        // dispatch on. WASM Bytes has no margin (it's a build artifact,
        // not a runtime resource), so the derive step must error.
        let config = DerivationConfig::margin_only(margin());
        let err = Derivation::from_report(&measurements, &config).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("WASM Bytes") && msg.contains("no configured margin"),
            "got: {msg}"
        );
    }

    #[test]
    fn scenario_limit_sums_components_with_margin() {
        let measurements = vec![
            tier_b("amm-pool-contract", "deposit", "CPU Instructions", 30_000),
            tier_b("amm-pool-contract", "swap", "CPU Instructions", 50_000),
            tier_b("amm-pool-contract", "withdraw", "CPU Instructions", 40_000),
        ];
        let mut scenarios = BTreeMap::new();
        scenarios.insert(
            "amm-pool-contract::full_workflow".to_string(),
            vec![
                "deposit".to_string(),
                "swap".to_string(),
                "withdraw".to_string(),
            ],
        );
        let config = DerivationConfig {
            margin: margin(),
            scenarios,
        };
        let derivation = Derivation::from_report(&measurements, &config).unwrap();
        let scenario = derivation
            .limits
            .iter()
            .find(|l| l.key.contains("SCENARIO__FULL_WORKFLOW"))
            .expect("scenario row present");
        // (30k + 50k + 40k) × 1.25 = 150_000.
        assert_eq!(scenario.tier_a_limit, 150_000);
        assert_eq!(scenario.tier_b_value, 120_000);
    }

    #[test]
    fn scenario_with_missing_component_errors() {
        let measurements = vec![tier_b(
            "amm-pool-contract",
            "deposit",
            "CPU Instructions",
            100,
        )];
        let mut scenarios = BTreeMap::new();
        scenarios.insert(
            "amm-pool-contract::full_workflow".to_string(),
            vec!["deposit".to_string(), "ghost".to_string()],
        );
        let config = DerivationConfig {
            margin: margin(),
            scenarios,
        };
        let err = Derivation::from_report(&measurements, &config).unwrap_err();
        assert!(format!("{err}").contains("ghost"), "err: {err}");
    }

    #[test]
    fn render_env_file_includes_provenance_and_values() {
        let limit = DerivedLimit {
            key: "TIER_A__AMM_POOL__REQUIRE_AUTH_ONLY__CPU".to_string(),
            tier_b_value: 10_000,
            margin: 1.25,
            tier_a_limit: 12_500,
            provenance: "amm-pool-contract::require_auth_only [CPU Instructions] tier_b=10000 × margin=1.2500 = 12500".to_string(),
        };
        let body = Derivation {
            limits: vec![limit],
        }
        .render_env_file(
            "build/budget-report.json",
            &margin(),
            Some("release"),
            "2026-01-01T00:00:00Z",
        );
        assert!(body.contains("# tier-a-limits.env"));
        assert!(body.contains("# Source Tier B JSON: build/budget-report.json"));
        assert!(
            body.contains("# Margins (cpu, memory, read, write): 1.2500, 1.1000, 1.5000, 2.0000")
        );
        assert!(body.contains("# Build profile of source WASM: release"));
        assert!(body.contains("# Generated at (UTC): 2026-01-01T00:00:00Z"));
        assert!(body.contains("TIER_A__AMM_POOL__REQUIRE_AUTH_ONLY__CPU=12500"));
        assert!(body.contains("# amm-pool-contract::require_auth_only"));
    }

    #[test]
    fn metric_to_env_segment_handles_label_variants() {
        assert_eq!(metric_to_env_segment("CPU Instructions"), "CPU");
        assert_eq!(metric_to_env_segment("Memory Bytes"), "MEM");
        assert_eq!(metric_to_env_segment("memory_bytes"), "MEM");
        assert_eq!(metric_to_env_segment("Read Bytes"), "READ");
        assert_eq!(metric_to_env_segment("Write Bytes"), "WRITE");
        // Unknown metrics keep their literal segment — emitting a key
        // would still be useful for callers to grep, but the parser
        // for the env form will skip unknown metrics so the value is
        // effectively ignored at test runtime. Better than silently
        // dropping a metric with no warning.
        assert_eq!(metric_to_env_segment("WASM Bytes"), "UNKNOWN");
    }

    #[test]
    fn env_var_key_uppercases_and_double_underscores() {
        let key = env_var_key("amm-pool-contract", "require_auth_only", "CPU Instructions");
        assert_eq!(key, "TIER_A__AMM_POOL_CONTRACT__REQUIRE_AUTH_ONLY__CPU");
    }

    #[test]
    fn env_var_scenario_key_inserts_scenario_marker() {
        let key = env_var_scenario_key("amm-pool-contract", "full_workflow", "CPU Instructions");
        assert_eq!(
            key,
            "TIER_A__AMM_POOL_CONTRACT__SCENARIO__FULL_WORKFLOW__CPU"
        );
    }

    // -- helpers --------------------------------------------------------------

    fn tier_b(p: &str, f: &str, m: &str, v: u64) -> TierBMeasurement {
        TierBMeasurement {
            package: p.to_string(),
            function: f.to_string(),
            metric: m.to_string(),
            value: v,
        }
    }

    #[test]
    fn margin_metric_lookup_round_trip() {
        let m = margin();
        // Sanity: every metric label the JSON report emits maps to the
        // matching margin. Stop adding label variants silently — keep
        // this table in lockstep with `MetricKind::label()` in the
        // baseline comparison logic.
        let table: HashMap<&str, f64> = [
            ("CPU Instructions", m.cpu),
            ("Memory Bytes", m.memory),
            ("Read Bytes", m.read),
            ("Write Bytes", m.write),
        ]
        .into_iter()
        .collect();
        for (label, expected) in table {
            assert_eq!(m.for_metric(label), Some(expected));
        }
    }
}
