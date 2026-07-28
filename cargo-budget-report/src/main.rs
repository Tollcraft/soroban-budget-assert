pub mod module_11;

use anyhow::{Context, Result};
use cargo_metadata::MetadataCommand;
use clap::Parser;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};
use stellar_xdr::curr::{Limits, ReadXdr, SorobanTransactionData};
use wasmparser::{Parser as WasmParser, Payload};

const BUDGET_TOML_TEMPLATE: &str = r#"# Target network for contract simulation.
network = "testnet"
source = "alice"

[functions.example]
args = []
"#;

#[derive(Parser, Debug)]
#[command(name = "cargo", bin_name = "cargo")]
enum CargoCli {
    BudgetReport(BudgetReportArgs),
}

#[derive(Parser, Debug)]
struct BudgetReportArgs {
    #[arg(long)]
    init: bool,
    #[arg(long)]
    force: bool,
    #[arg(long)]
    network: Option<String>,
    #[arg(long)]
    source: Option<String>,
    /// Override the RPC endpoint used for simulation.
    ///
    /// When supplied, --network-passphrase must also be supplied.
    #[arg(long, requires = "network_passphrase")]
    rpc_url: Option<String>,
    /// Network passphrase for a custom RPC endpoint.
    #[arg(long)]
    network_passphrase: Option<String>,
    #[arg(long, default_value_t = false)]
    json: bool,
    #[arg(long, default_value_t = false)]
    check: bool,
    #[arg(long, default_value_t = false)]
    csv: bool,
    #[arg(long, default_value_t = false)]
    quiet: bool,

    /// Validate reported metrics against the Stellar CLI's own XDR decoder.
    ///
    /// For each successfully simulated function, the base64 SorobanTransactionData
    /// XDR from the RPC response is re-decoded through `stellar xdr decode` and
    /// the resulting metrics are compared against cargo-budget-report's values.
    /// Any discrepancy is reported as a diagnostic; the tool still exits with a
    /// non-zero status when mismatches are found.
    ///
    /// Validation is skipped (not failed) when the Stellar CLI or the `xdr decode`
    /// subcommand is unavailable.
    #[arg(long, default_value_t = false)]
    validate: bool,

    /// Cargo build profile to use when compiling the contract WASM.
    ///
    /// Defaults to `release` when not provided. Custom profiles (e.g.
    /// `release-opt`) must be defined in the project's `Cargo.toml`.
    #[arg(long)]
    profile: Option<String>,

    /// Derive local (Tier A) test limits from a Tier B JSON report and
    /// exit. Reads the Tier B report from `--from <PATH>` (or stdin if
    /// `--from -`) and writes the chosen `KEY=VALUE` shape to the file
    /// at `<OUT>`.
    ///
    /// The Tier B report is the same JSON shape `cargo budget-report
    /// --json` emits — either the bare array of `CostReport`-shaped
    /// rows or the `{schema_version, snapshots}` wrapped form. The
    /// `--margin-{cpu,memory,read,write}` flags (or the `[margin]`
    /// section of `budget.toml`) supply the per-metric multipliers
    /// applied to the Tier B values; the resulting ceilings become
    /// Tier A test limits.
    ///
    /// The function-to-scenario mapping is recorded under
    /// `[[scenarios.<name>]]` blocks in `budget.toml` so component
    /// limits can be summed under a single Tier A `KEY=VALUE` for
    /// tests that exercise multi-step workflows.
    #[arg(long, value_name = "OUT")]
    derive_limits: Option<String>,

    /// Source Tier B JSON report for `--derive-limits`. Use `-` to
    /// read JSON from stdin (so `cargo budget-report --json | cargo
    /// budget-report --derive-limits tier-a-limits.env` composes).
    #[arg(long, value_name = "PATH")]
    from: Option<String>,

