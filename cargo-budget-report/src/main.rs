use crate::cli::{BudgetReportArgs, CargoCli, ColorChoice};
use crate::derive::{DerivationConfig, Margin};
use crate::error::{Error, Result, SimulationFailure, SimulationOutcome};
use anyhow::Context;
mod arg_spec;
mod cli;
mod compare;
mod fixture;
mod html_output;
mod live;
mod record;
mod replay;
mod transport;
use cargo_metadata::{CrateType, MetadataCommand};
use clap::Parser;
use compare::{
    build_baseline, check_against_baseline, max_allowed as max_allowed_metric, parse_tolerance,
    render_report_text, Baseline, Measurement, Tolerance,
};
use indicatif::{ProgressBar, ProgressStyle};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;
use stellar_xdr::{Limits, ReadXdr, SorobanTransactionData};
use tabled::settings::object::Rows;
use tabled::settings::Color as TabledColor;
use tabled::settings::Modify;
use tabled::{Table, Tabled};
use wasmparser::Parser as WasmParser;

mod derive;
mod error;
mod watch;

/// Maximum number of total deployment attempts (1 initial + 3 retries)
/// when friendbot funding is suspected to have failed transiently
/// (rate-limiting, network hiccups, or the account not being fully
/// confirmed on-ledger yet).
const MAX_DEPLOY_ATTEMPTS: u32 = 4;

/// Initial backoff delay between deployment retries. Doubles on each
/// subsequent attempt (2 s → 4 s → 8 s).
const INITIAL_RETRY_DELAY_SECS: u64 = 2;

/// `[retry]` section of `budget.toml`.
///
/// Both fields are optional; missing values fall back to the built-in
/// defaults (`MAX_DEPLOY_ATTEMPTS` / `INITIAL_RETRY_DELAY_SECS`).
#[derive(serde::Deserialize, Default, Debug, Clone, Copy)]
pub(crate) struct RetryToml {
    #[serde(default)]
    max_attempts: Option<u32>,
    #[serde(default)]
    initial_backoff_secs: Option<u64>,
}

/// Fully resolved retry policy: CLI over `budget.toml` over defaults.
///
/// The worst-case total sleep for one call site is bounded and derivable
/// from this struct: `initial_backoff * (2^(max_attempts - 1) - 1)`.
/// With the defaults (4 attempts, 2 s initial) that is 2 + 4 + 8 = 14 s.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RetryConfig {
    /// Total attempts including the first. A value of 1 disables retry.
    max_attempts: u32,
    initial_backoff: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        RetryConfig {
            max_attempts: MAX_DEPLOY_ATTEMPTS,
            initial_backoff: Duration::from_secs(INITIAL_RETRY_DELAY_SECS),
        }
    }
}

impl RetryConfig {
    /// True when retry is disabled (a single attempt is made).
    fn disabled(&self) -> bool {
        self.max_attempts <= 1
    }
}

/// Resolves the effective retry policy: CLI flags win over the
/// `budget.toml` `[retry]` section, which wins over the defaults.
pub(crate) fn resolve_retry_config(
    cli_max_attempts: Option<u32>,
    cli_backoff_secs: Option<u64>,
    toml_retry: Option<RetryToml>,
) -> Result<RetryConfig> {
    let mut config = RetryConfig::default();
    if let Some(retry) = toml_retry {
        if let Some(attempts) = retry.max_attempts {
            config.max_attempts = attempts;
        }
        if let Some(secs) = retry.initial_backoff_secs {
            config.initial_backoff = Duration::from_secs(secs);
        }
    }
    if let Some(attempts) = cli_max_attempts {
        config.max_attempts = attempts;
    }
    if let Some(secs) = cli_backoff_secs {
        config.initial_backoff = Duration::from_secs(secs);
    }
    if config.max_attempts == 0 {
        return Err(Error::Message(
            "retry max_attempts must be at least 1 (1 disables retry)".into(),
        ));
    }
    Ok(config)
}

/// Why a single attempt failed, from the point of view of the retry loop.
///
/// Only [`RetryFailure::Transient`] failures are retried; a
/// [`RetryFailure::Permanent`] failure aborts immediately because
/// repeating it cannot change the outcome.
enum RetryFailure {
    /// Plausibly transient: rate-limit responses, connection errors,
    /// timeouts. Worth retrying after a backoff.
    Transient(String),
    /// Deterministic: a contract that does not exist, a malformed XDR,
    /// an RPC-reported simulation error. Retrying would only make the
    /// run slower before failing anyway.
    Permanent(String),
}

/// Heuristically classifies an error message as plausibly transient.
///
/// Retried: rate-limit / HTTP-429 style responses ("rate limit",
/// "rate-limited", "429", "too many requests"), connection-level
/// failures ("connection", "timed out", "timeout", "reset by peer",
/// "broken pipe", "network"), server-side blips ("503", "502", "504",
/// "unavailable", "temporarily", "try again").
///
/// Everything else — unknown errors included — is treated as permanent.
/// A conservative whitelist keeps deterministic failures (missing
/// contract, malformed XDR, simulation errors) from being retried four
/// times before failing anyway.
fn is_transient_error(message: &str) -> bool {
    const TRANSIENT_MARKERS: [&str; 15] = [
        "rate limit",
        "rate-limited",
        "ratelimit",
        "429",
        "too many requests",
        "connection",
        "timed out",
        "timeout",
        "reset by peer",
        "broken pipe",
        "503",
        "502",
        "unavailable",
        "temporarily",
        "try again",
    ];
    let lowered = message.to_ascii_lowercase();
    TRANSIENT_MARKERS.iter().any(|m| lowered.contains(m))
}

/// Runs `op` up to `config.max_attempts` times with exponential backoff.
///
/// Backoff sleeps `config.initial_backoff` before the second attempt and
/// doubles on each further attempt, so the worst-case total sleep is
/// `initial_backoff * (2^(max_attempts - 1) - 1)` — bounded and
/// predictable from configuration alone.
///
/// Progress messages go to stderr unless `quiet` is set. When every
/// attempt fails, `exhausted` builds the final error from the last
/// transient error message so each call site can keep its own wording.
fn run_with_retry<T, Op, ErrFn>(
    config: &RetryConfig,
    quiet: bool,
    label: &str,
    mut op: Op,
    exhausted: ErrFn,
) -> Result<T>
where
    Op: FnMut() -> std::result::Result<T, RetryFailure>,
    ErrFn: FnOnce(&str) -> Error,
{
    let mut last_error = String::new();

    for attempt in 0..config.max_attempts {
        if attempt > 0 {
            let delay_secs = config.initial_backoff.as_secs() * 2u64.pow(attempt - 1);
            if !quiet {
                eprintln!(
                    "{label} attempt {}/{} failed. Retrying in {} s...",
                    attempt, config.max_attempts, delay_secs
                );
            }
            thread::sleep(Duration::from_secs(delay_secs));
        }

        match op() {
            Ok(value) => return Ok(value),
            Err(RetryFailure::Transient(err)) => last_error = err,
            Err(RetryFailure::Permanent(err)) => return Err(exhausted(&err)),
        }
    }

    Err(exhausted(&last_error))
}

/// Commented budget.toml template written by `cargo budget-report --init`.
const BUDGET_TOML_TEMPLATE: &str = r#"# -- Budget report configuration ---------------------------------------------
# Generated by `cargo budget-report --init`.
# See https://github.com/Tollcraft/soroban-budget-assert for the full reference.

# Target network for contract simulation.
# Supported values: "testnet", "futurenet", "local" (for a local container),
# or any custom network defined in your Stellar CLI config.
network = "testnet"

# Stellar source account keypair name (as configured in your Stellar CLI).
source = "alice"

# -- Per-function configuration ----------------------------------------------
# Declare one section per contract function you want to simulate and
# optionally enforce limits against.
#
#   [functions.<function_name>]
#   args = ["--arg1", "value1"]   # CLI arguments forwarded to the function
#   cpu_limit  = 5000000          # optional, inclusive CPU instruction limit
#   read_limit = 5000             # optional, inclusive read-bytes limit
#   write_limit = 1000            # optional, inclusive write-bytes limit
#
# A missing `*_limit` field means that metric is reported but not enforced.
# See `cargo budget-report --check` for limit enforcement.
# Unknown keys inside a [functions.*] block produce an error.
#
# Foreign top-level sections (e.g. [lints] for soroban-cost-linter) are
# silently accepted so that both tools can share a single budget.toml.

[functions.do_expensive_work]
args = ["--n", "10000"]
cpu_limit = 5000000
read_limit = 5000
write_limit = 1000
"#;

/// Top-level configuration deserialized from `budget.toml`.
///
/// Contains optional network and source-account overrides, plus a map of
/// per-function budget configurations keyed by exported function name.
#[derive(serde::Deserialize, Default, Debug)]
pub(crate) struct BudgetToml {
    network: Option<String>,
    source: Option<String>,
    /// Global default tolerance, used unless overridden per function or by `--tolerance`.
    #[serde(default)]
    tolerance: Option<f64>,
    #[serde(default)]
    margin: Option<MarginToml>,
    /// Per-function `[[scenarios.<name>]]` table mapping a scenario to
    /// the list of component function names it sums over. Keys mirror
    /// the `(package, scenario_name)` namespace used by
    /// `derive::env_var_scenario_key`.
    #[serde(default)]
    scenarios: HashMap<String, ScenarioToml>,
    #[serde(default)]
    functions: HashMap<String, FunctionConfig>,
    /// `[retry]` section controlling deploy / simulate / invoke-build
    /// retry behavior. Absent means "use the built-in defaults".
    #[serde(default)]
    retry: Option<RetryToml>,
}

/// Per-metric margin multipliers persisted in `budget.toml`.
///
/// All four fields are independently optional, but `Margin::new`
/// rejects any incomplete configuration at use-time — the
/// `cargo budget-report --derive-limits` flow propagates that error so
/// a half-set `[margin]` block cannot silently degrade to no margin.
#[derive(serde::Deserialize, Default, Debug, Clone, Copy)]
pub(crate) struct MarginToml {
    #[serde(default)]
    cpu_margin: Option<f64>,
    #[serde(default)]
    memory_margin: Option<f64>,
    #[serde(default)]
    read_margin: Option<f64>,
    #[serde(default)]
    write_margin: Option<f64>,
}

impl MarginToml {
    /// Build a [`Margin`] from this record. None of the fields are
    /// allowed to be missing — the caller is responsible for sourcing
    /// missing values from the CLI / failing the run.
    fn into_margin(self) -> Result<Margin> {
        let cpu = self
            .cpu_margin
            .ok_or_else(|| Error::Message("missing margin.cpu_margin in budget.toml".into()))?;
        let memory = self
            .memory_margin
            .ok_or_else(|| Error::Message("missing margin.memory_margin in budget.toml".into()))?;
        let read = self
            .read_margin
            .ok_or_else(|| Error::Message("missing margin.read_margin in budget.toml".into()))?;
        let write = self
            .write_margin
            .ok_or_else(|| Error::Message("missing margin.write_margin in budget.toml".into()))?;
        Margin::new(cpu, memory, read, write)
    }

    /// True when every margin field is set. Used by `derive-mode` to
    /// reject a half-configured file before falling back to CLI args.
    fn is_complete(&self) -> bool {
        self.cpu_margin.is_some()
            && self.memory_margin.is_some()
            && self.read_margin.is_some()
            && self.write_margin.is_some()
    }
}

/// One scenario declaration in the `[[scenarios]]` table.
#[derive(serde::Deserialize, Default, Debug, Clone)]
pub(crate) struct ScenarioToml {
    /// (package, scenario_name) namespace prefix used to scope this
    /// scenario. Without a package, the scenario is treated as package
    /// `""`, which is rarely what callers want — the error path
    /// surfaces that problem.
    #[serde(default)]
    package: Option<String>,
    /// Names of component functions whose Tier B values sum into this
    /// scenario's Tier A limit.
    #[serde(default)]
    functions: Vec<String>,
}

/// Raw resource metrics returned by the Soroban `simulateTransaction` RPC.
///
/// Maps directly to the `resources` field of a `SorobanTransactionData` XDR
/// object decoded from the RPC response.
#[allow(dead_code)]
#[derive(serde::Deserialize, Debug)]
pub(crate) struct Resources {
    instructions: u64,
    disk_read_bytes: u64,
    write_bytes: u64,
}

