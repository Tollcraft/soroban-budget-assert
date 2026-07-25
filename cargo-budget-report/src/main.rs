use anyhow::{Context, Result};
use cargo_metadata::MetadataCommand;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use stellar_xdr::curr::{Limits, ReadXdr, SorobanTransactionData};
use tabled::{Table, Tabled};
use wasmparser::Parser as WasmParser;

#[derive(Parser, Debug)]
#[command(name = "cargo", bin_name = "cargo")]
enum CargoCli {
    BudgetReport(BudgetReportArgs),
}

#[derive(Parser, Debug)]
struct BudgetReportArgs {
    #[arg(long)]
    network: Option<String>,

    #[arg(long)]
    source: Option<String>,

    #[arg(long, default_value_t = false)]
    json: bool,

    /// Enforce per-function limits declared in `budget.toml`.
    ///
    /// When set, each measured metric is compared against its configured
    /// `cpu_limit` / `read_limit` / `write_limit`. A missing limit means the
    /// metric is reported but **not** enforced. The process exits with a
    /// non-zero status when any limit is breached, or when a function that
    /// has a `budget.toml` entry fails to simulate. Functions that are not
    /// declared in `budget.toml` are reported only.
    #[arg(long, default_value_t = false)]
    check: bool,
}

#[derive(serde::Deserialize, Default, Debug)]
struct BudgetToml {
    network: Option<String>,
    source: Option<String>,
    #[serde(default)]
    functions: HashMap<String, FunctionConfig>,
}

#[allow(dead_code)]
#[derive(serde::Deserialize, Debug)]
struct Resources {
    instructions: u64,
    disk_read_bytes: u64,
    write_bytes: u64,
}

#[allow(dead_code)]
#[derive(serde::Deserialize, Debug)]
struct TransactionData {
    #[serde(alias = "resources")]
    resources: Resources,
}

impl TransactionData {
    #[cfg(test)]
    fn parse_json(json_str: &str) -> Result<Self> {
        let parsed_json: serde_json::Value =
            serde_json::from_str(json_str).context("Failed to parse JSON")?;
        serde_json::from_value(parsed_json).context("Failed to deserialize transaction data")
    }
}

#[derive(serde::Deserialize, Default, Debug)]
struct FunctionConfig {
    #[serde(default)]
    args: Vec<String>,
    /// Inclusive upper bound on the measured CPU `Instructions` metric. `None`
    /// means this metric is reported but not enforced by `--check`.
    #[serde(default)]
    cpu_limit: Option<u64>,
    #[serde(default)]
    read_limit: Option<u64>,
    #[serde(default)]
    write_limit: Option<u64>,
}