    /// Per-metric multiplier applied to Tier B CPU values. Required
    /// unless `[margin].cpu_margin` is set in `budget.toml`; no
    /// default is applied because the project deliberately treats the
    /// margin as data (issue #45) and silently picking a value would
    /// defeat the audit trail.
    #[arg(long, value_name = "F")]
    margin_cpu: Option<String>,

    /// Per-metric multiplier applied to Tier B memory values.
    #[arg(long, value_name = "F")]
    margin_memory: Option<String>,

    /// Per-metric multiplier applied to Tier B read-bytes values.
    #[arg(long, value_name = "F")]
    margin_read: Option<String>,

    /// Per-metric multiplier applied to Tier B write-bytes values.
    #[arg(long, value_name = "F")]
    margin_write: Option<String>,

    /// Path to write the Markdown provenance table next to the env
    /// file. Defaults to `<OUT>` with `.env` replaced by `.md` (e.g.
    /// `tier-a-limits.provenance.md` for `tier-a-limits.env`).
    #[arg(long, value_name = "PATH")]
    provenance_out: Option<String>,

    /// Append per-package subtotal rows and a workspace total row to
    /// the human-readable `--format table` output. JSON, CSV, and
    /// check/baseline/derive modes are unchanged.
    ///
    /// Aggregates are computed only over successfully simulated
    /// functions; failed simulations are excluded. Summed per metric,
    /// never across metrics (different units).
    #[arg(long, default_value_t = false)]
    totals: bool,
}

#[derive(serde::Deserialize, Default, Debug)]
struct BudgetToml {
    /// Network to target. Defaults to `"testnet"` when not specified.
    #[serde(default = "default_network")]
    network: Option<String>,
    /// Stellar source account keypair name. Defaults to `"alice"` when not specified.
    #[serde(default = "default_source")]
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
}

#[derive(serde::Deserialize, Default, Debug, Clone)]
#[serde(deny_unknown_fields)]
struct FunctionConfig {
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    cpu_limit: Option<u64>,
    #[serde(default)]
    mem_limit: Option<u64>,
    #[serde(default)]
    read_limit: Option<u64>,
    #[serde(default)]
    write_limit: Option<u64>,
    /// Optional per-function override for the regression tolerance.
    #[serde(default)]
    tolerance: Option<f64>,
}

#[derive(Clone, Copy)]
struct MeasuredResources {
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

#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
struct CacheEntry {
    wasm_sha256: String,
    contract_id: String,
    network: String,
}

#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
struct BudgetCache {
    package: HashMap<String, CacheEntry>,
}

#[derive(Serialize)]
struct CostReport {
    package: String,
    function: String,
    metric: &'static str,
    value: u32,
}

fn build_network_args(
    args: &mut Vec<String>,
    network: &str,
    rpc_url: Option<&str>,
    network_passphrase: Option<&str>,
) {
    if let (Some(url), Some(passphrase)) = (rpc_url, network_passphrase) {
        args.extend([
            "--rpc-url".to_string(),
            url.to_string(),
            "--network-passphrase".to_string(),
            passphrase.to_string(),
        ]);
    } else {
        args.extend(["--network".to_string(), network.to_string()]);
    }
}

fn build_invoke_args(
    contract_id: &str,
    source: &str,
    network: &str,
    function: &str,
    func_args: &[String],
    rpc_url: Option<&str>,
    network_passphrase: Option<&str>,
) -> Vec<String> {
    let mut invoke_args = vec![
        "contract".to_string(),
        "invoke".to_string(),
        "--id".to_string(),
        contract_id.to_string(),
        "--source".to_string(),
        source.to_string(),
    ];
    build_network_args(&mut invoke_args, network, rpc_url, network_passphrase);
    invoke_args.extend(["--build-only".to_string(), "--".to_string(), function.to_string()]);
    invoke_args.extend(func_args.iter().cloned());
    invoke_args
}

fn build_rpc_payload(b64_xdr: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "simulateTransaction",
        "params": { "transaction": b64_xdr }
    })
}