/// Top-level wrapper for the deserialized `simulateTransaction` response.
///
/// Currently only carries the `resources` sub-object, but exists as a
/// named type so that additional RPC response fields can be added without
/// changing the extraction call-site.
#[allow(dead_code)]
#[derive(serde::Deserialize, Debug)]
pub(crate) struct TransactionData {
    #[serde(alias = "resources")]
    resources: Resources,
}

impl TransactionData {
    #[cfg(test)]
    fn parse_json(json_str: &str) -> anyhow::Result<Self> {
        let parsed_json: serde_json::Value =
            serde_json::from_str(json_str).context("Failed to parse JSON")?;
        serde_json::from_value(parsed_json).context("Failed to deserialize transaction data")
    }
}

/// Per-function configuration read from a `[functions.<name>]` section of
/// `budget.toml`.
///
/// Controls which CLI arguments are forwarded to the contract invocation
/// and which resource limits are enforced in `--check` mode.
#[derive(serde::Deserialize, Default, Debug)]
#[serde(deny_unknown_fields)]
pub(crate) struct FunctionConfig {
    #[serde(default)]
    args: Vec<arg_spec::ArgSpec>,
    /// Inclusive upper bound on the measured CPU `Instructions` metric. `None`
    /// means this metric is reported but not enforced by `--check`.
    #[serde(default)]
    cpu_limit: Option<u64>,
    #[serde(default)]
    read_limit: Option<u64>,
    #[serde(default)]
    write_limit: Option<u64>,
    /// Optional per-function override for the regression tolerance.
    #[serde(default)]
    tolerance: Option<f64>,
}

#[derive(Clone, Copy)]
pub(crate) struct MeasuredResources {
    instructions: u64,
    read_bytes: u64,
    write_bytes: u64,
}

impl MeasuredResources {
    fn as_compare(self) -> Measurement {
        Measurement {
            cpu_instructions: self.instructions,
            read_bytes: self.read_bytes,
            write_bytes: self.write_bytes,
        }
    }
}

/// A single row in the budget report, representing one metric for one
/// exported function of one workspace package.
///
/// In `--check` mode the `limit` and `pass` fields are populated so that
/// consumers (table, JSON, CSV) can render per-metric pass/fail status.
#[derive(Serialize)]
pub(crate) struct CostReport {
    package: String,
    function: String,
    metric: &'static str,
    /// The measured value, or `None` if the simulation failed to produce one
    /// (only emitted in `--check` mode for functions declared in
    /// `budget.toml`).
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<u32>,
    /// Configured upper bound for the metric, if any. Emitted in `--check`
    /// mode only.
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u64>,
    /// `true` if the measured value is within the configured limit, `false`
    /// if it exceeds the limit **or** the simulation failed for a configured
    /// function. Emitted in `--check` mode only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pass: Option<bool>,
}

/// A `CostReport` formatted for rendering in the plain-text [`Table`] output.
///
/// Only rows with a measured value (`value.is_some()`) are included in the
/// table; simulation failures and `--check`-only stubs are filtered out
/// before this type is constructed.
#[derive(Tabled)]
struct TableCostReport {
    package: String,
    function: String,
    metric: &'static str,
    value: String,
}

/// A `CostReport` row for the plain-text table in `--check` mode.
///
/// Extends the default table with the configured limit and a textual
/// pass/fail marker, so a breaching row stays identifiable without any
/// colour at all (log files, colour-blind readers, terminals without
/// ANSI support). Colour, when enabled, is applied on top of these text
/// markers and never replaces them.
#[derive(Tabled)]
struct CheckTableCostReport {
    package: String,
    function: String,
    metric: &'static str,
    value: String,
    limit: String,
    check: &'static str,
}

/// True when the no-color.org convention applies: `NO_COLOR` is present
/// with a non-empty value. Any other value (unset, empty) means colour
/// is permitted.
fn no_color_requested_from(no_color_env: Option<&std::ffi::OsStr>) -> bool {
    match no_color_env {
        Some(value) => !value.is_empty(),
        None => false,
    }
}

fn no_color_requested() -> bool {
    no_color_requested_from(std::env::var_os("NO_COLOR").as_deref())
}

/// Pure decision core for [`color_enabled`], kept free of environment
/// and terminal access so it can be unit-tested exhaustively.
fn color_enabled_with(
    choice: ColorChoice,
    no_color_env_set: bool,
    stdout_is_terminal: bool,
) -> bool {
    if no_color_env_set || !stdout_is_terminal {
        return false;
    }
    match choice {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => true,
    }
}

/// Whether the plain-text report should be colourised for this run.
///
/// Only meaningful in `--check` mode; callers gate on `args.check`
/// before consulting this.
fn color_enabled(choice: ColorChoice) -> bool {
    color_enabled_with(
        choice,
        no_color_requested(),
        std::io::stdout().is_terminal(),
    )
}

/// ANSI reset / foreground codes for standalone summary lines.
///
/// These are deliberately *not* inserted into [`Table`] cells — the
/// table uses tabled's own styling (`Modify` + `Color`) so its column
/// width calculation accounts for the escapes. The summary lines below
/// the table have no width calculation, so plain constants suffice.
const ANSI_RESET: &str = "\u{1b}[0m";
const ANSI_FG_RED: &str = "\u{1b}[31m";

/// Wraps `text` in the given ANSI colour code when `colour` is set;
/// returns `text` unchanged otherwise.
fn paint(colour: bool, code: &str, text: &str) -> String {
    if colour {
        format!("{code}{text}{ANSI_RESET}")
    } else {
        text.to_string()
    }
}

/// Formats a configured limit for display in the tables. Limits wider
/// than u32::MAX are clamped for display; anything near the practical
/// ceiling formats fine.
fn format_limit_display(limit_val: u64, metric: &str) -> String {
    let display_value = u32::try_from(limit_val).unwrap_or(u32::MAX);
    format_with_commas_and_units(u64::from(display_value), metric)
}

/// Renders the plain-text workspace table for `--check` mode.
///
/// Rows carrying a measured value get the extra `limit` and `check`
/// columns (`PASS`/`FAIL`). When `colour` is set, breaching rows are
/// additionally rendered in red through tabled's styling so the escapes
/// never disturb the column-width calculation. Passing rows keep the
/// default style — the distinction comes from colour *and* the text
/// marker, never from colour alone.
fn render_check_table(reports: &[CostReport], colour: bool) -> String {
    let valued: Vec<&CostReport> = reports.iter().filter(|r| r.value.is_some()).collect();
    let rows: Vec<CheckTableCostReport> = valued
        .iter()
        .map(|report| CheckTableCostReport {
            package: report.package.clone(),
            function: report.function.clone(),
            metric: report.metric,
            value: format_with_commas_and_units(
                u64::from(report.value.unwrap_or(0)),
                report.metric,
            ),
            limit: report
                .limit
                .map(|l| format_limit_display(l, report.metric))
                .unwrap_or_else(|| "-".to_string()),
            check: if report.pass == Some(false) {
                "FAIL"
            } else {
                "PASS"
            },
        })
        .collect();
    let mut table = Table::new(rows);
    if colour {
        // Data rows start at table index 1; index 0 is the header row.
        for (idx, report) in valued.iter().enumerate() {
            if report.pass == Some(false) {
                table.with(Modify::new(Rows::new((idx + 1)..(idx + 2))).with(TabledColor::FG_RED));
            }
        }
    }
    table.to_string()
}

/// Returns the configured limit (if any) for the given metric name.
pub(crate) fn limit_for_metric(func_config: &FunctionConfig, metric: &str) -> Option<u64> {
    match metric {
        "CPU Instructions" => func_config.cpu_limit,
        "Read Bytes" => func_config.read_limit,
        "Write Bytes" => func_config.write_limit,
        _ => None,
    }
}

/// Given a measured value and an optional configured limit, returns the
/// `(limit, pass)` pair that should be attached to a `CostReport`.
///
/// * No limit configured → `(None, None)`; the metric is reported but not
///   enforced.
/// * Limit configured and value is within it → `(Some(limit), Some(true))`.
/// * Limit configured and value exceeds it → `(Some(limit), Some(false))`;
///   the caller should mark the check as failed.
pub(crate) fn evaluate_check(value: u32, limit: Option<u64>) -> (Option<u64>, Option<bool>) {
    match limit {
        Some(limit_value) => (Some(limit_value), Some(u64::from(value) <= limit_value)),
        None => (None, None),
    }
}

/// Emit one stub `CostReport` per metric (`CPU Instructions`, `Read Bytes`,
/// `Write Bytes`) so that the `--check` JSON output and check summary make
/// the failure visible per metric.
///
/// * Metrics with a configured `*_limit` get `value: None, limit: Some(n),
///   pass: Some(false)` — the consumer can read the breached limit.
/// * Metrics without a configured limit get `value: None, limit: None,
///   pass: Some(false)` — still a hook for `--check --json` consumers, but
///   the table filter (`value.is_some()`) keeps it out of the plain-text
///   report and the summary lines remain unchanged.
///
/// The caller has already set the `checks_failed` flag for the function as a
/// whole, so emitting one entry per metric — even metrics without a limit —
/// does not change the exit-code semantics.
pub(crate) fn emit_check_failure_entries(
    reports: &mut Vec<CostReport>,
    package_name: &str,
    function: &str,
    func_config: &FunctionConfig,
) {
    for metric in ["CPU Instructions", "Read Bytes", "Write Bytes"] {
        let limit = limit_for_metric(func_config, metric);
        reports.push(CostReport {
            package: package_name.to_string(),
            function: function.to_string(),
            metric,
            value: None,
            limit,
            pass: Some(false),
        });
    }
}

/// Formats a `u64` value with commas for readability and appends the
/// appropriate unit suffix (`inst.` for instructions, `B` for bytes).
///
/// # Arguments
///
/// * `value` - The raw numeric value to format.
/// * `metric` - The metric name; if it contains `"Bytes"` the suffix is
///   `B`, otherwise `inst.`.
pub(crate) fn format_with_commas_and_units(value: u64, metric: &str) -> String {
    let value_str = value.to_string();
    let mut result = String::new();
    let mut digit_count = 0;
    for ch in value_str.chars().rev() {
        if digit_count == 3 {
            result.push(',');
            digit_count = 0;
        }
        result.push(ch);
        digit_count += 1;
    }
    let formatted = result.chars().rev().collect::<String>();

    if metric.contains("Bytes") {
        format!("{} B", formatted)
    } else {
        format!("{} inst.", formatted)
    }
}

/// Extracts CPU instructions, read bytes, and write bytes from a
/// `simulateTransaction` JSON-RPC response.
///
/// The response must contain a `result.transactionData` field holding a
/// base64-encoded `SorobanTransactionData` XDR blob.
///
/// # Errors
///
/// Returns an error if the RPC response contains an `"error"` field, if
/// `transactionData` is missing or not a string, or if the base64 XDR
/// cannot be decoded.
fn extract_metrics(rpc_response: &serde_json::Value) -> Result<(u32, u32, u32)> {
    if let Some(error) = rpc_response.get("error") {
        return Err(Error::Rpc(error.to_string()));
    }

    if let Some(error) = rpc_response.get("result").and_then(|r| r.get("error")) {
        let err_msg = error.as_str().unwrap_or("");
        if !err_msg.is_empty() {
            return Err(Error::Rpc(err_msg.to_string()));
        } else {
            return Err(Error::Rpc(error.to_string()));
        }
    }

    let tx_data_b64 = rpc_response["result"]["transactionData"]
        .as_str()
        .ok_or_else(|| Error::MissingField("transactionData".into()))?;

    // Decode the transaction data natively using the stellar-xdr crate
    // to avoid the overhead and instability of shelling out to the stellar CLI.
    let tx_data = SorobanTransactionData::from_xdr_base64(tx_data_b64, Limits::none())
        .map_err(|e| Error::Xdr(format!("Failed to decode SorobanTransactionData: {}", e)))?;

    Ok((
        tx_data.resources.instructions,
        // Renamed in Protocol 23 XDR: footprint reads that hit disk-backed
        // ledger entries. In-memory reads of live state are no longer metered
        // as read bytes. This is the field the report's "Read Bytes" now tracks.
        tx_data.resources.disk_read_bytes,
        tx_data.resources.write_bytes,
    ))
}