#[derive(Serialize)]
struct CostReport {
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

#[derive(Tabled)]
struct TableCostReport {
    package: String,
    function: String,
    metric: &'static str,
    value: String,
}

/// Returns the configured limit (if any) for the given metric name.
fn limit_for_metric(func_config: &FunctionConfig, metric: &str) -> Option<u64> {
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
fn evaluate_check(value: u32, limit: Option<u64>) -> (Option<u64>, Option<bool>) {
    match limit {
        Some(n) => (Some(n), Some(u64::from(value) <= n)),
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
fn emit_check_failure_entries(
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

fn format_with_commas_and_units(value: u64, metric: &str) -> String {
    let s = value.to_string();
    let mut result = String::new();
    let mut count = 0;
    for c in s.chars().rev() {
        if count == 3 {
            result.push(',');
            count = 0;
        }
        result.push(c);
        count += 1;
    }
    let formatted = result.chars().rev().collect::<String>();

    if metric.contains("Bytes") {
        format!("{} B", formatted)
    } else {
        format!("{} inst.", formatted)
    }
}

fn extract_metrics(rpc_response: &serde_json::Value) -> Result<(u32, u32, u32)> {
    if let Some(error) = rpc_response.get("error") {
        anyhow::bail!("{}", error);
    }

    let tx_data_b64 = rpc_response["result"]["transactionData"]
        .as_str()
        .context("No transactionData found in simulateTransaction response.")?;

    let tx_data = SorobanTransactionData::from_xdr_base64(tx_data_b64, Limits::none())
        .context("Failed to decode SorobanTransactionData from base64 XDR")?;

    Ok((
        tx_data.resources.instructions,
        tx_data.resources.read_bytes,
        tx_data.resources.write_bytes,
    ))
}

fn load_budget_toml<P: AsRef<Path>>(path: P) -> Result<BudgetToml> {
    match std::fs::read_to_string(&path) {
        Ok(contents) => toml::from_str(&contents)
            .map_err(|err| anyhow::anyhow!("failed to parse {}: {}", path.as_ref().display(), err)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(BudgetToml::default()),
        Err(err) => Err(err).with_context(|| format!("failed to read {}", path.as_ref().display())),
    }
}

fn main() -> Result<()> {
    let CargoCli::BudgetReport(args) = CargoCli::parse();

    let toml_config = load_budget_toml("budget.toml")?;

    let network = args
        .network
        .or(toml_config.network)
        .context("missing --network or budget.toml network field")?;
    let source = args
        .source
        .or(toml_config.source)
        .context("missing --source or budget.toml source field")?;

    eprintln!("Discovering workspace members...");
    let metadata = MetadataCommand::new()
        .no_deps()
        .exec()
        .context("failed to execute cargo metadata")?;

    let mut reports = Vec::new();
    let mut has_errors = false;
    let mut checks_failed = false;

    for package in metadata.packages {
        let is_cdylib = package
            .targets
            .iter()
            .any(|t| t.crate_types.iter().any(|c| *c == "cdylib"));
        if !is_cdylib {
            continue;
        }

        eprintln!("Building package '{}' for wasm32...", package.name);
        let build_status = Command::new("cargo")
            .args([
                "build",
                "-p",
                &package.name,
                "--target",
                "wasm32-unknown-unknown",
                "--release",
            ])
            .status()
            .context("failed to build package")?;

        if !build_status.success() {
            anyhow::bail!("Failed to build {}", package.name);
        }

        // Locate wasm
        let wasm_name = package.name.replace('-', "_");
        let wasm_path = metadata
            .target_directory
            .join("wasm32-unknown-unknown")
            .join("release")
            .join(format!("{}.wasm", wasm_name));

        if !wasm_path.exists() {
            eprintln!("Warning: WASM not found at {}", wasm_path);
            continue;
        }

        // Parse WASM exports
        let wasm_bytes = std::fs::read(&wasm_path).context("failed to read wasm file")?;
        let mut exported_fns = Vec::new();

        for payload in WasmParser::new(0).parse_all(&wasm_bytes) {
            if let wasmparser::Payload::ExportSection(s) = payload? {
                for export in s {
                    let export = export?;
                    if export.kind == wasmparser::ExternalKind::Func {
                        let name = export.name.to_string();
                        // Ignore internal and common exports
                        if !name.starts_with('_') && name != "memory" {
                            exported_fns.push(name);
                        }
                    }
                }
            }
        }

        if exported_fns.is_empty() {
            eprintln!("No exported functions found in {}", package.name);
            continue;
        }

        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "✔"])
                .template("{spinner:.green} Deploying contract {msg}...")
                .unwrap(),
        );
        spinner.set_message(package.name.to_string());
        spinner.enable_steady_tick(std::time::Duration::from_millis(100));

        let deploy_output = Command::new("stellar")
            .args([
                "contract",
                "deploy",
                "--wasm",
                wasm_path.as_str(),
                "--source",
                &source,
                "--network",
                &network,
            ])
            .output()
            .context("failed to execute stellar-cli deploy")?;

        spinner.finish_and_clear();

        if !deploy_output.status.success() {
            anyhow::bail!(
                "Failed to deploy {}. Ensure your source account is funded.\nError: {}",
                package.name,
                String::from_utf8_lossy(&deploy_output.stderr)
            );
        }

        let contract_id = String::from_utf8_lossy(&deploy_output.stdout)
            .trim()
            .to_string();
        eprintln!("Contract deployed at: {}", contract_id);

        for function in exported_fns {
            eprintln!("Simulating function '{}'...", function);

            let func_config = toml_config.functions.get(&function);
            let func_args = func_config.map(|c| c.args.clone()).unwrap_or_default();

            let mut invoke_args = vec![
                "contract".to_string(),
                "invoke".to_string(),
                "--id".to_string(),
                contract_id.clone(),
                "--source".to_string(),
                source.clone(),
                "--network".to_string(),
                network.clone(),
                "--build-only".to_string(),
                "--".to_string(),
                function.clone(),
            ];
            invoke_args.extend(func_args);

            let invoke_output = Command::new("stellar")
                .args(&invoke_args)
                .output()
                .context("failed to execute stellar-cli invoke")?;

            if !invoke_output.status.success() {
                has_errors = true;
                eprintln!(
                    "Warning: Simulation failed for {}: {}",
                    function,
                    String::from_utf8_lossy(&invoke_output.stderr)
                );
                if let (true, Some(fc)) = (args.check, func_config) {
                    // A configured function that won't simulate cannot satisfy
                    // any of its declared limits; record this as a check failure
                    // even if no `*_limit` is set on this row of budget.toml.
                    checks_failed = true;
                    emit_check_failure_entries(&mut reports, &package.name, &function, fc);
                }
            } else {
                let b64_xdr = String::from_utf8_lossy(&invoke_output.stdout)
                    .trim()
                    .to_string();

                let rpc_payload = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "simulateTransaction",
                    "params": {
                        "transaction": b64_xdr
                    }
                });

                use std::io::Write;
                let mut curl = Command::new("curl")
                    .args([
                        "-s",
                        "-X",
                        "POST",
                        "-H",
                        "Content-Type: application/json",
                        "-d",
                        "@-",
                        "https://soroban-testnet.stellar.org:443",
                    ])
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .spawn()
                    .context("failed to execute curl")?;

                {
                    let stdin = curl.stdin.as_mut().context("Failed to open stdin")?;
                    stdin
                        .write_all(rpc_payload.to_string().as_bytes())
                        .context("Failed to write to stdin")?;
                }

                let curl_output = curl
                    .wait_with_output()
                    .context("Failed to read curl output")?;
                let rpc_resp: serde_json::Value = serde_json::from_slice(&curl_output.stdout)
                    .context("Failed to parse RPC response")?;

                if let Some(error) = rpc_resp.get("error") {
                    has_errors = true;
                    eprintln!("Warning: RPC error for {}: {}", function, error);
                    if let (true, Some(fc)) = (args.check, func_config) {
                        checks_failed = true;
                        emit_check_failure_entries(&mut reports, &package.name, &function, fc);
                    }
                } else {
                    match extract_metrics(&rpc_resp) {
                        Ok((instructions, read_bytes, write_bytes)) => {
                            // Build three CostReport entries for this function.
                            // In --check mode, attach the configured limit and
                            // pass/fail to each entry.
                            for (metric, value) in [
                                ("CPU Instructions", instructions),
                                ("Read Bytes", read_bytes),
                                ("Write Bytes", write_bytes),
                            ] {
                                let limit = func_config.and_then(|c| limit_for_metric(c, metric));
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
                        }
                        Err(err) => {
                            has_errors = true;
                            eprintln!(
                                "Warning: Failed to extract metrics for {}: {:#}",
                                function, err
                            );
                            if let (true, Some(fc)) = (args.check, func_config) {
                                // A configured function whose sim produced an
                                // extractable-but-unparseable response cannot
                                // satisfy any of its declared limits.
                                checks_failed = true;
                                emit_check_failure_entries(
                                    &mut reports,
                                    &package.name,
                                    &function,
                                    fc,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    if reports.is_empty() {
        eprintln!("No successful simulations to report.");
        if has_errors || (args.check && checks_failed) {
            std::process::exit(1);
        }
        return Ok(());
    }

    if args.json {
        let json_output =
            serde_json::to_string_pretty(&reports).context("Failed to serialize report to JSON")?;
        println!("{}", json_output);
    } else {
        // The plain text report path is preserved byte-for-byte when
        // `--check` is not passed: only entries with a measured value are
        // rendered in the table, and summary text is unchanged.
        println!("\n=== WORKSPACE BUDGET REPORT ===");
        let table_reports: Vec<TableCostReport> = reports
            .iter()
            .filter(|r| r.value.is_some())
            .map(|r| {
                let value = r.value.unwrap_or(0);
                let formatted = format_with_commas_and_units(u64::from(value), r.metric);
                TableCostReport {
                    package: r.package.clone(),
                    function: r.function.clone(),
                    metric: r.metric,
                    value: formatted,
                }
            })
            .collect();
        let table = Table::new(table_reports).to_string();
        println!("{}", table);
        println!("\nSummary: The values above are simulated resource amounts, not fees. They are three of the inputs to the non-refundable resource fee.");
        println!("* Not measured: transaction size, ledger footprint entry counts, refundable fees (rent, events, return value), the inclusion fee, and therefore the total fee charged.");
        println!("* Note: These are simulated numbers on testnet and may vary slightly depending on ledger state.");
        println!("* See the \"Measurement scope\" section of the Tool Reference for what to use instead when you need those figures.");

        if args.check {
            println!("\n=== BUDGET CHECKS ===");
            let mut passed: usize = 0;
            let mut failed: usize = 0;
            for r in &reports {
                let Some(pass) = r.pass else {
                    continue;
                };
                let status = if pass { "PASS" } else { "FAIL" };
                let value_str = match r.value {
                    Some(v) => format_with_commas_and_units(u64::from(v), r.metric),
                    None => "<simulation failed>".to_string(),
                };
                let limit_str = r
                    .limit
                    .map(|n| {
                        // Limits wider than u32::MAX are not representable in
                        // the table's units, but anything close to the
                        // practical ceiling formats fine.
                        let v = u32::try_from(n).unwrap_or(u32::MAX);
                        format_with_commas_and_units(u64::from(v), r.metric)
                    })
                    .unwrap_or_else(|| "-".to_string());
                println!(
                    "{}::{} [{}] value={} limit={} {}",
                    r.package, r.function, r.metric, value_str, limit_str, status
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

    if has_errors || (args.check && checks_failed) {
        std::process::exit(1);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use stellar_xdr::curr::WriteXdr;

    fn unique_test_path() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is before UNIX_EPOCH")
            .as_nanos();
        path.push(format!("cargo_budget_report_test_{}.toml", nanos));
        path
    }

    // --- Metric extraction tests ---

    const FIXTURE_INSTRUCTIONS: u32 = 1_000_000;
    const FIXTURE_READ_BYTES: u32 = 2_048;
    const FIXTURE_WRITE_BYTES: u32 = 4_096;
    const FIXTURE_RESOURCE_FEE: i64 = 0;

    fn make_fixture_tx_data() -> SorobanTransactionData {
        use stellar_xdr::curr::{ExtensionPoint, LedgerFootprint, VecM};
        SorobanTransactionData {
            ext: ExtensionPoint::V0,
            resources: stellar_xdr::curr::SorobanResources {
                footprint: LedgerFootprint {
                    read_only: VecM::default(),
                    read_write: VecM::default(),
                },
                instructions: FIXTURE_INSTRUCTIONS,
                read_bytes: FIXTURE_READ_BYTES,
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
        let err = format!("{:#}", result.as_ref().unwrap_err());
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

        assert!(err_text.contains("failed to parse"));
        assert!(err_text.contains("line") || err_text.contains("Line"));
        assert!(err_text.contains("column") || err_text.contains("Column"));
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
        let err_msg = format!("{:#}", result.as_ref().unwrap_err());
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
        let err_msg = format!("{:#}", result.as_ref().unwrap_err());
        assert!(
            err_msg.contains("invalid type") || err_msg.contains("not-a-number"),
            "Error should mention type mismatch, got: {}",
            err_msg
        );
    }

    // --- budget.toml limit parsing tests ---

    #[test]
    fn function_config_parses_with_all_limits() {
        let path = unique_test_path();
        fs::write(
            &path,
            r#"
[functions.do_expensive_work]
args = ["--n", "10000"]
cpu_limit = 2000000
read_limit = 5000
write_limit = 1000
"#,
        )
        .expect("failed to write budget.toml");

        let config = load_budget_toml(&path).expect("budget.toml should parse");
        let fc = config
            .functions
            .get("do_expensive_work")
            .expect("function should be present");
        assert_eq!(fc.args, vec!["--n", "10000"]);
        assert_eq!(fc.cpu_limit, Some(2_000_000));
        assert_eq!(fc.read_limit, Some(5_000));
        assert_eq!(fc.write_limit, Some(1_000));
    }

    #[test]
    fn function_config_parses_with_partial_limits() {
        let path = unique_test_path();
        fs::write(
            &path,
            r#"
[functions.do_expensive_work]
cpu_limit = 1000
"#,
        )
        .expect("failed to write budget.toml");

        let config = load_budget_toml(&path).expect("budget.toml should parse");
        let fc = config
            .functions
            .get("do_expensive_work")
            .expect("function should be present");
        assert!(fc.args.is_empty());
        assert_eq!(fc.cpu_limit, Some(1_000));
        assert_eq!(fc.read_limit, None);
        assert_eq!(fc.write_limit, None);
    }

    #[test]
    fn function_config_parses_without_limits() {
        let path = unique_test_path();
        fs::write(
            &path,
            r#"
[functions.do_expensive_work]
args = ["--n", "10000"]
"#,
        )
        .expect("failed to write budget.toml");

        let config = load_budget_toml(&path).expect("budget.toml should parse");
        let fc = config
            .functions
            .get("do_expensive_work")
            .expect("function should be present");
        assert_eq!(fc.cpu_limit, None);
        assert_eq!(fc.read_limit, None);
        assert_eq!(fc.write_limit, None);
    }

    // --- evaluate_check tests ---

    #[test]
    fn evaluate_check_no_limit_returns_none_pair() {
        assert_eq!(evaluate_check(123, None), (None, None));
    }

    #[test]
    fn evaluate_check_within_limit_passes() {
        assert_eq!(evaluate_check(500, Some(1_000)), (Some(1_000), Some(true)));
    }

    #[test]
    fn evaluate_check_exactly_at_limit_passes() {
        // u64::from(value) <= n is the inclusive comparison
        assert_eq!(
            evaluate_check(1_000, Some(1_000)),
            (Some(1_000), Some(true))
        );
    }

    #[test]
    fn evaluate_check_over_limit_fails() {
        assert_eq!(
            evaluate_check(2_000, Some(1_000)),
            (Some(1_000), Some(false))
        );
    }

    #[test]
    fn evaluate_check_large_limit_does_not_overflow() {
        // Limits are u64; values are u32 so widening is safe.
        assert_eq!(
            evaluate_check(u32::MAX, Some(u64::from(u32::MAX))),
            (Some(u64::from(u32::MAX)), Some(true))
        );
    }

    // --- limit_for_metric tests ---

    fn configured_function(
        cpu: Option<u64>,
        read: Option<u64>,
        write: Option<u64>,
    ) -> FunctionConfig {
        FunctionConfig {
            args: vec![],
            cpu_limit: cpu,
            read_limit: read,
            write_limit: write,
        }
    }

    #[test]
    fn limit_for_metric_returns_configured_limits() {
        let fc = configured_function(Some(100), Some(200), Some(300));
        assert_eq!(limit_for_metric(&fc, "CPU Instructions"), Some(100));
        assert_eq!(limit_for_metric(&fc, "Read Bytes"), Some(200));
        assert_eq!(limit_for_metric(&fc, "Write Bytes"), Some(300));
    }

    #[test]
    fn limit_for_metric_returns_none_for_unconfigured_metrics() {
        let fc = configured_function(Some(100), None, None);
        assert_eq!(limit_for_metric(&fc, "CPU Instructions"), Some(100));
        assert_eq!(limit_for_metric(&fc, "Read Bytes"), None);
        assert_eq!(limit_for_metric(&fc, "Write Bytes"), None);
    }

    #[test]
    fn limit_for_metric_returns_none_for_unknown_metric_name() {
        let fc = configured_function(Some(100), Some(200), Some(300));
        assert_eq!(limit_for_metric(&fc, "Something Else"), None);
    }

    // --- CostReport serialization tests ---

    #[test]
    fn cost_report_json_omits_limit_and_pass_when_unset() {
        // Mirrors the plain (no --check) serialization shape: limit/pass/value
        // are all None-equivalent or absent so existing JSON consumers see
        // byte-for-byte identical output.
        let report = CostReport {
            package: "amm-pool-contract".to_string(),
            function: "do_expensive_work".to_string(),
            metric: "CPU Instructions",
            value: Some(756_678),
            limit: None,
            pass: None,
        };
        let s = serde_json::to_string(&report).expect("serialization should succeed");
        assert!(s.contains("\"value\":756678"));
        assert!(!s.contains("\"limit\""));
        assert!(!s.contains("\"pass\""));
    }

    #[test]
    fn cost_report_json_omits_value_when_simulation_failed() {
        // In --check mode, a configured function with a failed sim emits an
        // entry that has no `value` but carries an explicit pass=false (and
        // a configured limit when one exists).
        let report = CostReport {
            package: "amm-pool-contract".to_string(),
            function: "do_expensive_work".to_string(),
            metric: "CPU Instructions",
            value: None,
            limit: Some(2_000_000),
            pass: Some(false),
        };
        let s = serde_json::to_string(&report).expect("serialization should succeed");
        assert!(!s.contains("\"value\""));
        assert!(s.contains("\"limit\":2000000"));
        assert!(s.contains("\"pass\":false"));
    }

    #[test]
    fn cost_report_json_omits_value_and_limit_when_simulation_failed_with_no_limit() {
        // Even when no limit is configured for a metric, a configured
        // function whose sim failed still emits an entry — but with no
        // limit so consumers can tell the metric was not enforced.
        let report = CostReport {
            package: "amm-pool-contract".to_string(),
            function: "do_expensive_work".to_string(),
            metric: "CPU Instructions",
            value: None,
            limit: None,
            pass: Some(false),
        };
        let s = serde_json::to_string(&report).expect("serialization should succeed");
        assert!(!s.contains("\"value\""));
        assert!(!s.contains("\"limit\""));
        assert!(s.contains("\"pass\":false"));
    }

    #[test]
    fn cost_report_json_includes_limit_and_pass_when_configured() {
        let report = CostReport {
            package: "amm-pool-contract".to_string(),
            function: "do_expensive_work".to_string(),
            metric: "CPU Instructions",
            value: Some(756_678),
            limit: Some(2_000_000),
            pass: Some(true),
        };
        let s = serde_json::to_string(&report).expect("serialization should succeed");
        assert!(s.contains("\"value\":756678"));
        assert!(s.contains("\"limit\":2000000"));
        assert!(s.contains("\"pass\":true"));
    }

    // --- format_with_commas_and_units regression ---

    #[test]
    fn format_with_commas_and_units_preserves_commas_and_unit_suffix() {
        // The existing function is used for the table; preserve the historical
        // formatting (commas every three digits, " B" / " inst." suffix) for
        // values that fit in u32.
        assert_eq!(
            format_with_commas_and_units(1_000_000, "CPU Instructions"),
            "1,000,000 inst."
        );
        assert_eq!(format_with_commas_and_units(2_048, "Read Bytes"), "2,048 B");
        assert_eq!(format_with_commas_and_units(0, "Write Bytes"), "0 B");
    }

    // --- emit_check_failure_entries tests ---

    fn collect_failure_entries(func_config: &FunctionConfig) -> Vec<CostReport> {
        let mut reports = Vec::new();
        emit_check_failure_entries(
            &mut reports,
            "amm-pool-contract",
            "do_expensive_work",
            func_config,
        );
        reports
    }

    #[test]
    fn emit_check_failure_entries_emits_stub_per_metric_with_all_limits() {
        let fc = configured_function(Some(2_000_000), Some(5_000), Some(1_000));
        let reports = collect_failure_entries(&fc);
        assert_eq!(reports.len(), 3);
        let names: Vec<&'static str> = reports.iter().map(|r| r.metric).collect();
        assert_eq!(names, ["CPU Instructions", "Read Bytes", "Write Bytes"]);
        for r in &reports {
            assert!(r.value.is_none(), "no value for failed sim entries");
            assert!(r.limit.is_some(), "limit should be passed through");
            assert_eq!(r.pass, Some(false));
            assert_eq!(r.package, "amm-pool-contract");
            assert_eq!(r.function, "do_expensive_work");
        }
        assert_eq!(reports[0].limit, Some(2_000_000));
        assert_eq!(reports[1].limit, Some(5_000));
        assert_eq!(reports[2].limit, Some(1_000));
    }

    #[test]
    fn emit_check_failure_entries_emits_only_configured_metric_when_only_cpu_limit_set() {
        let fc = configured_function(Some(2_000_000), None, None);
        let reports = collect_failure_entries(&fc);
        assert_eq!(reports.len(), 3);
        // Every metric gets a stub, but only the one with a configured limit carries it.
        assert!(reports[0].limit.is_some());
        assert!(reports[1].limit.is_none());
        assert!(reports[2].limit.is_none());
        for r in &reports {
            assert_eq!(r.pass, Some(false));
            assert!(r.value.is_none());
        }
    }

    #[test]
    fn emit_check_failure_entries_still_emits_three_stubs_when_no_limits_set() {
        // Even with no per-metric limits, a failed sim of a configured
        // function still produces three stub entries so `--check --json`
        // consumers see what went wrong (with `value` and `limit` omitted,
        // `pass: false`).
        let fc = configured_function(None, None, None);
        let reports = collect_failure_entries(&fc);
        assert_eq!(reports.len(), 3);
        for r in &reports {
            assert!(r.value.is_none());
            assert!(r.limit.is_none());
            assert_eq!(r.pass, Some(false));
        }
    }
}
