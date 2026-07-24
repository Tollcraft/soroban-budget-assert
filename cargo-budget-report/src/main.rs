use anyhow::{Context, Result};
use cargo_metadata::MetadataCommand;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
enum Metric {
    #[serde(rename = "CPU Instructions")]
    CpuInstructions,
    #[serde(rename = "Read Bytes")]
    ReadBytes,
    #[serde(rename = "Write Bytes")]
    WriteBytes,
}

impl std::fmt::Display for Metric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Metric::CpuInstructions => write!(f, "CPU Instructions"),
            Metric::ReadBytes => write!(f, "Read Bytes"),
            Metric::WriteBytes => write!(f, "Write Bytes"),
        }
    }
}

impl Metric {
    fn unit(&self) -> &'static str {
        match self {
            Metric::CpuInstructions => "inst.",
            Metric::ReadBytes | Metric::WriteBytes => "B",
        }
    }
}

#[derive(serde::Serialize)]
struct CostReport {
    package: String,
    function: String,
    metric: Metric,
    value: u32,
}

#[derive(Tabled)]
struct TableCostReport {
    package: String,
    function: String,
    metric: Metric,
    value: String,
}

fn format_with_commas_and_units(value: u32, metric: Metric) -> String {
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

    format!("{} {}", formatted, metric.unit())
}

fn extract_metrics(rpc_response: &serde_json::Value) -> Result<(u32, u32, u32)> {
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
                has_errors = true;
                eprintln!("Warning: RPC error for {}: {}", function, error);
                continue;
            }

            let (instructions, read_bytes, write_bytes) = extract_metrics(&rpc_resp)
                .context("Failed to extract metrics from RPC response")?;

            reports.push(CostReport {
                package: package.name.to_string(),
                function: function.clone(),
                metric: Metric::CpuInstructions,
                value: instructions,
            });
            reports.push(CostReport {
                package: package.name.to_string(),
                function: function.clone(),
                metric: Metric::ReadBytes,
                value: read_bytes,
            });
            reports.push(CostReport {
                package: package.name.to_string(),
                function: function.clone(),
                metric: Metric::WriteBytes,
                value: write_bytes,
            });
        }
    }

    if reports.is_empty() {
        eprintln!("No successful simulations to report.");
        if has_errors {
            std::process::exit(1);
        }
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

    if has_errors {
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
        .expect("failed to parse malformed fixture JSON");

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

    // --- Metric enum tests ---

    #[test]
    fn metric_display_returns_correct_names() {
        assert_eq!(Metric::CpuInstructions.to_string(), "CPU Instructions");
        assert_eq!(Metric::ReadBytes.to_string(), "Read Bytes");
        assert_eq!(Metric::WriteBytes.to_string(), "Write Bytes");
    }

    #[test]
    fn metric_unit_returns_correct_suffixes() {
        assert_eq!(Metric::CpuInstructions.unit(), "inst.");
        assert_eq!(Metric::ReadBytes.unit(), "B");
        assert_eq!(Metric::WriteBytes.unit(), "B");
    }

    #[test]
    fn format_with_commas_and_units_uses_metric_type() {
        assert_eq!(
            format_with_commas_and_units(1_000_000, Metric::CpuInstructions),
            "1,000,000 inst."
        );
        assert_eq!(
            format_with_commas_and_units(2_048, Metric::ReadBytes),
            "2,048 B"
        );
        assert_eq!(
            format_with_commas_and_units(4_096, Metric::WriteBytes),
            "4,096 B"
        );
    }

    #[test]
    fn json_serialization_preserves_expected_keys() {
        let reports = vec![
            CostReport {
                package: "amm-pool-contract".to_string(),
                function: "do_expensive_work".to_string(),
                metric: Metric::CpuInstructions,
                value: 1_000_000,
            },
            CostReport {
                package: "amm-pool-contract".to_string(),
                function: "do_expensive_work".to_string(),
                metric: Metric::ReadBytes,
                value: 2_048,
            },
            CostReport {
                package: "amm-pool-contract".to_string(),
                function: "do_expensive_work".to_string(),
                metric: Metric::WriteBytes,
                value: 4_096,
            },
        ];
        let json = serde_json::to_string_pretty(&reports).unwrap();
        let expected = "[\n  {\n    \"package\": \"amm-pool-contract\",\n    \"function\": \"do_expensive_work\",\n    \"metric\": \"CPU Instructions\",\n    \"value\": 1000000\n  },\n  {\n    \"package\": \"amm-pool-contract\",\n    \"function\": \"do_expensive_work\",\n    \"metric\": \"Read Bytes\",\n    \"value\": 2048\n  },\n  {\n    \"package\": \"amm-pool-contract\",\n    \"function\": \"do_expensive_work\",\n    \"metric\": \"Write Bytes\",\n    \"value\": 4096\n  }\n]";
        assert_eq!(json, expected, "JSON serialization format must not change");
    }
}