/// Builds the `stellar contract invoke --build-only -- <function> [args..]`
/// argument list for one exported function.
///
/// The resulting argument vector is passed directly to `Command::new("stellar")`.
///
/// # Arguments
///
/// * `contract_id` - The deployed contract ID (hex string).
/// * `source` - The Stellar source account keypair name.
/// * `network` - The target network passphrase or alias.
/// * `function` - The exported function name to invoke.
/// * `func_args` - Additional CLI arguments forwarded after the `--` separator.
fn build_invoke_args(
    contract_id: &str,
    source: &str,
    network: &str,
    function: &str,
    func_args: &[String],
) -> Vec<String> {
    let mut invoke_args = vec![
        "contract".to_string(),
        "invoke".to_string(),
        "--id".to_string(),
        contract_id.to_string(),
        "--source".to_string(),
        source.to_string(),
        "--network".to_string(),
        network.to_string(),
        "--build-only".to_string(),
        "--".to_string(),
        function.to_string(),
    ];
    invoke_args.extend(func_args.iter().cloned());
    invoke_args
}

/// Builds the JSON-RPC `simulateTransaction` request body for a base64 XDR
/// transaction envelope.
///
/// The request conforms to the Stellar JSON-RPC v2.0 specification and
/// contains a single `params.transaction` field with the base64-encoded
/// XDR envelope produced by `stellar contract invoke --build-only`.
///
/// # Arguments
///
/// * `b64_xdr` - The base64-encoded XDR transaction envelope.
fn build_rpc_payload(b64_xdr: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "simulateTransaction",
        "params": {
            "transaction": b64_xdr
        }
    })
}

/// Simulates one exported function end-to-end through a
/// [`transport::Transport`]: builds the invocation transaction, POSTs it to
/// `simulateTransaction` and decodes the reported resource usage.
///
/// `LiveTransport` shells out to `stellar contract invoke --build-only` and
/// `curl`, retrying plausibly transient failures (rate limits, connection
/// errors) with the crate-wide retry configuration; `ReplayTransport`
/// serves the same calls from a recorded fixture, which is how the rest of
/// the crate can be tested without a network.
///
/// Returns `Err` only for a persistent RPC transport failure after every
/// retry attempt is exhausted, or for an unrecoverable environment
/// problem — the tool cannot proceed without those binaries. A
/// *recoverable* simulation failure (non-zero invoke exit, an RPC `error`
/// field, or an undecodable response) is reported as
/// `Ok(SimulationOutcome::Failed(..))` so the caller can move on to the next
/// function instead of aborting the whole report.
pub(crate) fn simulate_function(
    transport: &mut impl transport::Transport,
    contract_id: &str,
    source: &str,
    network: &str,
    function: &str,
    func_args: &[String],
    package: &str,
) -> Result<SimulationOutcome> {
    // Build the invocation XDR through the transport. The live transport
    // reports a failed `stellar contract invoke` as an error (after any
    // transient retries); that is a recoverable per-function failure (the
    // CLI ran, the invocation failed), so it is recorded as
    // `Failed(Invoke(..))` rather than aborting the whole report.
    let b64_xdr = match transport.build_invoke_xdr(
        contract_id,
        source,
        network,
        function,
        func_args,
        package,
    ) {
        Ok(xdr) => xdr,
        Err(err) => {
            return Ok(SimulationOutcome::Failed(SimulationFailure::Invoke(
                format!("{:#}", err),
            )));
        }
    };

    let rpc_resp = transport
        .simulate_transaction(&b64_xdr, package, function)
        .map_err(|e| Error::CommandFailed(format!("{:#}", e)))?;

    if let Some(error) = rpc_resp.get("error") {
        return Ok(SimulationOutcome::Failed(SimulationFailure::Rpc(
            error.to_string(),
        )));
    }

    // Capture the raw transactionData XDR before decode, so --validate
    // can re-decode it through the Stellar CLI independently.
    let tx_data_xdr_b64 = rpc_resp["result"]["transactionData"]
        .as_str()
        .map(|s| s.to_string());

    match extract_metrics(&rpc_resp) {
        Ok((instructions, read_bytes, write_bytes)) => Ok(SimulationOutcome::Metrics {
            instructions,
            read_bytes,
            write_bytes,
            transaction_data_xdr: tx_data_xdr_b64.unwrap_or_default(),
        }),
        Err(err) => Ok(SimulationOutcome::Failed(
            SimulationFailure::MetricsExtraction(format!("{:#}", err)),
        )),
    }
}

/// Loads and parses a `budget.toml` configuration file.
///
/// If the file does not exist, returns a default (empty) configuration
/// so that callers can proceed without explicit error handling.
///
/// # Errors
///
/// Returns an error if the file exists but cannot be read or parsed.
pub(crate) fn load_budget_toml<P: AsRef<Path>>(path: P) -> Result<BudgetToml> {
    match std::fs::read_to_string(&path) {
        Ok(contents) => {
            let trimmed = contents.trim();
            if trimmed.is_empty()
                || trimmed
                    .lines()
                    .all(|line| line.trim().is_empty() || line.trim_start().starts_with('#'))
            {
                return Ok(BudgetToml::default());
            }

            toml::from_str(&contents).map_err(Error::Toml)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(BudgetToml::default()),
        Err(err) => Err(Error::Io(err)),
    }
}

pub(crate) fn resolve_tolerance(
    cli_override: Option<&str>,
    config: &BudgetToml,
) -> Result<Tolerance> {
    if let Some(raw) = cli_override {
        return parse_tolerance(raw).map_err(|e| Error::Message(e.to_string()));
    }
    if let Some(t) = config.tolerance {
        return Ok(Tolerance::new(t));
    }
    Ok(Tolerance::default())
}

#[derive(serde::Serialize)]
struct CheckReportJson<'r> {
    has_regressions: bool,
    regression_count: usize,
    default_tolerance: f64,
    regressions: Vec<RegressionJson<'r>>,
    improvements: Vec<ImprovementJson<'r>>,
    new_entries: Vec<NewEntryJson<'r>>,
    stale_entries: Vec<StaleEntryJson<'r>>,
}

#[derive(serde::Serialize)]
struct RegressionJson<'r> {
    package: &'r str,
    function: &'r str,
    metric: &'r str,
    baseline: u64,
    current: u64,
    tolerance: f64,
    max_allowed: u64,
}

#[derive(serde::Serialize)]
struct ImprovementJson<'r> {
    package: &'r str,
    function: &'r str,
    metric: &'r str,
    baseline: u64,
    current: u64,
    tolerance: f64,
}

#[derive(serde::Serialize)]
struct NewEntryJson<'r> {
    package: &'r str,
    function: &'r str,
}

#[derive(serde::Serialize)]
struct StaleEntryJson<'r> {
    package: &'r str,
    function: &'r str,
}

fn render_check_report_json(
    report: &compare::CheckReport,
    default_tolerance: Tolerance,
) -> serde_json::Value {
    let mut regressions = Vec::new();
    let mut improvements = Vec::new();
    for func in &report.compared {
        for m in &func.metrics {
            match m.verdict {
                compare::Verdict::Regression => {
                    regressions.push(RegressionJson {
                        package: &func.package,
                        function: &func.function,
                        metric: m.metric.label(),
                        baseline: m.baseline,
                        current: m.current,
                        tolerance: m.tolerance.value,
                        max_allowed: max_allowed_metric(m.baseline, m.tolerance.value),
                    });
                }
                compare::Verdict::Improvement => {
                    improvements.push(ImprovementJson {
                        package: &func.package,
                        function: &func.function,
                        metric: m.metric.label(),
                        baseline: m.baseline,
                        current: m.current,
                        tolerance: m.tolerance.value,
                    });
                }
                compare::Verdict::Pass => {}
            }
        }
    }
    let new_entries: Vec<_> = report
        .new
        .iter()
        .map(|e| NewEntryJson {
            package: &e.package,
            function: &e.function,
        })
        .collect();
    let stale_entries: Vec<_> = report
        .stale
        .iter()
        .map(|e| StaleEntryJson {
            package: &e.package,
            function: &e.function,
        })
        .collect();
    let summary = CheckReportJson {
        has_regressions: report.has_regressions(),
        regression_count: report.regression_count(),
        default_tolerance: default_tolerance.value,
        regressions,
        improvements,
        new_entries,
        stale_entries,
    };
    serde_json::to_value(summary).expect("CheckReportJson serialization is infallible")
}

/// Scaffold a commented `budget.toml` template. Errors if the file already
/// exists and `force` is not set.
fn scaffold_init(force: bool, quiet: bool) -> Result<()> {
    let path = Path::new("budget.toml");
    if path.exists() && !force {
        return Err(Error::Message(
            "budget.toml already exists; use --force to overwrite".into(),
        ));
    }
    std::fs::write(path, BUDGET_TOML_TEMPLATE)
        .map_err(|e| Error::Message(format!("failed to write {}: {}", path.display(), e)))?;
    if !quiet {
        eprintln!("Wrote {}", path.display());
    }
    Ok(())
}