fn simulate_transaction_rpc(b64_xdr: &str, rpc_url: &str) -> Result<serde_json::Value> {
    let payload = serde_json::to_string(&build_rpc_payload(b64_xdr))?;
    let output = Command::new("curl")
        .args(["-sS", "-X", "POST", "-H", "Content-Type: application/json", "-d", &payload, rpc_url])
        .output()
        .context("failed to execute curl")?;
    if !output.status.success() {
        anyhow::bail!("RPC request failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    serde_json::from_slice(&output.stdout).context("failed to parse RPC response")
}

fn extract_metrics(response: &serde_json::Value) -> Result<(u32, u32, u32)> {
    if let Some(error) = response.get("error") {
        anyhow::bail!("RPC error: {error}");
    }
    let encoded = response["result"]["transactionData"]
        .as_str()
        .context("No transactionData found in simulateTransaction response")?;
    let data = SorobanTransactionData::from_xdr_base64(encoded, Limits::none())
        .context("failed to decode SorobanTransactionData")?;
    Ok((
        data.resources.instructions,
        data.resources.read_bytes,
        data.resources.write_bytes,
    ))
}

fn build_contract(package: &str) -> Result<()> {
    let status = Command::new("cargo")
        .args(["build", "-p", package, "--release", "--target", "wasm32-unknown-unknown"])
        .status()
        .context("failed to execute cargo build")?;
    if !status.success() {
        anyhow::bail!("failed to build package {package}");
    }
    Ok(())
}

fn wasm_path(package: &str) -> String {
    format!("target/wasm32-unknown-unknown/release/{}.wasm", package.replace('-', "_"))
}

fn exported_functions(path: &Path) -> Result<Vec<String>> {
    let bytes = std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut functions = Vec::new();
    for payload in WasmParser::new(0).parse_all(&bytes) {
        if let Payload::ExportSection(exports) = payload? {
            for export in exports {
                let export = export?;
                if export.kind == wasmparser::ExternalKind::Func {
                    functions.push(export.name.to_string());
                }
            }
        }
    }
    Ok(functions)
}

fn deploy_contract(
    wasm: &Path,
    source: &str,
    network: &str,
    rpc_url: Option<&str>,
    network_passphrase: Option<&str>,
) -> Result<String> {
    let mut args = vec![
        "contract".to_string(),
        "deploy".to_string(),
        "--wasm".to_string(),
        wasm.display().to_string(),
        "--source".to_string(),
        source.to_string(),
    ];
    build_network_args(&mut args, network, rpc_url, network_passphrase);
    let output = Command::new("stellar")
        .args(args)
        .output()
        .context("failed to execute stellar contract deploy")?;
    if !output.status.success() {
        anyhow::bail!("contract deployment failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .map(ToOwned::to_owned)
        .context("deployment did not return a contract ID")
}

fn invoke_and_simulate(
    contract_id: &str,
    source: &str,
    network: &str,
    function: &str,
    function_config: &FunctionConfig,
    rpc_url: &str,
    custom_rpc_url: Option<&str>,
    network_passphrase: Option<&str>,
) -> Result<(u32, u32, u32)> {
    let args = build_invoke_args(
        contract_id,
        source,
        network,
        function,
        &function_config.args,
        custom_rpc_url,
        network_passphrase,
    );
    let output = Command::new("stellar")
        .args(args)
        .stdout(Stdio::piped())
        .output()
        .context("failed to execute stellar contract invoke")?;
    if !output.status.success() {
        anyhow::bail!("contract invocation failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    let transaction = String::from_utf8_lossy(&output.stdout)
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .context("contract invocation did not return a transaction")?;
    let response = simulate_transaction_rpc(transaction, rpc_url)?;
    extract_metrics(&response)
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
        match toml_config
            .margin
            .and_then(|m| if m.is_complete() { Some(m) } else { None })
        {
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
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Build the per-package subtotal rows and the workspace total rows
/// that the `--totals` flag inserts into the human-readable table output.
///
/// Returns one [`TableCostReport`] row per metric listed in
/// [`TOTALS_METRICS`] for every package that contains at least one
/// successfully simulated function, plus one row per metric for the
/// workspace-wide total.
///
/// Aggregation rules (also documented in the table footer printed by
/// the Table output branch):
///
/// * Only rows with a measured `value` are summed; `value: None` rows
///   (failed simulations or `--check` failure stubs) are excluded.
/// * Metrics are summed independently — instructions vs. bytes have
///   different units, so no cross-metric totals are emitted.
/// * A package whose every function failed contributes no subtotal row
///   (an empty subtotal would look indistinguishable from a real
///   measurement and is omitted to prevent that confusion).
/// * Workspace totals are accumulated from the original measured rows
///   during the same iteration as subtotals, so a workspace total can
///   never accidentally double-count a subtotal row.
fn totals_for_table(reports: &[CostReport]) -> Vec<TableCostReport> {
    // Group measured rows by package, preserving first-seen order so
    // the output mirrors the simulation loop's traversal order. Rows
    // whose `value` is None are dropped here so they neither contribute
    // to subtotals nor appear in the table.
    let mut by_package: Vec<(String, Vec<&CostReport>)> = Vec::new();
    for r in reports {
        if r.value.is_none() {
            continue;
        }
        if let Some(entry) = by_package.iter_mut().find(|(n, _)| n == &r.package) {
            entry.1.push(r);
        } else {
            by_package.push((r.package.clone(), vec![r]));
        }
    }

    let mut out: Vec<TableCostReport> = Vec::new();
    let mut workspace_totals: HashMap<&'static str, u64> = HashMap::new();

    for (package, rows) in by_package {
        let mut subtotals: HashMap<&'static str, u64> = HashMap::new();
        for r in &rows {
            // Safe: we filtered `value.is_none()` above.
            let v = r.value.expect("value.is_none() filtered above") as u64;
            *subtotals.entry(r.metric).or_insert(0) += v;
            *workspace_totals.entry(r.metric).or_insert(0) += v;
        }
        for &metric in TOTALS_METRICS.iter() {
            if let Some(&total) = subtotals.get(&metric) {
                out.push(TableCostReport {
                    package: package.clone(),
                    function: TOTALS_SUBTOTAL_FUNCTION.to_string(),
                    metric,
                    value: format_with_commas_and_units(total, metric),
                });
            }
        }
    }

    for &metric in TOTALS_METRICS.iter() {
        if let Some(&total) = workspace_totals.get(&metric) {
            out.push(TableCostReport {
                package: TOTALS_WORKSPACE_PACKAGE.to_string(),
                function: TOTALS_WORKSPACE_FUNCTION.to_string(),
                metric,
                value: format_with_commas_and_units(total, metric),
            });
        }
    }

    out
}

fn main() -> anyhow::Result<()> {
    let CargoCli::BudgetReport(args) = CargoCli::parse();
    if args.init {
        let path = Path::new("budget.toml");
        if path.exists() && !args.force {
            anyhow::bail!("budget.toml already exists; use --force to overwrite it");
        }
        std::fs::write(path, BUDGET_TOML_TEMPLATE)?;
        return Ok(());
    }

    let config = std::fs::read_to_string("budget.toml")
        .ok()
        .and_then(|contents| toml::from_str::<BudgetToml>(&contents).ok())
        .unwrap_or_default();
    let network = args.network.as_deref().or(config.network.as_deref()).unwrap_or("testnet");
    let source = args.source.as_deref().or(config.source.as_deref()).unwrap_or("alice");
    let rpc_url = args.rpc_url.as_deref();
    let network_passphrase = args.network_passphrase.as_deref();
    let simulation_url = rpc_url.unwrap_or_else(|| match network {
        "testnet" => "https://soroban-testnet.stellar.org",
        "futurenet" => "https://rpc-futurenet.stellar.org",
        _ => "http://localhost:8000/soroban/rpc",
    });

    let metadata = MetadataCommand::new().exec()?;
    let mut reports = Vec::new();
    for package in metadata.packages {
        if package
            .targets
            .iter()
            .all(|target| !target.kind.iter().any(|kind| kind == "lib"))
        {
            continue;
        }
        let name = package.name.to_string();
        build_contract(&name)?;
        let wasm = Path::new(&wasm_path(&name));
        let contract_id = deploy_contract(wasm, source, network, rpc_url, network_passphrase)?;
        for function in exported_functions(wasm)? {
            if function.starts_with("__") {
                continue;
            }
            let function_config = config.functions.get(&function).cloned().unwrap_or_default();
            let (instructions, read_bytes, write_bytes) = invoke_and_simulate(
                &contract_id,
                source,
                network,
                &function,
                &function_config,
                simulation_url,
                rpc_url,
                network_passphrase,
            )?;
            reports.extend([
                CostReport { package: name.clone(), function: function.clone(), metric: "CPU Instructions", value: instructions },
                CostReport { package: name.clone(), function: function.clone(), metric: "Read Bytes", value: read_bytes },
                CostReport { package: name.clone(), function, metric: "Write Bytes", value: write_bytes },
            ]);
        }
    }
    if args.json {
        println!("{}", serde_json::to_string_pretty(&reports)?);
    } else {
        for report in reports {
            println!("{}::{} [{}] {}", report.package, report.function, report.metric, report.value);
        }
    }
    Ok(())
}

mod module_2;
mod module_3;
mod module_4;
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
mod module_32;

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parses_custom_rpc_options() {
        let cli = CargoCli::try_parse_from([
            "cargo",
            "budget-report",
            "--rpc-url",
            "http://localhost:8000/soroban/rpc",
            "--network-passphrase",
            "Standalone Network ; February 2025",
        ])
        .expect("custom RPC options should parse");
        let CargoCli::BudgetReport(args) = cli;
        assert_eq!(args.rpc_url.as_deref(), Some("http://localhost:8000/soroban/rpc"));
        assert_eq!(args.network_passphrase.as_deref(), Some("Standalone Network ; February 2025"));
    }

    #[test]
    fn rpc_url_requires_network_passphrase() {
        let result = CargoCli::try_parse_from([
            "cargo",
            "budget-report",
            "--rpc-url",
            "http://localhost:8000/soroban/rpc",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn custom_network_arguments_override_network_alias() {
        let args = build_invoke_args(
            "CABC",
            "alice",
            "testnet",
            "ping",
            &[],
            Some("http://localhost:8000/soroban/rpc"),
            Some("Standalone Network"),
        );
        assert!(args.contains(&"--rpc-url".to_string()));
        assert!(args.contains(&"--network-passphrase".to_string()));
        assert!(!args.contains(&"--network".to_string()));
    }

    // ── JSON serialization tests ────────────────────────────────────────

    /// Helper to serialize a slice of CostReport to a pretty-printed JSON
    /// string, matching the `--json` output path.
    fn reports_to_json(reports: &[CostReport]) -> String {
        serde_json::to_string_pretty(reports).unwrap()
    }

    #[test]
    fn json_output_without_check_has_package_function_metric_value() {
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
        let json = reports_to_json(&reports);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 2);

        assert_eq!(arr[0]["package"], "my-contract");
        assert_eq!(arr[0]["function"], "do_work");
        assert_eq!(arr[0]["metric"], "CPU Instructions");
        assert_eq!(arr[0]["value"], 1_000_000);
        assert!(arr[0].get("limit").is_none());
        assert!(arr[0].get("pass").is_none());

        assert_eq!(arr[1]["package"], "my-contract");
        assert_eq!(arr[1]["function"], "do_work");
        assert_eq!(arr[1]["metric"], "Read Bytes");
        assert_eq!(arr[1]["value"], 2_048);
        assert!(arr[1].get("limit").is_none());
        assert!(arr[1].get("pass").is_none());
    }

    #[test]
    fn json_output_with_check_includes_limit_and_pass() {
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
        let json = reports_to_json(&reports);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 2);

        assert_eq!(arr[0]["package"], "my-contract");
        assert_eq!(arr[0]["function"], "do_work");
        assert_eq!(arr[0]["metric"], "CPU Instructions");
        assert_eq!(arr[0]["value"], 1_000_000);
        assert_eq!(arr[0]["limit"], 5_000_000);
        assert_eq!(arr[0]["pass"], true);

        assert_eq!(arr[1]["package"], "my-contract");
        assert_eq!(arr[1]["function"], "do_work");
        assert_eq!(arr[1]["metric"], "Write Bytes");
        assert_eq!(arr[1]["value"], 4_096);
        assert_eq!(arr[1]["limit"], 1_000);
        assert_eq!(arr[1]["pass"], false);
    }

    #[test]
    fn json_output_without_check_excludes_null_values() {
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
        let json = reports_to_json(&reports);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 2);

        // The CPU Instructions entry has value: None; with
        // skip_serializing_if, "value" is absent from the JSON object.
        assert!(arr[0].get("value").is_none());
        // But the entry itself is still present in the array.
        assert_eq!(arr[0]["metric"], "CPU Instructions");

        assert_eq!(arr[1]["metric"], "Read Bytes");
        assert_eq!(arr[1]["value"], 2_048);
    }

    #[test]
    fn json_output_with_check_includes_simulation_failures() {
        let reports = vec![CostReport {
            package: "my-contract".to_string(),
            function: "do_work".to_string(),
            metric: "CPU Instructions",
            value: None,
            limit: Some(5_000_000),
            pass: Some(false),
        }];
        let json = reports_to_json(&reports);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 1);

        assert_eq!(arr[0]["package"], "my-contract");
        assert_eq!(arr[0]["function"], "do_work");
        assert_eq!(arr[0]["metric"], "CPU Instructions");
        assert!(arr[0].get("value").is_none());
        assert_eq!(arr[0]["limit"], 5_000_000);
        assert_eq!(arr[0]["pass"], false);
    }

    #[test]
    fn json_output_empty_reports_produces_empty_array() {
        let reports: Vec<CostReport> = vec![];
        let json = reports_to_json(&reports);
        assert_eq!(json, "[]");
    }

    #[test]
    fn json_output_with_all_metric_types() {
        let reports = vec![
            CostReport {
                package: "pkg".to_string(),
                function: "f".to_string(),
                metric: "CPU Instructions",
                value: Some(1_000_000),
                limit: None,
                pass: None,
            },
            CostReport {
                package: "pkg".to_string(),
                function: "f".to_string(),
                metric: "Read Bytes",
                value: Some(2_048),
                limit: None,
                pass: None,
            },
            CostReport {
                package: "pkg".to_string(),
                function: "f".to_string(),
                metric: "Write Bytes",
                value: Some(4_096),
                limit: None,
                pass: None,
            },
            CostReport {
                package: "pkg".to_string(),
                function: "f".to_string(),
                metric: "WASM Bytes",
                value: Some(12_345),
                limit: None,
                pass: None,
            },
        ];
        let json = reports_to_json(&reports);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 4);

        let metrics: Vec<&str> = arr.iter().map(|r| r["metric"].as_str().unwrap()).collect();
        assert_eq!(
            metrics,
            vec![
                "CPU Instructions",
                "Read Bytes",
                "Write Bytes",
                "WASM Bytes"
            ]
        );
    }
}
