use anyhow::{Context, Result};
use cargo_metadata::MetadataCommand;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
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
}

#[derive(serde::Deserialize, Default, Debug)]
struct BudgetToml {
    network: Option<String>,
    source: Option<String>,
    #[serde(default)]
    functions: HashMap<String, FunctionConfig>,
}

#[derive(serde::Deserialize, Debug)]
struct Resources {
    instructions: u64,
    disk_read_bytes: u64,
    write_bytes: u64,
}

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
}

#[derive(serde::Serialize)]
struct CostReport {
    package: String,
    function: String,
    metric: &'static str,
    value: u32,
}

#[derive(Tabled)]
struct TableCostReport {
    package: String,
    function: String,
    metric: &'static str,
    value: String,
}

fn format_with_commas_and_units(value: u32, metric: &str) -> String {
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

    for package in metadata.packages {
        let is_cdylib = package
            .targets
            .iter()
            .any(|t| t.crate_types.iter().any(|c| c.to_string() == "cdylib"));
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
                eprintln!(
                    "Warning: Simulation failed for {}: {}",
                    function,
                    String::from_utf8_lossy(&invoke_output.stderr)
                );
                continue;
            }

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
                eprintln!("Warning: RPC error for {}: {}", function, error);
                continue;
            }

            let tx_data_b64 = rpc_resp["result"]["transactionData"]
                .as_str()
                .context("No transactionData found in simulateTransaction response.")?;

            let mut decode_cmd = Command::new("stellar")
                .args(["xdr", "decode", "--type", "SorobanTransactionData"])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .spawn()
                .context("failed to execute stellar xdr decode")?;

            {
                let stdin = decode_cmd.stdin.as_mut().context("Failed to open stdin")?;
                stdin
                    .write_all(tx_data_b64.as_bytes())
                    .context("Failed to write to stdin")?;
            }

            let decode_output = decode_cmd
                .wait_with_output()
                .context("Failed to read stdout")?;
            let json_str = String::from_utf8_lossy(&decode_output.stdout);
            let parsed_json: serde_json::Value =
                serde_json::from_str(&json_str).context("Failed to parse XDR JSON")?;

            let tx_data: TransactionData = serde_json::from_value(parsed_json)
                .context("Failed to deserialize transaction data")?;

            let instructions = tx_data.resources.instructions;
            let read_bytes = tx_data.resources.disk_read_bytes;
            let write_bytes = tx_data.resources.write_bytes;

            reports.push(CostReport {
                package: package.name.to_string(),
                function: function.clone(),
                metric: "CPU Instructions",
                value: instructions as u32,
            });
            reports.push(CostReport {
                package: package.name.to_string(),
                function: function.clone(),
                metric: "Read Bytes",
                value: read_bytes as u32,
            });
            reports.push(CostReport {
                package: package.name.to_string(),
                function: function.clone(),
                metric: "Write Bytes",
                value: write_bytes as u32,
            });
        }
    }

    if reports.is_empty() {
        eprintln!("No successful simulations to report.");
        return Ok(());
    }

    if args.json {
        let json_output =
            serde_json::to_string_pretty(&reports).context("Failed to serialize report to JSON")?;
        println!("{}", json_output);
    } else {
        println!("\n=== WORKSPACE BUDGET REPORT ===");
        let table_reports: Vec<TableCostReport> = reports
            .into_iter()
            .map(|r| {
                let formatted = format_with_commas_and_units(r.value, r.metric);
                TableCostReport {
                    package: r.package,
                    function: r.function,
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
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_test_path() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is before UNIX_EPOCH")
            .as_nanos();
        path.push(format!("cargo_budget_report_test_{}.toml", nanos));
        path
    }

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
}