/// Run environment preflight checks before building or deploying.
///
/// Each check fails fast with an actionable error message. Checks that are
/// not applicable (e.g. rustup not installed) are silently skipped.
fn run_preflight_checks(quiet: bool) -> Result<()> {
    // ── stellar CLI ─────────────────────────────────────────────────────
    if !quiet {
        eprint!("Checking Stellar CLI... ");
    }
    let stellar_check = Command::new("stellar").arg("--version").output();
    match stellar_check {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(Error::Message(
                "Stellar CLI is not installed or not on PATH.\n\
                 Install it with:  cargo install --locked stellar-cli\n\
                 See: https://github.com/stellar/stellar-cli"
                    .to_string(),
            ));
        }
        Err(e) => {
            return Err(Error::CommandFailed(format!(
                "failed to execute stellar --version: {}",
                e
            )));
        }
        Ok(output) if !output.status.success() => {
            return Err(Error::CommandFailed(format!(
                "Stellar CLI failed to run.\n\
                 stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(_output) => {
            if !quiet {
                eprintln!("found");
            }
        }
    }
    // ── wasm32 target ───────────────────────────────────────────────────
    if !quiet {
        eprint!("Checking wasm32-unknown-unknown target... ");
    }
    let rustup_check = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output();
    match rustup_check {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // rustup is not installed — skip the check silently.
            if !quiet {
                eprintln!("skipped (rustup not found)");
            }
        }
        Err(e) => {
            return Err(Error::CommandFailed(format!(
                "failed to execute rustup: {}",
                e
            )));
        }
        Ok(output) => {
            let installed = String::from_utf8_lossy(&output.stdout);
            if installed
                .lines()
                .any(|line| line.trim() == "wasm32-unknown-unknown")
            {
                if !quiet {
                    eprintln!("found");
                }
            } else {
                return Err(Error::Message(
                    "wasm32-unknown-unknown target is not installed.\n\
                     Install it with:  rustup target add wasm32-unknown-unknown"
                        .to_string(),
                ));
            }
        }
    }

    Ok(())
}

/// Deploys a contract WASM to the network through a [`transport::Transport`]
/// and turns a failed deploy into the canonical user-facing error.
///
/// Retrying is the live transport's job: [`live::LiveTransport`] wraps the
/// `stellar contract deploy` call in the crate-wide retry machinery, which
/// retries friendbot rate limits and other plausibly transient stderr (429,
/// connection errors) up to `retry_config.max_attempts` times with
/// exponential backoff and skips retrying deterministic failures. All this
/// function does is report the outcome with the familiar "source account is
/// funded" hint.
pub(crate) fn deploy_contract_with_retry(
    transport: &mut impl transport::Transport,
    wasm_path: &Path,
    source: &str,
    network: &str,
    package_name: &str,
    retry_config: &RetryConfig,
) -> Result<String> {
    match transport.deploy_contract(wasm_path, source, network, package_name) {
        Ok(contract_id) => Ok(contract_id),
        Err(err) => Err(Error::Message(format!(
            "Failed to deploy {} after {} attempts. Ensure your source account is funded.\nLast error: {}",
            package_name, retry_config.max_attempts, err
        ))),
    }
}

fn run_derive_mode(args: &BudgetReportArgs, toml_config: &BudgetToml) -> Result<()> {
    let Mode::Derive(out_env, out_provenance) = Mode::from_args(args) else {
        return Err(Error::Message(
            "internal: run_derive_mode called outside Derive mode".into(),
        ));
    };

    // 1) Read the Tier B JSON report.
    let from_path = args.from.as_deref().unwrap_or("-");
    let source_label = if from_path == "-" {
        "<stdin>".to_string()
    } else {
        from_path.to_string()
    };
    let from_pathbuf = std::path::PathBuf::from(from_path);
    let measurements = derive::load_tier_b_report(&from_pathbuf)?;

    // 2) Resolve the margin. CLI overrides win over `budget.toml`.
    //    Detect missing-vs-present on the CLI side first so a partial
    //    CLI override errors out instead of falling through to the
    //    toml fallback (which would silently change behaviour).
    fn parse_cli_margin(field: &str, raw: Option<&String>) -> Result<Option<f64>> {
        match raw {
            None => Ok(None),
            Some(text) => text
                .trim()
                .parse::<f64>()
                .map(Some)
                .map_err(|e| Error::Message(format!("invalid --margin-{field} `{text}`: {e}"))),
        }
    }
    let cli_parts = [
        ("cpu", parse_cli_margin("cpu", args.margin_cpu.as_ref())?),
        (
            "memory",
            parse_cli_margin("memory", args.margin_memory.as_ref())?,
        ),
        ("read", parse_cli_margin("read", args.margin_read.as_ref())?),
        (
            "write",
            parse_cli_margin("write", args.margin_write.as_ref())?,
        ),
    ];
    let cli_any = cli_parts.iter().any(|(_, v)| v.is_some());

    let margin = if cli_any {
        let missing: Vec<&str> = cli_parts
            .iter()
            .filter_map(|(name, v)| if v.is_none() { Some(*name) } else { None })
            .collect();
        if !missing.is_empty() {
            return Err(Error::Message(format!(
                "CLI margin is partially set; supply all four \
                 --margin-{{cpu,memory,read,write}} flags or none of them \
                 (missing: {missing:?})"
            )));
        }
        let cpu = cli_parts[0].1.unwrap();
        let memory = cli_parts[1].1.unwrap();
        let read = cli_parts[2].1.unwrap();
        let write = cli_parts[3].1.unwrap();
        Margin::new(cpu, memory, read, write)?
    } else {
        match toml_config.margin.filter(|m| m.is_complete()) {
            Some(m) => m.into_margin()?,
            None => {
                return Err(Error::Message(
                    "no margin supplied; pass --margin-cpu / --margin-memory / \
                     --margin-read / --margin-write, or add a complete [margin] \
                     section to budget.toml"
                        .into(),
                ));
            }
        }
    };

    // 3) Lift budget.toml scenarios into the derivation config.
    let scenarios: BTreeMap<String, Vec<String>> = toml_config
        .scenarios
        .iter()
        .map(|(name, s)| {
            let key = match &s.package {
                Some(pkg) => format!("{pkg}::{name}"),
                None => format!("::{name}"),
            };
            (key, s.functions.clone())
        })
        .collect();
    let config = DerivationConfig { margin, scenarios };

    // 4) Run the derivation and write the outputs atomically.
    let derivation = derive::Derivation::from_report(&measurements, &config)?;
    let timestamp_utc = build_utc_timestamp();
    let provenance = out_provenance.unwrap_or_else(|| default_provenance_path(&out_env));
    derive::write_outputs(
        &out_env,
        Some(&provenance),
        &derivation,
        &source_label,
        &margin,
        args.profile.as_deref(),
        &timestamp_utc,
    )?;

    if !args.quiet {
        eprintln!(
            "Wrote {} ({} limits) and {}",
            out_env.display(),
            derivation.limits.len(),
            provenance.display(),
        );
    }
    Ok(())
}

/// Replace `tier-a-limits.env` / `tier-a-limits.json` / etc. with the
/// matching `*.provenance.md` sibling. The split keeps the standard
/// env/provenance pairing intuitive for the common case.
fn default_provenance_path(out_env: &std::path::Path) -> std::path::PathBuf {
    out_env.with_extension("provenance.md")
}

/// UTC ISO-8601 timestamp at second precision — enough granularity
/// for the provenance header without depending on `chrono`.
fn build_utc_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| Error::Message(format!("system time error: {e}")))
        .map(|d| {
            // Approximate UTC seconds-since-epoch using a 0-based
            // bijection: 86400 seconds/day, 365.25 days/year. Good
            // enough for an audit-trail timestamp; rounding to days
            // would also be acceptable.
            d.as_secs()
        })
        .unwrap_or(0);
    // The header timestamp is descriptive, not asserted, so it is
    // fine to format it loosely. The string-form here is the
    // seconds-since-epoch expressed in ISO-8601 by hand: the
    // calendar math below is intentionally simple (no leap rules
    // beyond the standard 4/100/400-year rule) and is sufficient
    // for human-readable audit trail of when the derivation ran.
    format_unix_timestamp_as_iso8601(now)
}

fn format_unix_timestamp_as_iso8601(secs: u64) -> String {
    // Split into days + remainder; convert days to Y-M-D.
    let days = secs / 86_400;
    let rem_secs = secs % 86_400;
    let hh = rem_secs / 3600;
    let mm = (rem_secs % 3600) / 60;
    let ss = rem_secs % 60;
    let (y, m, d) = days_to_ymd(days);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Days-since-epoch → (year, month, day). Uses the proleptic Gregorian
/// calendar with the standard century-leap corrections.
fn days_to_ymd(days_since_epoch: u64) -> (u64, u64, u64) {
    let mut year: u64 = 1970;
    let mut remaining = days_since_epoch;
    loop {
        let leap = is_leap(year);
        let len = if leap { 366 } else { 365 };
        if remaining < len {
            break;
        }
        remaining -= len;
        year += 1;
    }
    let leap = is_leap(year);
    let month_lengths = [
        31u64,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u64;
    for &len in &month_lengths {
        if remaining < len {
            break;
        }
        remaining -= len;
        month += 1;
    }
    let day = remaining + 1;
    (year, month, day)
}

fn is_leap(y: u64) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

/// The transport a run uses, chosen by CLI flags.
///
/// `--replay <path>` serves every deploy/invoke/simulate response from a
/// recorded fixture, so the whole pipeline runs with no `stellar` CLI,
/// `curl`, or network access. `--record <path>` wraps the live transport
/// and captures every response so a later run can be replayed. Without
/// either flag, runs use [`live::LiveTransport`] directly.
enum TransportKind {
    Live(live::LiveTransport),
    Recording(record::RecordingTransport<live::LiveTransport>),
    Replay(replay::ReplayTransport),
}

impl transport::Transport for TransportKind {
    fn deploy_contract(
        &mut self,
        wasm_path: &Path,
        source: &str,
        network: &str,
        package_name: &str,
    ) -> anyhow::Result<String> {
        match self {
            TransportKind::Live(t) => t.deploy_contract(wasm_path, source, network, package_name),
            TransportKind::Recording(t) => {
                t.deploy_contract(wasm_path, source, network, package_name)
            }
            TransportKind::Replay(t) => t.deploy_contract(wasm_path, source, network, package_name),
        }
    }

    fn build_invoke_xdr(
        &mut self,
        contract_id: &str,
        source: &str,
        network: &str,
        function: &str,
        func_args: &[String],
        package: &str,
    ) -> anyhow::Result<String> {
        match self {
            TransportKind::Live(t) => {
                t.build_invoke_xdr(contract_id, source, network, function, func_args, package)
            }
            TransportKind::Recording(t) => {
                t.build_invoke_xdr(contract_id, source, network, function, func_args, package)
            }
            TransportKind::Replay(t) => {
                t.build_invoke_xdr(contract_id, source, network, function, func_args, package)
            }
        }
    }

    fn simulate_transaction(
        &mut self,
        b64_xdr: &str,
        package: &str,
        function: &str,
    ) -> anyhow::Result<serde_json::Value> {
        match self {
            TransportKind::Live(t) => t.simulate_transaction(b64_xdr, package, function),
            TransportKind::Recording(t) => t.simulate_transaction(b64_xdr, package, function),
            TransportKind::Replay(t) => t.simulate_transaction(b64_xdr, package, function),
        }
    }
}

fn main() -> anyhow::Result<()> {
    let CargoCli::BudgetReport(args) = CargoCli::parse();

    // ── --init: scaffold a template and exit ──────────────────────────
    if args.init {
        scaffold_init(args.force, args.quiet)?;
        return Ok(());
    }

    // ── --derive-limits: read Tier B JSON → write env file, no simulation ──
    // Must run *before* the preflight checks because derivation does
    // not need the `stellar` CLI, network access, or a built WASM.
    // Splitting here keeps the otherwise-expensive setup out of the
    // derivation path entirely.
    if matches!(Mode::from_args(&args), Mode::Derive(..)) {
        let toml_config = load_budget_toml("budget.toml")?;
        run_derive_mode(&args, &toml_config)?;
        return Ok(());
    }

    // ── Preflight environment checks ──────────────────────────────────
    // Replay runs serve every network call from a fixture, so they need
    // neither the `stellar` CLI nor `curl`; skip the checks entirely.
    if args.replay.is_none() {
        run_preflight_checks(args.quiet)?;
    }

    let toml_config = load_budget_toml("budget.toml")?;
    let default_tolerance = resolve_tolerance(args.tolerance.as_deref(), &toml_config)
        .context("failed to resolve tolerance")?;

    let mode = Mode::from_args(&args);

    let retry_config = resolve_retry_config(
        args.max_retry_attempts,
        args.retry_backoff_secs,
        toml_config.retry,
    )
    .context("failed to resolve retry configuration")?;
    if retry_config.disabled() && !args.quiet {
        eprintln!("Retry is disabled (--max-retry-attempts 1): each call gets a single attempt.");
    }

    // ── Watch mode: delegate to the watch loop and exit ────────────────
    if args.watch {
        if !args.quiet {
            eprintln!("Discovering workspace members...");
        }
        let metadata = MetadataCommand::new()
            .no_deps()
            .exec()
            .context("failed to execute cargo metadata")?;
        let network = args
            .network
            .clone()
            .or(toml_config.network.clone())
            .context("missing --network or budget.toml network field")?;
        let source = args
            .source
            .clone()
            .or(toml_config.source.clone())
            .context("missing --source or budget.toml source field")?;
        return watch::watch_loop(
            &args,
            metadata,
            toml_config,
            default_tolerance,
            network,
            source,
            retry_config,
        );
    }

    let network = args
        .network
        .or(toml_config.network.clone())
        .context("missing --network or budget.toml network field")?;
    let source = args
        .source
        .or(toml_config.source.clone())
        .context("missing --source or budget.toml source field")?;

    if !args.quiet {
        eprintln!("Discovering workspace members...");
    }
    let metadata = MetadataCommand::new()
        .no_deps()
        .exec()
        .context("failed to execute cargo metadata")?;

    let mut reports = Vec::new();
    // `measurements` is the basis for both the legacy table output and the
    // snapshot/baseline modes. The BTreeMap ordering carries through to the
    // baseline file for stable PR diffs.
    let mut measurements: BTreeMap<String, BTreeMap<String, MeasuredResources>> = BTreeMap::new();
    let mut has_errors = false;
    let mut checks_failed = false;
    let mut validation_failed = false;

    let build_profile = args.profile.as_deref().unwrap_or("release");

    // All network interaction happens through the transport. Production runs
    // use `LiveTransport` (which owns the retry policy); `--record` wraps it
    // in a `RecordingTransport` that captures every response, and `--replay`
    // serves responses back from a recorded fixture with no network at all.
    let mut transport = if let Some(replay_path) = &args.replay {
        TransportKind::Replay(replay::ReplayTransport::new(
            fixture::FixtureFile::load(replay_path)
                .with_context(|| format!("failed to load replay fixture {}", replay_path))?,
        ))
    } else if args.record.is_some() {
        TransportKind::Recording(record::RecordingTransport::new(live::LiveTransport::new(
            retry_config,
            args.quiet,
        )))
    } else {
        TransportKind::Live(live::LiveTransport::new(retry_config, args.quiet))
    };

    // Union of every function exported by every contract in the workspace.
    // Used at the end of the run (issue #399) to validate that every function
    // configured in `budget.toml` actually exists.
    let mut all_exported: HashSet<String> = HashSet::new();

    for package in metadata.packages {
        let is_cdylib = package
            .targets
            .iter()
            .any(|target| target.crate_types.contains(&CrateType::CDyLib));
        if !is_cdylib {
            continue;
        }

        if !args.quiet {
            eprintln!("Building package '{}' for wasm32...", package.name);
        }
        let build_status = Command::new("cargo")
            .args([
                "build",
                "-p",
                package.name.as_str(),
                "--target",
                "wasm32-unknown-unknown",
                "--profile",
                build_profile,
            ])
            .status()
            .context("failed to build package")?;

        if !build_status.success() {
            anyhow::bail!("Failed to build {}", package.name);
        }

        // Locate the cdylib target to derive the correct WASM filename.
        // A crate's [lib] name may differ from its package name, so we
        // cannot rely on package.name.replace('-', "_").
        let cdylib_target = package
            .targets
            .iter()
            .find(|t| t.crate_types.contains(&CrateType::CDyLib));
        let wasm_name = match cdylib_target {
            Some(target) => target.name.clone(),
            None => {
                eprintln!(
                    "Warning: no cdylib target found for package '{}' — skipping",
                    package.name
                );
                continue;
            }
        };
        let wasm_path = metadata
            .target_directory
            .join("wasm32-unknown-unknown")
            .join(build_profile)
            .join(format!("{}.wasm", wasm_name));

        if !wasm_path.exists() {
            eprintln!(
                "Error: WASM not found at {}\n  Package: {} (lib target: {})\n  The `cargo build` step above should have produced a cdylib WASM at this path.",
                wasm_path.as_str(),
                package.name,
                wasm_name,
            );
            has_errors = true;
            continue;
        }

        // Parse WASM exports
        let wasm_bytes = std::fs::read(&wasm_path)?;
        let wasm_size: u32 = wasm_bytes.len().try_into().unwrap_or(u32::MAX);
        let mut exported_fns: HashSet<String> = HashSet::new();

        for payload in WasmParser::new(0).parse_all(&wasm_bytes) {
            if let wasmparser::Payload::ExportSection(export_section) = payload? {
                for export_item in export_section {
                    let export_item = export_item?;
                    if export_item.kind == wasmparser::ExternalKind::Func {
                        let name = export_item.name.to_string();
                        // Ignore internal and common exports
                        if !name.starts_with('_') && name != "memory" {
                            exported_fns.insert(name.clone());
                            all_exported.insert(name);
                        }
                    }
                }
            }
        }

        if exported_fns.is_empty() {
            if !args.quiet {
                eprintln!("No exported functions found in {}", package.name);
            }
            continue;
        }

        let spinner = if args.quiet {
            None
        } else {
            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::default_spinner()
                    .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "✔"])
                    .template("{spinner:.green} Deploying contract {msg}...")
                    .unwrap(),
            );
            pb.set_message(package.name.to_string());
            pb.enable_steady_tick(std::time::Duration::from_millis(100));
            Some(pb)
        };

        let contract_id = deploy_contract_with_retry(
            &mut transport,
            wasm_path.as_std_path(),
            &source,
            &network,
            &package.name,
            &retry_config,
        )?;

        if let Some(spinner) = spinner {
            spinner.finish_and_clear();
        }

        eprintln!("Contract deployed at: {}", contract_id);

        for function in exported_fns {
            if !args.quiet {
                eprintln!("Simulating function '{}'...", function);
            }

            let func_config = toml_config.functions.get(&function);
            let func_args = match func_config {
                Some(cfg) => arg_spec::render_args(&cfg.args, &function)
                    .map_err(|e| Error::Message(format!("{e:#}")))?,
                None => Vec::new(),
            };

            match simulate_function(
                &mut transport,
                &contract_id,
                &source,
                &network,
                &function,
                &func_args,
                &package.name,
            )? {
                SimulationOutcome::Metrics {
                    instructions,
                    read_bytes,
                    write_bytes,
                    transaction_data_xdr,
                } => {
                    // Record the measurement for baseline/snapshot mode. This
                    // was previously only wired up in stale pre-refactor
                    // code; it belongs here in the success arm.
                    let measured = MeasuredResources {
                        instructions: instructions as u64,
                        read_bytes: read_bytes as u64,
                        write_bytes: write_bytes as u64,
                    };
                    measurements
                        .entry(package.name.as_str().to_string())
                        .or_default()
                        .insert(function.clone(), measured);

                    // Build three CostReport entries for this function. In
                    // --check mode, attach the configured limit and
                    // pass/fail to each entry.
                    for (metric, value) in [
                        ("CPU Instructions", instructions),
                        ("Read Bytes", read_bytes),
                        ("Write Bytes", write_bytes),
                        ("WASM Bytes", wasm_size),
                    ] {
                        let limit = func_config.and_then(|cfg| limit_for_metric(cfg, metric));
                        let (entry_limit, pass) = evaluate_check(value, limit);
                        if pass == Some(false) {
                            checks_failed = true;
                        }
                        reports.push(CostReport {
                            package: package.name.to_string(),
                            function: function.clone(),
                            metric,
                            value: Some(value),
                            limit: entry_limit,
                            pass,
                        });
                    }

                    // ── Optional Stellar CLI validation ──────────────
                    // `--validate` shells out to `stellar xdr decode`, which
                    // replay mode cannot assume exists; skip it there.
                    if args.validate && args.replay.is_none() {
                        let v_result = validate::validate_metrics(
                            &transaction_data_xdr,
                            instructions,
                            read_bytes,
                            write_bytes,
                        );
                        match v_result {
                            validate::ValidationResult::Match => {
                                if !args.quiet {
                                    eprintln!("  ✓ validation passed for '{}'", function);
                                }
                            }
                            validate::ValidationResult::Mismatch { diagnostics } => {
                                validation_failed = true;
                                eprintln!(
                                    "  ✗ VALIDATION FAILED for '{}' in package '{}':",
                                    function, package.name
                                );
                                for d in &diagnostics {
                                    eprintln!("    {}", d);
                                }
                            }
                            validate::ValidationResult::Skipped { reason } => {
                                if !args.quiet {
                                    eprintln!(
                                        "  - validation skipped for '{}': {}",
                                        function, reason
                                    );
                                }
                            }
                        }
                    }
                }
                SimulationOutcome::Failed(failure) => {
                    has_errors = true;
                    if !args.quiet {
                        match &failure {
                            SimulationFailure::Invoke(stderr) => {
                                eprintln!(
                                    "Warning: Simulation failed for {}: {}",
                                    function, stderr
                                );
                            }
                            SimulationFailure::Rpc(error) => {
                                eprintln!("Warning: RPC error for {}: {}", function, error);
                            }
                            SimulationFailure::MetricsExtraction(err) => {
                                eprintln!(
                                    "Warning: Failed to extract metrics for {}: {}",
                                    function, err
                                );
                            }
                        }
                    }
                    if let (true, Some(function_config)) = (args.check, func_config) {
                        // A configured function that won't simulate cannot
                        // satisfy any of its declared limits; record this as
                        // a check failure even if no `*_limit` is set on
                        // this row of budget.toml.
                        checks_failed = true;
                        emit_check_failure_entries(
                            &mut reports,
                            &package.name,
                            &function,
                            function_config,
                        );
                    }
                }
            }
        }
    }

    // Persist the recorded fixture when `--record` was requested, so the
    // run can be reproduced offline with `--replay`.
    if let Some(path) = &args.record {
        match transport {
            TransportKind::Recording(recording) => {
                recording
                    .into_fixture()
                    .save(path)
                    .with_context(|| format!("failed to save fixture to {}", path))?;
                if !args.quiet {
                    eprintln!("Recorded fixture to {}", path);
                }
            }
            _ => unreachable!("--record always constructs a RecordingTransport"),
        }
    }

    // Issue #399: validate budget.toml against the schema before reporting, so
    // a misspelled function name or unknown key fails loudly instead of
    // silently producing a report that omits the function. Runs in every mode
    // that reached this point (Report / Record / Check).
    {
        let available: Vec<String> = all_exported.into_iter().collect();
        if let Ok(content) = std::fs::read_to_string("budget.toml") {
            if let Err(errs) = validate::validate_budget_toml(&content, &available) {
                let report = errs
                    .iter()
                    .map(|e| format!("  - [{}] {}", e.location, e.message))
                    .collect::<Vec<_>>()
                    .join("\n");
                anyhow::bail!("budget.toml validation failed:\n{report}");
            }
        }
    }

    if measurements.is_empty() {
        // `--html` still produces a valid page so a consumer pointed at the
        // output sees an explicit empty state rather than an empty file.
        if args.html {
            println!("{}", html_output::render_html(&[], args.check));
        }
        if !args.quiet {
            eprintln!("No successful simulations to report.");
        }
        if has_errors || (args.check && checks_failed) || validation_failed {
            std::process::exit(1);
        }
        return Ok(());
    }

    // Per-function tolerance overrides from `budget.toml` (top-level plus
    // per-function). Built once so Mode::Check (baseline regression) and the
    // regular --check path below use the same input.
    let tolerance_overrides: BTreeMap<String, Tolerance> = toml_config
        .functions
        .iter()
        .filter_map(|(name, fc)| fc.tolerance.map(|t| (name.clone(), Tolerance::new(t))))
        .collect();

    // Re-shape the local `MeasuredResources` map into the `compare::Measurement`
    // shape so the baseline modes (`--record-baseline`, `--check-baseline`)
    // can hand it to `build_baseline` / `check_against_baseline` without a
    // second walk over the per-package data.
    let measurement_map: BTreeMap<String, BTreeMap<String, Measurement>> = measurements
        .iter()
        .map(|(pkg, fns)| {
            (
                pkg.clone(),
                fns.iter()
                    .map(|(name, m)| (name.clone(), m.as_compare()))
                    .collect(),
            )
        })
        .collect();

    match mode {
        Mode::Record(path) => {
            let baseline = build_baseline(&measurement_map);
            baseline
                .save(&path)
                .with_context(|| format!("failed to save baseline to {}", path.display()))?;
            eprintln!("Recorded baseline to {}", path.display());
            return Ok(());
        }
        Mode::Check(path) => {
            let baseline = Baseline::load(&path)
                .with_context(|| format!("failed to load baseline {}", path.display()))?;
            let report = check_against_baseline(
                &baseline,
                &measurement_map,
                default_tolerance,
                &tolerance_overrides,
            );
            if args.json {
                let json = render_check_report_json(&report, default_tolerance);
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json).context("Failed to serialize JSON")?
                );
            } else {
                print!("{}", render_report_text(&report));
            }
            if report.has_regressions() {
                std::process::exit(1);
            }
            return Ok(());
        }
        Mode::Derive(_, _) => unreachable!("derive mode returns early before this point"),
        Mode::Report => {} // Fall through to the legacy rendering below.
    }

    if args.csv {
        let mut csv_writer = csv::Writer::from_writer(std::io::stdout());
        if args.check {
            csv_writer
                .write_record(["package", "function", "metric", "value", "limit", "pass"])
                .context("Failed to write CSV header")?;
            for report in &reports {
                let value_str = report.value.map(|val| val.to_string()).unwrap_or_default();
                let limit_str = report.limit.map(|lim| lim.to_string()).unwrap_or_default();
                let pass_str = report.pass.map(|p| p.to_string()).unwrap_or_default();
                csv_writer
                    .write_record([
                        report.package.as_str(),
                        report.function.as_str(),
                        report.metric,
                        value_str.as_str(),
                        limit_str.as_str(),
                        pass_str.as_str(),
                    ])
                    .context("Failed to write CSV record")?;
            }
        } else {
            csv_writer
                .write_record(["package", "function", "metric", "value"])
                .context("Failed to write CSV header")?;
            for report in &reports {
                if report.value.is_some() {
                    let value_str = report.value.map(|val| val.to_string()).unwrap_or_default();
                    csv_writer
                        .write_record([
                            report.package.as_str(),
                            report.function.as_str(),
                            report.metric,
                            value_str.as_str(),
                        ])
                        .context("Failed to write CSV record")?;
                }
            }
        }
        csv_writer.flush().context("Failed to flush CSV writer")?;
    } else if args.json {
        let json_output =
            serde_json::to_string_pretty(&reports).context("Failed to serialize report to JSON")?;
        println!("{}", json_output);
    } else if args.html {
        print!("{}", html_output::render_html(&reports, args.check));
    } else {
        // The plain text report path is preserved byte-for-byte when
        // `--check` is not passed: only entries with a measured value are
        // rendered in the table, and summary text is unchanged. Colour
        // exists only in `--check` mode — there are no limits to compare
        // against otherwise.
        println!("\n=== WORKSPACE BUDGET REPORT ===");
        let colour = args.check && color_enabled(args.color);
        let table = if args.check {
            render_check_table(&reports, colour)
        } else {
            let table_reports: Vec<TableCostReport> = reports
                .iter()
                .filter(|report| report.value.is_some())
                .map(|report| {
                    let value = report.value.unwrap_or(0);
                    let formatted = format_with_commas_and_units(u64::from(value), report.metric);
                    TableCostReport {
                        package: report.package.clone(),
                        function: report.function.clone(),
                        metric: report.metric,
                        value: formatted,
                    }
                })
                .collect();
            Table::new(table_reports).to_string()
        };
        println!("{}", table);
        println!("\nSummary: The values above are simulated resource amounts, not fees. They are three of the inputs to the non-refundable resource fee.");
        println!("* Not measured: transaction size, ledger footprint entry counts, refundable fees (rent, events, return value), the inclusion fee, and therefore the total fee charged.");
        println!("* Note: These are simulated numbers on testnet and may vary slightly depending on ledger state.");
        println!("* See the \"Measurement scope\" section of the Tool Reference for what to use instead when you need those figures.");

        // Fixed: was `if check` — `check` is not in scope here, this needs
        // to read the CLI flag `args.check`.
        if args.check {
            println!("\n=== BUDGET CHECKS ===");
            let mut passed: usize = 0;
            let mut failed: usize = 0;
            for report in &reports {
                let Some(pass) = report.pass else {
                    continue;
                };
                let value_str = match report.value {
                    Some(v) => format_with_commas_and_units(u64::from(v), report.metric),
                    None => "<simulation failed>".to_string(),
                };
                let limit_str = report
                    .limit
                    .map(|limit_val| format_limit_display(limit_val, report.metric))
                    .unwrap_or_else(|| "-".to_string());
                let status = if pass {
                    "PASS".to_string()
                } else {
                    paint(colour, ANSI_FG_RED, "FAIL")
                };
                println!(
                    "{}::{} [{}] value={} limit={} {}",
                    report.package, report.function, report.metric, value_str, limit_str, status
                );
                if pass {
                    passed += 1;
                } else {
                    failed += 1;
                }
            }
            println!("Summary: {} check(s) passed, {} failed", passed, failed);
        }
    }
    // PR #195: `--check` exits non-zero when any limit was breached so CI can
    // gate on the result. Mirrors the empty-measurements branch above.
    if (args.check && checks_failed) || validation_failed {
        std::process::exit(1);
    }
    Ok(())
}

mod config;
mod limit_checks;
mod url_checks;
pub mod validate;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Mode {
    Report,
    Record(PathBuf),
    Check(PathBuf),
    /// Tier A limit derivation. The path is the destination env file.
    /// The optional secondary path, when present, is the destination
    /// for the Markdown provenance sidecar (or `None` to derive the
    /// default `<OUT>.provenance.md` next to it).
    Derive(PathBuf, Option<PathBuf>),
}

impl Mode {
    fn from_args(args: &BudgetReportArgs) -> Self {
        if let Some(out) = &args.derive_limits {
            let provenance = args.provenance_out.as_deref().map(PathBuf::from);
            return Mode::Derive(PathBuf::from(out), provenance);
        }
        if let Some(p) = &args.record_baseline {
            Mode::Record(PathBuf::from(p))
        } else if let Some(p) = &args.check_baseline {
            Mode::Check(PathBuf::from(p))
        } else {
            Mode::Report
        }
    }
}

#[cfg(test)]
mod edge_case_tests;

#[cfg(test)]
mod boundary_tests;

#[cfg(test)]
mod additional_edge_tests;

/// Serializes tests that mutate the process working directory.
#[cfg(test)]
static TEST_CWD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use stellar_xdr::WriteXdr;

    const SHARED_BUDGET_TOML: &str = include_str!("../fixtures/shared_budget.toml");

    fn unique_test_path() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is before UNIX_EPOCH")
            .as_nanos();
        path.push(format!("cargo_budget_report_test_{}.toml", nanos));
        path
    }

    // --- Network simulation loop helper tests ---

    #[test]
    fn build_invoke_args_without_function_args() {
        let invoke_args = build_invoke_args("CCONTRACT", "alice", "testnet", "do_work", &[]);
        assert_eq!(
            invoke_args,
            vec![
                "contract",
                "invoke",
                "--id",
                "CCONTRACT",
                "--source",
                "alice",
                "--network",
                "testnet",
                "--build-only",
                "--",
                "do_work",
            ]
        );
    }

    #[test]
    fn build_invoke_args_appends_function_args_after_separator() {
        let func_args = vec!["--n".to_string(), "10000".to_string()];
        let invoke_args = build_invoke_args("CCONTRACT", "alice", "testnet", "do_work", &func_args);
        assert_eq!(
            invoke_args,
            vec![
                "contract",
                "invoke",
                "--id",
                "CCONTRACT",
                "--source",
                "alice",
                "--network",
                "testnet",
                "--build-only",
                "--",
                "do_work",
                "--n",
                "10000",
            ]
        );
    }

    #[test]
    fn build_rpc_payload_wraps_xdr_in_simulate_transaction_request() {
        let payload = build_rpc_payload("AAAAAgAAAAA=");
        assert_eq!(payload["jsonrpc"], "2.0");
        assert_eq!(payload["method"], "simulateTransaction");
        assert_eq!(payload["params"]["transaction"], "AAAAAgAAAAA=");
    }

    #[test]
    fn build_rpc_payload_empty_xdr_is_still_well_formed() {
        let payload = build_rpc_payload("");
        assert_eq!(payload["params"]["transaction"], "");
    }

    // --- Metric extraction tests ---

    const FIXTURE_INSTRUCTIONS: u32 = 1_000_000;
    const FIXTURE_READ_BYTES: u32 = 2_048;
    const FIXTURE_WRITE_BYTES: u32 = 4_096;
    const FIXTURE_RESOURCE_FEE: i64 = 0;

    fn make_fixture_tx_data() -> SorobanTransactionData {
        use stellar_xdr::{LedgerFootprint, SorobanTransactionDataExt, VecM};
        SorobanTransactionData {
            ext: SorobanTransactionDataExt::V0,
            resources: stellar_xdr::SorobanResources {
                footprint: LedgerFootprint {
                    read_only: VecM::default(),
                    read_write: VecM::default(),
                },
                instructions: FIXTURE_INSTRUCTIONS,
                disk_read_bytes: FIXTURE_READ_BYTES,
                write_bytes: FIXTURE_WRITE_BYTES,
            },
            resource_fee: FIXTURE_RESOURCE_FEE,
        }
    }

    fn fixture_rpc_response_json() -> serde_json::Value {
        let tx_data = make_fixture_tx_data();
        let b64 = tx_data
            .to_xdr_base64(Limits::none())
            .expect("failed to encode fixture SorobanTransactionData");
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "transactionData": b64
            }
        })
    }

    #[test]
    fn extract_metrics_from_programmatic_rpc_response() {
        let rpc_json = fixture_rpc_response_json();
        let (instructions, read_bytes, write_bytes) =
            extract_metrics(&rpc_json).expect("extraction should succeed");
        assert_eq!(instructions, FIXTURE_INSTRUCTIONS);
        assert_eq!(read_bytes, FIXTURE_READ_BYTES);
        assert_eq!(write_bytes, FIXTURE_WRITE_BYTES);
    }

    #[test]
    fn extract_metrics_from_fixture_file() {
        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join("simulate_transaction_response_valid.json");
        let fixture_json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&fixture_path).expect("failed to read fixture file"),
        )
        .expect("failed to parse fixture JSON");

        let meta = &fixture_json["_metadata"];
        assert_eq!(meta["network"].as_str(), Some("testnet"));
        assert!(meta["protocol_version"].as_u64().is_some());

        let (instructions, read_bytes, write_bytes) =
            extract_metrics(&fixture_json).expect("extraction from fixture should succeed");
        assert_eq!(instructions, FIXTURE_INSTRUCTIONS);
        assert_eq!(read_bytes, FIXTURE_READ_BYTES);
        assert_eq!(write_bytes, FIXTURE_WRITE_BYTES);
    }

    #[test]
    fn extract_metrics_fails_on_rpc_error() {
        let rpc_json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32600,
                "message": "Invalid Request"
            }
        });
        let result = extract_metrics(&rpc_json);
        assert!(result.is_err());
        let err = format!("{:#}", result.as_ref().unwrap_err());
        assert!(
            err.contains("Invalid Request"),
            "error should mention the RPC error message, got: {}",
            err
        );
    }

    #[test]
    fn extract_metrics_fails_on_missing_transaction_data() {
        let rpc_json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "some_other_field": "no transaction data here"
            }
        });
        let result = extract_metrics(&rpc_json);
        assert!(result.is_err());
        let err = format!("{:#}", result.unwrap_err());
        assert!(
            err.contains("transactionData"),
            "error should mention transactionData, got: {}",
            err
        );
    }

    #[test]
    fn extract_metrics_fails_on_invalid_xdr() {
        let rpc_json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "transactionData": "this-is-not-valid-base64-xdr!!!"
            }
        });
        let result = extract_metrics(&rpc_json);
        assert!(result.is_err(), "extraction should fail on invalid XDR");
    }

    #[test]
    fn extract_metrics_from_malformed_fixture_file() {
        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join("simulate_transaction_response_malformed.json");
        let fixture_json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&fixture_path).expect("failed to read malformed fixture file"),
        )
        .expect("failed to parse malformed JSON");

        let result = extract_metrics(&fixture_json);
        assert!(
            result.is_err(),
            "extraction should fail on malformed response"
        );
        let err = format!("{:#}", result.unwrap_err());
        assert!(
            err.contains("No transaction data available"),
            "Expected specific error message"
        );
    }

    #[test]
    fn extract_metrics_from_cpu_exceeded_fixture_file() {
        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join("simulate_transaction_response_cpu_exceeded.json");
        let fixture_json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&fixture_path)
                .expect("failed to read cpu exceeded fixture file"),
        )
        .expect("failed to parse cpu exceeded JSON");

        let result = extract_metrics(&fixture_json);
        assert!(
            result.is_err(),
            "extraction should fail on cpu exceeded response"
        );
        let err = format!("{:#}", result.unwrap_err());
        assert!(
            err.contains("CPU budget exceeded"),
            "Expected error to mention CPU budget exceeded, got: {}",
            err
        );
    }

    #[test]
    fn extract_metrics_fails_on_non_string_transaction_data() {
        let rpc_json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "transactionData": 12345
            }
        });
        let result = extract_metrics(&rpc_json);
        assert!(
            result.is_err(),
            "extraction should fail when transactionData is not a string"
        );
    }

    // --- Budget toml loading tests ---

    #[test]
    fn missing_budget_toml_returns_default() {
        let path = unique_test_path();
        let _ = fs::remove_file(&path);

        let config = load_budget_toml(&path).expect("missing file should return default");
        assert!(config.network.is_none());
        assert!(config.source.is_none());
        assert!(config.functions.is_empty());
        assert!(config.tolerance.is_none());
    }

    #[test]
    fn empty_budget_toml_returns_default() {
        let path = unique_test_path();
        fs::write(&path, "\n\n").expect("failed to write empty budget.toml");

        let config = load_budget_toml(&path).expect("empty file should return default");
        assert!(config.network.is_none());
        assert!(config.source.is_none());
        assert!(config.functions.is_empty());
    }

    #[test]
    fn malformed_budget_toml_errors_with_parse_message() {
        let path = unique_test_path();
        fs::write(
            &path,
            "network = \"testnet\"\n[functions.do_expensive_work]\nargs = \"--n 10\"\n",
        )
        .expect("failed to write malformed budget.toml");

        let err = load_budget_toml(&path).unwrap_err();
        let err_text = err.to_string();

        assert!(
            err_text.contains("TOML error"),
            "expected TOML error in message, got: {}",
            err_text
        );
        assert!(err_text.contains("line") || err_text.contains("Line"));
        assert!(err_text.contains("column") || err_text.contains("Column"));
    }

    #[test]
    fn budget_toml_parses_global_and_per_function_tolerance() {
        let path = unique_test_path();
        fs::write(
            &path,
            "network = \"testnet\"\ntolerance = 0.10\n\
             [functions.do_expensive_work]\nargs = [\"--n\", \"10\"]\ntolerance = 0.05\n",
        )
        .expect("failed to write budget.toml");
        let config = load_budget_toml(&path).expect("parse should succeed");
        assert_eq!(config.tolerance, Some(0.10));
        let func = config
            .functions
            .get("do_expensive_work")
            .expect("function present");
        assert_eq!(func.tolerance, Some(0.05));
    }

    // --- Tolerance resolution ----------------------------------------------

    #[test]
    fn resolve_tolerance_precedence_cli_over_toml_over_default() {
        let config = BudgetToml {
            tolerance: Some(0.25),
            ..Default::default()
        };
        let t = resolve_tolerance(Some("0.42"), &config).expect("cli should win");
        assert!((t.value - 0.42).abs() < f64::EPSILON);

        let t = resolve_tolerance(None, &config).expect("toml should win");
        assert!((t.value - 0.25).abs() < f64::EPSILON);

        let empty = BudgetToml::default();
        let t = resolve_tolerance(None, &empty).expect("default should win");
        assert!((t.value - 0.10).abs() < f64::EPSILON);
    }

    #[test]
    fn resolve_tolerance_rejects_invalid_cli() {
        let config = BudgetToml::default();
        let err = resolve_tolerance(Some("nope"), &config)
            .unwrap_err()
            .to_string();
        assert!(err.contains("tolerance must be a number"), "got: {err}");
    }

    // --- Retry configuration -------------------------------------------------

    #[test]
    fn resolve_retry_config_precedence_cli_over_toml_over_default() {
        let toml_retry = RetryToml {
            max_attempts: Some(6),
            initial_backoff_secs: Some(5),
        };

        // Defaults when neither source sets anything.
        let config = resolve_retry_config(None, None, None).expect("defaults should resolve");
        assert_eq!(config.max_attempts, MAX_DEPLOY_ATTEMPTS);
        assert_eq!(
            config.initial_backoff,
            Duration::from_secs(INITIAL_RETRY_DELAY_SECS)
        );
        assert!(!config.disabled());

        // budget.toml wins over defaults.
        let config =
            resolve_retry_config(None, None, Some(toml_retry)).expect("toml should resolve");
        assert_eq!(config.max_attempts, 6);
        assert_eq!(config.initial_backoff, Duration::from_secs(5));

        // CLI wins over both.
        let config = resolve_retry_config(Some(1), Some(7), Some(toml_retry))
            .expect("cli should win over toml");
        assert_eq!(config.max_attempts, 1);
        assert_eq!(config.initial_backoff, Duration::from_secs(7));
        assert!(config.disabled(), "max_attempts = 1 must disable retry");
    }

    #[test]
    fn resolve_retry_config_partial_toml_section_keeps_defaults_for_missing_fields() {
        let config = resolve_retry_config(
            None,
            None,
            Some(RetryToml {
                max_attempts: Some(2),
                initial_backoff_secs: None,
            }),
        )
        .expect("partial toml should resolve");
        assert_eq!(config.max_attempts, 2);
        assert_eq!(
            config.initial_backoff,
            Duration::from_secs(INITIAL_RETRY_DELAY_SECS)
        );
    }

    #[test]
    fn resolve_retry_config_rejects_zero_attempts() {
        let err = resolve_retry_config(Some(0), None, None)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("max_attempts must be at least 1"),
            "got: {err}"
        );
    }

    #[test]
    fn is_transient_error_matches_plausibly_retryable_failures() {
        for msg in [
            "friendbot rate-limited (try again later)",
            "HTTP 429 Too Many Requests",
            "Connection refused",
            "request timed out",
            "connection reset by peer",
            "service unavailable",
        ] {
            assert!(
                is_transient_error(msg),
                "{msg:?} should classify as transient"
            );
        }
    }

    #[test]
    fn is_transient_error_treats_deterministic_failures_as_permanent() {
        for msg in [
            "contract CAMOCKCONTRACTID does not exist",
            "Failed to decode XDR: invalid base64",
            "simulation error: HostError",
            "unknown argument --bogus",
            "",
        ] {
            assert!(
                !is_transient_error(msg),
                "{msg:?} should classify as permanent"
            );
        }
    }

    #[test]
    fn run_with_retry_max_attempts_one_makes_exactly_one_call() {
        let config = RetryConfig {
            max_attempts: 1,
            initial_backoff: Duration::from_secs(0),
        };
        let calls = std::cell::Cell::new(0);
        let result = run_with_retry(
            &config,
            true,
            "test",
            || {
                calls.set(calls.get() + 1);
                Err::<(), _>(RetryFailure::Transient("rate-limited".into()))
            },
            |last| Error::Message(format!("exhausted: {last}")),
        );
        assert!(result.is_err());
        assert_eq!(calls.get(), 1, "retry disabled means exactly one attempt");
    }

    #[test]
    fn run_with_retry_does_not_retry_permanent_failures() {
        let config = RetryConfig {
            max_attempts: 4,
            initial_backoff: Duration::from_secs(0),
        };
        let calls = std::cell::Cell::new(0);
        let result: Result<()> = run_with_retry(
            &config,
            true,
            "test",
            || {
                calls.set(calls.get() + 1);
                Err(RetryFailure::Permanent("contract does not exist".into()))
            },
            |last| Error::Message(format!("failed: {last}")),
        );
        assert!(result.is_err());
        assert_eq!(calls.get(), 1, "permanent failures abort immediately");
    }

    #[test]
    fn run_with_retry_reports_last_transient_error_on_exhaustion() {
        let config = RetryConfig {
            max_attempts: 3,
            initial_backoff: Duration::from_secs(0),
        };
        let result: Result<()> = run_with_retry(
            &config,
            true,
            "test",
            || Err(RetryFailure::Transient("still rate-limited".into())),
            |last| Error::Message(format!("exhausted: {last}")),
        );
        let err = result.unwrap_err().to_string();
        assert!(err.contains("still rate-limited"), "got: {err}");
    }

    #[test]
    fn run_with_retry_stops_as_soon_as_an_attempt_succeeds() {
        let config = RetryConfig {
            max_attempts: 4,
            initial_backoff: Duration::from_secs(0),
        };
        let calls = std::cell::Cell::new(0);
        let result: Result<&str> = run_with_retry(
            &config,
            true,
            "test",
            || {
                let n = calls.get();
                calls.set(n + 1);
                if n < 2 {
                    Err(RetryFailure::Transient("timed out".into()))
                } else {
                    Ok("ok")
                }
            },
            |last| Error::Message(format!("exhausted: {last}")),
        );
        assert_eq!(result.expect("should succeed on third attempt"), "ok");
        assert_eq!(calls.get(), 3);
    }

    // --- Mode dispatch ------------------------------------------------------

    #[test]
    fn mode_defaults_to_report_when_no_flags() {
        let args = BudgetReportArgs {
            init: false,
            force: false,
            network: None,
            source: None,
            json: false,
            check: false,
            csv: false,
            html: false,
            record_baseline: None,
            check_baseline: None,
            tolerance: None,
            quiet: false,
            validate: false,
            record: None,
            replay: None,
            profile: None,
            derive_limits: None,
            from: None,
            margin_cpu: None,
            margin_memory: None,
            margin_read: None,
            margin_write: None,
            provenance_out: None,
            max_retry_attempts: None,
            retry_backoff_secs: None,
            color: ColorChoice::Auto,
            watch: false,
        };
        assert_eq!(Mode::from_args(&args), Mode::Report);
    }

    #[test]
    fn mode_distinguishes_record_and_check() {
        let record = BudgetReportArgs {
            init: false,
            force: false,
            network: None,
            source: None,
            json: false,
            check: false,
            csv: false,
            html: false,
            record_baseline: Some("budget-baseline.toml".to_string()),
            check_baseline: None,
            tolerance: None,
            quiet: false,
            validate: false,
            record: None,
            replay: None,
            profile: None,
            derive_limits: None,
            from: None,
            margin_cpu: None,
            margin_memory: None,
            margin_read: None,
            margin_write: None,
            provenance_out: None,
            max_retry_attempts: None,
            retry_backoff_secs: None,
            color: ColorChoice::Auto,
            watch: false,
        };
        assert_eq!(
            Mode::from_args(&record),
            Mode::Record(PathBuf::from("budget-baseline.toml"))
        );

        let check = BudgetReportArgs {
            init: false,
            force: false,
            network: None,
            source: None,
            json: false,
            check: false,
            csv: false,
            html: false,
            record_baseline: None,
            check_baseline: Some("custom.toml".to_string()),
            tolerance: None,
            quiet: false,
            validate: false,
            record: None,
            replay: None,
            profile: None,
            derive_limits: None,
            from: None,
            margin_cpu: None,
            margin_memory: None,
            margin_read: None,
            margin_write: None,
            provenance_out: None,
            max_retry_attempts: None,
            retry_backoff_secs: None,
            color: ColorChoice::Auto,
            watch: false,
        };
        assert_eq!(
            Mode::from_args(&check),
            Mode::Check(PathBuf::from("custom.toml"))
        );
    }

    #[test]
    fn mode_detects_derive() {
        let args = BudgetReportArgs {
            init: false,
            force: false,
            network: None,
            source: None,
            json: false,
            check: false,
            csv: false,
            html: false,
            record_baseline: None,
            check_baseline: None,
            tolerance: None,
            quiet: false,
            validate: false,
            record: None,
            replay: None,
            profile: None,
            derive_limits: Some("tier-a-limits.env".to_string()),
            from: None,
            margin_cpu: None,
            margin_memory: None,
            margin_read: None,
            margin_write: None,
            provenance_out: None,
            max_retry_attempts: None,
            retry_backoff_secs: None,
            color: ColorChoice::Auto,
            watch: false,
        };
        match Mode::from_args(&args) {
            Mode::Derive(out, _) => assert_eq!(out, PathBuf::from("tier-a-limits.env")),
            other => panic!("expected Derive mode, got {other:?}"),
        }
    }

    #[test]
    fn shared_budget_toml_parses_with_foreign_sections() {
        let config: BudgetToml = toml::from_str(SHARED_BUDGET_TOML)
            .expect("shared budget.toml with foreign [lints] section should parse");
        assert_eq!(config.network.as_deref(), Some("testnet"));
        assert_eq!(config.source.as_deref(), Some("alice"));
        assert!(config.functions.contains_key("do_expensive_work"));
        assert!(config.functions.contains_key("require_auth_only"));
    }

    #[test]
    fn unknown_function_keys_produce_error() {
        let err =
            toml::from_str::<BudgetToml>("[functions.do_expensive_work]\ncpu_lmit = 5000000\n")
                .unwrap_err();
        let err_text = err.to_string();
        assert!(
            err_text.contains("unknown field") || err_text.contains("cpu_lmit"),
            "expected error mentioning unknown field or the offending key, got: {err_text}"
        );
    }

    #[test]
    fn known_function_key_spelling_produces_correct_deserialization() {
        let config: BudgetToml = toml::from_str(
            r#"
[functions.do_expensive_work]
cpu_limit = 5000000
read_limit = 5000
write_limit = 1000
"#,
        )
        .expect("valid function config should parse");
        let func = &config.functions["do_expensive_work"];
        assert_eq!(func.cpu_limit, Some(5000000));
        assert_eq!(func.read_limit, Some(5000));
        assert_eq!(func.write_limit, Some(1000));
    }

    // --- TransactionData deserialization tests ---

    #[test]
    fn transaction_data_parsing_deserializes_successfully() {
        let json_str = r#"{"resources": {"instructions": 1000, "disk_read_bytes": 2048, "write_bytes": 3072}}"#;
        let tx_data = TransactionData::parse_json(json_str).expect("Parsing should succeed");
        assert_eq!(tx_data.resources.instructions, 1000);
        assert_eq!(tx_data.resources.disk_read_bytes, 2048);
        assert_eq!(tx_data.resources.write_bytes, 3072);
    }

    #[test]
    fn transaction_data_parsing_fails_on_missing_field() {
        let json_str = r#"{"resources": {"instructions": 1000, "disk_read_bytes": 2048}}"#;
        let result = TransactionData::parse_json(json_str);
        assert!(result.is_err(), "Parsing should fail on missing field");
        let err_msg = format!("{:#}", result.unwrap_err());
        assert!(
            err_msg.contains("write_bytes"),
            "Error should mention missing field, got: {}",
            err_msg
        );
    }

    #[test]
    fn transaction_data_parsing_fails_on_non_numeric_field() {
        let json_str = r#"{"resources": {"instructions": "not-a-number", "disk_read_bytes": 2048, "write_bytes": 3072}}"#;
        let result = TransactionData::parse_json(json_str);
        assert!(result.is_err(), "Parsing should fail on non-numeric field");
        let err_msg = format!("{:#}", result.unwrap_err());
        assert!(
            err_msg.contains("invalid type") || err_msg.contains("not-a-number"),
            "Error should mention type mismatch, got: {}",
            err_msg
        );
    }

    // --- Cost value formatter tests ---

    #[test]
    fn formatter_zero_cpu() {
        assert_eq!(
            format_with_commas_and_units(0, "CPU Instructions"),
            "0 inst."
        );
    }

    #[test]
    fn formatter_zero_bytes() {
        assert_eq!(format_with_commas_and_units(0, "Read Bytes"), "0 B");
    }

    #[test]
    fn formatter_single_digit_cpu() {
        assert_eq!(
            format_with_commas_and_units(7, "CPU Instructions"),
            "7 inst."
        );
    }

    #[test]
    fn formatter_single_digit_bytes() {
        assert_eq!(format_with_commas_and_units(3, "Write Bytes"), "3 B");
    }

    #[test]
    fn formatter_just_below_thousand() {
        assert_eq!(
            format_with_commas_and_units(999, "CPU Instructions"),
            "999 inst."
        );
    }

    #[test]
    fn formatter_at_thousand() {
        assert_eq!(
            format_with_commas_and_units(1_000, "CPU Instructions"),
            "1,000 inst."
        );
    }

    #[test]
    fn formatter_just_above_thousand() {
        assert_eq!(
            format_with_commas_and_units(1_001, "CPU Instructions"),
            "1,001 inst."
        );
    }

    #[test]
    fn formatter_just_below_million() {
        assert_eq!(
            format_with_commas_and_units(999_999, "Read Bytes"),
            "999,999 B"
        );
    }

    #[test]
    fn formatter_at_million() {
        assert_eq!(
            format_with_commas_and_units(1_000_000, "CPU Instructions"),
            "1,000,000 inst."
        );
    }

    #[test]
    fn formatter_just_above_million() {
        assert_eq!(
            format_with_commas_and_units(1_000_001, "Write Bytes"),
            "1,000,001 B"
        );
    }

    #[test]
    fn formatter_ten_million() {
        assert_eq!(
            format_with_commas_and_units(10_000_000, "CPU Instructions"),
            "10,000,000 inst."
        );
    }

    #[test]
    fn formatter_u32_max_cpu() {
        assert_eq!(
            format_with_commas_and_units(u64::from(u32::MAX), "CPU Instructions"),
            "4,294,967,295 inst."
        );
    }

    #[test]
    fn formatter_u32_max_bytes() {
        assert_eq!(
            format_with_commas_and_units(u64::from(u32::MAX), "Read Bytes"),
            "4,294,967,295 B"
        );
    }

    #[test]
    fn formatter_write_bytes_gets_byte_unit() {
        assert_eq!(
            format_with_commas_and_units(4_096, "Write Bytes"),
            "4,096 B"
        );
    }

    #[test]
    fn formatter_non_bytes_metric_gets_inst_unit() {
        assert_eq!(
            format_with_commas_and_units(500, "Some Other Metric"),
            "500 inst."
        );
    }

    // --- Check-result colouring ---------------------------------------------

    fn mixed_pass_fail_reports() -> Vec<CostReport> {
        vec![
            CostReport {
                package: "my-contract".to_string(),
                function: "do_work".to_string(),
                metric: "CPU Instructions",
                value: Some(1_000_000),
                limit: Some(5_000_000),
                pass: Some(true),
            },
            CostReport {
                package: "my-contract".to_string(),
                function: "do_work".to_string(),
                metric: "Write Bytes",
                value: Some(4_096),
                limit: Some(1_000),
                pass: Some(false),
            },
            CostReport {
                package: "my-contract".to_string(),
                function: "do_work".to_string(),
                metric: "Read Bytes",
                value: Some(2_048),
                limit: None,
                pass: None,
            },
        ]
    }

    #[test]
    fn color_decision_auto_requires_terminal_and_no_no_color() {
        use ColorChoice::{Always, Auto, Never};
        assert!(color_enabled_with(Auto, false, true));
        assert!(!color_enabled_with(Auto, false, false));
        assert!(!color_enabled_with(Auto, true, true));
        assert!(!color_enabled_with(Auto, true, false));
        // Explicit colour still cannot override NO_COLOR or non-terminal
        // suppression.
        assert!(!color_enabled_with(Always, true, false));
        assert!(!color_enabled_with(Always, true, true));
        assert!(color_enabled_with(Always, false, true));
        assert!(!color_enabled_with(Never, false, true));
    }

    #[test]
    fn no_color_convention_only_non_empty_value_disables_colour() {
        use std::ffi::OsStr;
        assert!(no_color_requested_from(Some(OsStr::new("1"))));
        assert!(!no_color_requested_from(Some(OsStr::new(""))));
        assert!(!no_color_requested_from(None));
    }

    #[test]
    fn check_table_carries_pass_fail_text_without_colour() {
        let reports = mixed_pass_fail_reports();
        let table = render_check_table(&reports, false);
        assert!(table.contains("PASS"), "marker column must exist: {table}");
        assert!(table.contains("FAIL"), "marker column must exist: {table}");
        assert!(
            !table.contains('\u{1b}'),
            "no ANSI escapes when colour disabled: {table:?}"
        );
        assert!(table.contains("limit"), "limit column must exist: {table}");
        assert!(table.contains("-"), "unconfigured limit renders as dash");
    }

    #[test]
    fn check_table_colours_only_breaching_rows_when_enabled() {
        let reports = mixed_pass_fail_reports();
        let table = render_check_table(&reports, true);
        let red = "\u{1b}[31m";
        assert!(
            table.contains(red),
            "breaching rows must be red when colour enabled: {table:?}"
        );
        let fail_line = table
            .lines()
            .find(|line| line.contains("FAIL"))
            .expect("FAIL marker present");
        assert!(
            fail_line.contains(red),
            "the FAIL row carries the escape: {fail_line:?}"
        );
        let pass_line = table
            .lines()
            .find(|line| line.contains("PASS"))
            .expect("PASS marker present");
        assert!(
            !pass_line.contains('\u{1b}'),
            "passing rows stay default-styled: {pass_line:?}"
        );
    }

    #[test]
    fn check_table_skips_simulation_failure_rows_like_default_table() {
        let mut reports = mixed_pass_fail_reports();
        reports.push(CostReport {
            package: "my-contract".to_string(),
            function: "broken".to_string(),
            metric: "CPU Instructions",
            value: None,
            limit: Some(5_000),
            pass: Some(false),
        });
        let table = render_check_table(&reports, true);
        assert!(
            !table.contains("broken"),
            "value-less rows stay out of the workspace table: {table}"
        );
    }

    #[test]
    fn paint_wraps_text_only_when_enabled() {
        assert_eq!(paint(false, ANSI_FG_RED, "FAIL"), "FAIL");
        assert_eq!(paint(true, ANSI_FG_RED, "FAIL"), "\u{1b}[31mFAIL\u{1b}[0m");
    }

    #[test]
    fn csv_output_is_never_coloured_even_in_check_mode() {
        // The CSV writer path takes its data straight from `CostReport`
        // fields; this asserts the contract end-to-end for a coloured run's
        // worth of rows.
        let reports = mixed_pass_fail_reports();
        let csv = reports_to_csv(&reports, true);
        assert!(!csv.contains('\u{1b}'), "CSV must be plain: {csv:?}");
        assert!(csv.contains(",false"));
    }

    // --- CSV serialization tests ---

    /// Helper to serialize a slice of CostReport to CSV bytes and return the
    /// result as a String, using the same logic as the `--csv` output path.
    fn reports_to_csv(reports: &[CostReport], check: bool) -> String {
        let mut csv_writer = csv::Writer::from_writer(vec![]);
        if check {
            csv_writer
                .write_record(["package", "function", "metric", "value", "limit", "pass"])
                .unwrap();
            for report in reports {
                let value_str = report.value.map(|val| val.to_string()).unwrap_or_default();
                let limit_str = report.limit.map(|lim| lim.to_string()).unwrap_or_default();
                let pass_str = report.pass.map(|p| p.to_string()).unwrap_or_default();
                csv_writer
                    .write_record([
                        report.package.as_str(),
                        report.function.as_str(),
                        report.metric,
                        value_str.as_str(),
                        limit_str.as_str(),
                        pass_str.as_str(),
                    ])
                    .unwrap();
            }
        } else {
            csv_writer
                .write_record(["package", "function", "metric", "value"])
                .unwrap();
            for report in reports {
                if report.value.is_some() {
                    let value_str = report.value.map(|val| val.to_string()).unwrap_or_default();
                    csv_writer
                        .write_record([
                            report.package.as_str(),
                            report.function.as_str(),
                            report.metric,
                            value_str.as_str(),
                        ])
                        .unwrap();
                }
            }
        }
        csv_writer.flush().unwrap();
        String::from_utf8(csv_writer.into_inner().unwrap()).unwrap()
    }

    #[test]
    fn csv_output_without_check_has_four_columns() {
        let reports = vec![
            CostReport {
                package: "my-contract".to_string(),
                function: "do_work".to_string(),
                metric: "CPU Instructions",
                value: Some(1_000_000),
                limit: None,
                pass: None,
            },
            CostReport {
                package: "my-contract".to_string(),
                function: "do_work".to_string(),
                metric: "Read Bytes",
                value: Some(2_048),
                limit: None,
                pass: None,
            },
        ];
        let csv = reports_to_csv(&reports, false);
        let expected = concat!(
            "package,function,metric,value\n",
            "my-contract,do_work,CPU Instructions,1000000\n",
            "my-contract,do_work,Read Bytes,2048\n",
        );
        assert_eq!(csv, expected);
    }

    #[test]
    fn csv_output_with_check_has_six_columns() {
        let reports = vec![
            CostReport {
                package: "my-contract".to_string(),
                function: "do_work".to_string(),
                metric: "CPU Instructions",
                value: Some(1_000_000),
                limit: Some(5_000_000),
                pass: Some(true),
            },
            CostReport {
                package: "my-contract".to_string(),
                function: "do_work".to_string(),
                metric: "Write Bytes",
                value: Some(4_096),
                limit: Some(1_000),
                pass: Some(false),
            },
        ];
        let csv = reports_to_csv(&reports, true);
        let expected = concat!(
            "package,function,metric,value,limit,pass\n",
            "my-contract,do_work,CPU Instructions,1000000,5000000,true\n",
            "my-contract,do_work,Write Bytes,4096,1000,false\n",
        );
        assert_eq!(csv, expected);
    }

    #[test]
    fn csv_output_without_check_excludes_null_values() {
        let reports = vec![
            CostReport {
                package: "my-contract".to_string(),
                function: "do_work".to_string(),
                metric: "CPU Instructions",
                value: None,
                limit: None,
                pass: None,
            },
            CostReport {
                package: "my-contract".to_string(),
                function: "do_work".to_string(),
                metric: "Read Bytes",
                value: Some(2_048),
                limit: None,
                pass: None,
            },
        ];
        let csv = reports_to_csv(&reports, false);
        let expected = concat!(
            "package,function,metric,value\n",
            "my-contract,do_work,Read Bytes,2048\n",
        );
        assert_eq!(csv, expected);
    }

    #[test]
    fn csv_output_with_check_includes_simulation_failures() {
        let reports = vec![CostReport {
            package: "my-contract".to_string(),
            function: "do_work".to_string(),
            metric: "CPU Instructions",
            value: None,
            limit: Some(5_000_000),
            pass: Some(false),
        }];
        let csv = reports_to_csv(&reports, true);
        let expected = concat!(
            "package,function,metric,value,limit,pass\n",
            "my-contract,do_work,CPU Instructions,,5000000,false\n",
        );
        assert_eq!(csv, expected);
    }

    #[test]
    fn csv_output_empty_reports_produces_header_only() {
        let reports: Vec<CostReport> = vec![];
        let csv = reports_to_csv(&reports, false);
        assert_eq!(csv, "package,function,metric,value\n");
    }
}
