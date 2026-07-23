use anyhow::{Context, Result};
use cargo_metadata::MetadataCommand;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Write;
use std::process::Command;
use tabled::{Table, Tabled};
use wasmparser::Parser as WasmParser;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(name = "cargo", bin_name = "cargo")]
pub enum CargoCli {
    BudgetReport(BudgetReportArgs),
}

#[derive(Parser, Debug)]
pub struct BudgetReportArgs {
    #[arg(long)]
    pub network: Option<String>,

    #[arg(long)]
    pub source: Option<String>,

    #[arg(long, default_value_t = false)]
    pub json: bool,
}

#[derive(Deserialize, Default, Debug)]
pub struct BudgetToml {
    pub network: Option<String>,
    pub source: Option<String>,
    #[serde(default)]
    pub functions: HashMap<String, FunctionConfig>,
}

#[derive(Deserialize, Default, Debug)]
pub struct FunctionConfig {
    #[serde(default)]
    pub args: Vec<String>,
}

pub fn resolve_config(
    args: &BudgetReportArgs,
    toml_content: Option<&str>,
) -> Result<(String, String, HashMap<String, Vec<String>>)> {
    let toml_config: BudgetToml = toml_content
        .and_then(|s| toml::from_str(s).ok())
        .unwrap_or_default();

    let network = args
        .network
        .clone()
        .or(toml_config.network)
        .context("missing --network or budget.toml network field")?;
    let source = args
        .source
        .clone()
        .or(toml_config.source)
        .context("missing --source or budget.toml source field")?;

    let func_args: HashMap<String, Vec<String>> = toml_config
        .functions
        .into_iter()
        .map(|(k, v)| (k, v.args))
        .collect();

    Ok((network, source, func_args))
}

// ---------------------------------------------------------------------------
// WASM export parsing
// ---------------------------------------------------------------------------

pub fn parse_exported_functions(wasm_bytes: &[u8]) -> Result<Vec<String>> {
    let mut functions = Vec::new();
    for payload in WasmParser::new(0).parse_all(wasm_bytes) {
        if let wasmparser::Payload::ExportSection(s) = payload? {
            for export in s {
                let export = export?;
                if export.kind == wasmparser::ExternalKind::Func {
                    let name = export.name.to_string();
                    if !name.starts_with('_') && name != "memory" {
                        functions.push(name);
                    }
                }
            }
        }
    }
    Ok(functions)
}

// ---------------------------------------------------------------------------
// Simulation response parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct CostMetrics {
    pub instructions: u32,
    pub read_bytes: u32,
    pub write_bytes: u32,
}

pub fn extract_tx_data_from_rpc(rpc_response: &serde_json::Value) -> Result<&str> {
    if let Some(error) = rpc_response.get("error") {
        anyhow::bail!("RPC error: {}", error);
    }
    rpc_response["result"]["transactionData"]
        .as_str()
        .context("No transactionData found in simulateTransaction response.")
}

pub fn parse_costs_from_decoded_xdr(decoded: &serde_json::Value) -> CostMetrics {
    CostMetrics {
        instructions: decoded["resources"]["instructions"].as_u64().unwrap_or(0) as u32,
        read_bytes: decoded["resources"]["disk_read_bytes"]
            .as_u64()
            .unwrap_or(0) as u32,
        write_bytes: decoded["resources"]["write_bytes"].as_u64().unwrap_or(0) as u32,
    }
}

pub fn send_rpc_request(payload: &serde_json::Value) -> Result<serde_json::Value> {
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
            .write_all(payload.to_string().as_bytes())
            .context("Failed to write to stdin")?;
    }

    let output = curl
        .wait_with_output()
        .context("Failed to read curl output")?;

    serde_json::from_slice(&output.stdout).context("Failed to parse RPC response")
}

pub fn decode_xdr(b64: &str) -> Result<serde_json::Value> {
    let mut decode_cmd = Command::new("stellar")
        .args(["xdr", "decode", "--type", "SorobanTransactionData"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .context("failed to execute stellar xdr decode")?;

    {
        let stdin = decode_cmd.stdin.as_mut().context("Failed to open stdin")?;
        stdin
            .write_all(b64.as_bytes())
            .context("Failed to write to stdin")?;
    }

    let output = decode_cmd
        .wait_with_output()
        .context("Failed to read stdout")?;

    let json_str = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&json_str).context("Failed to parse XDR JSON")
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
pub struct CostReport {
    pub package: String,
    pub function: String,
    pub metric: &'static str,
    pub value: u32,
}

#[derive(Tabled)]
pub struct TableCostReport {
    pub package: String,
    pub function: String,
    pub metric: &'static str,
    pub value: String,
}

pub fn format_with_commas_and_units(value: u32, metric: &str) -> String {
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

pub fn build_table_reports(reports: &[CostReport]) -> String {
    let table_reports: Vec<TableCostReport> = reports
        .iter()
        .map(|r| {
            let formatted = format_with_commas_and_units(r.value, r.metric);
            TableCostReport {
                package: r.package.clone(),
                function: r.function.clone(),
                metric: r.metric,
                value: formatted,
            }
        })
        .collect();

    let mut output = String::from("\n=== WORKSPACE BUDGET REPORT ===\n");
    output.push_str(&Table::new(table_reports).to_string());
    output.push_str("\n\nSummary: The metrics above represent the total unrefundable network execution costs required to run your contract functions.");
    output.push_str("\n* Note: These are simulated numbers on testnet and may vary slightly depending on ledger state.\n");
    output
}

pub fn build_json_reports(reports: &[CostReport]) -> Result<String> {
    serde_json::to_string_pretty(reports).context("Failed to serialize report to JSON")
}

// ---------------------------------------------------------------------------
// Workspace discovery & build
// ---------------------------------------------------------------------------

pub fn discover_workspace() -> Result<cargo_metadata::Metadata> {
    eprintln!("Discovering workspace members...");
    MetadataCommand::new()
        .no_deps()
        .exec()
        .context("failed to execute cargo metadata")
}

pub fn build_package(name: &str) -> Result<()> {
    eprintln!("Building package '{name}' for wasm32...");
    let status = Command::new("cargo")
        .args([
            "build",
            "-p",
            name,
            "--target",
            "wasm32-unknown-unknown",
            "--release",
        ])
        .status()
        .context("failed to build package")?;

    if !status.success() {
        anyhow::bail!("Failed to build {name}");
    }
    Ok(())
}

pub fn find_wasm_path(
    metadata: &cargo_metadata::Metadata,
    package_name: &str,
) -> Option<std::path::PathBuf> {
    let wasm_name = package_name.replace('-', "_");
    let path = metadata
        .target_directory
        .join("wasm32-unknown-unknown")
        .join("release")
        .join(format!("{wasm_name}.wasm"))
        .into_std_path_buf();
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

pub fn deploy_contract(wasm_path: &std::path::Path, source: &str, network: &str) -> Result<String> {
    let output = Command::new("stellar")
        .args([
            "contract",
            "deploy",
            "--wasm",
            wasm_path.to_str().unwrap(),
            "--source",
            source,
            "--network",
            network,
        ])
        .output()
        .context("failed to execute stellar-cli deploy")?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to deploy contract. Ensure your source account is funded.\nError: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn simulate_function(
    contract_id: &str,
    function: &str,
    func_args: &[String],
    source: &str,
    network: &str,
) -> Result<String> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "✔"])
            .template("{spinner:.green} Simulating {msg}...")
            .unwrap(),
    );
    spinner.set_message(format!("{contract_id}/{function}"));
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

    let mut invoke_args: Vec<String> = vec![
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

    let output = Command::new("stellar")
        .args(&invoke_args)
        .output()
        .context("failed to execute stellar-cli invoke")?;

    spinner.finish_and_clear();

    if !output.status.success() {
        anyhow::bail!(
            "Simulation failed for {function}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // WASM export parsing tests
    // -----------------------------------------------------------------------

    fn make_wasm(exports: &[(&str, u8, u32)]) -> Vec<u8> {
        // Build a minimal valid WASM module with the given exports.
        // Each export: (name, kind, index)
        let type_count: u8 = 1;
        let func_count: u8 = exports
            .iter()
            .filter(|(_, kind, _)| *kind == 0)
            .map(|(_, _, idx)| *idx as u8)
            .max()
            .map(|m| m + 1)
            .unwrap_or(0);

        let mut wasm = Vec::new();
        // magic + version
        wasm.extend_from_slice(b"\x00asm\x01\x00\x00\x00");
        // Type section: 1 x () -> ()
        let type_content = vec![type_count, 0x60, 0x00, 0x00];
        wasm.push(0x01); // section id
        wasm.push(type_content.len() as u8); // section size
        wasm.extend_from_slice(&type_content);
        // Function section (only if there are function exports)
        if func_count > 0 {
            wasm.push(0x03); // section id
            wasm.push(1 + func_count); // section size (count + type_indices)
            wasm.push(func_count); // count
            wasm.extend(vec![0x00; func_count as usize]); // all type index 0
        }
        // Export section
        {
            let mut export_content = Vec::new();
            export_content.push(exports.len() as u8); // count
            for (name, kind, index) in exports {
                export_content.push(name.len() as u8);
                export_content.extend_from_slice(name.as_bytes());
                export_content.push(*kind);
                export_content.extend_from_slice(&index.to_le_bytes()[..1]);
            }
            wasm.push(0x07); // section id
            wasm.push(export_content.len() as u8); // section size
            wasm.extend_from_slice(&export_content);
        }
        // Code section (one empty body per function)
        if func_count > 0 {
            let body: Vec<u8> = std::iter::repeat([0x02u8, 0x00, 0x0b])
                .flatten()
                .take(func_count as usize * 3)
                .collect();
            let mut code_content = Vec::new();
            code_content.push(func_count); // count
            code_content.extend_from_slice(&body);
            wasm.push(0x0a); // section id
            wasm.push(code_content.len() as u8); // section size
            wasm.extend_from_slice(&code_content);
        }

        wasm
    }

    fn test_wasm() -> Vec<u8> {
        make_wasm(&[("foo", 0, 0), ("bar", 0, 1)])
    }

    fn test_wasm_with_filtered() -> Vec<u8> {
        make_wasm(&[("foo", 0, 0), ("_internal", 0, 1), ("memory", 2, 0)])
    }

    #[test]
    fn test_parse_exported_functions() {
        let fns = parse_exported_functions(&test_wasm()).unwrap();
        assert_eq!(fns, vec!["foo", "bar"]);
    }

    #[test]
    fn test_parse_exported_functions_filters_internal_and_memory() {
        let fns = parse_exported_functions(&test_wasm_with_filtered()).unwrap();
        assert_eq!(fns, vec!["foo"]);
    }

    #[test]
    fn test_parse_exported_functions_empty() {
        let empty_wasm = make_wasm(&[]);
        let fns = parse_exported_functions(&empty_wasm).unwrap();
        assert!(fns.is_empty());
    }

    // -----------------------------------------------------------------------
    // Config resolution tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_config_cli_overrides_toml() {
        let args = BudgetReportArgs {
            network: Some("mainnet".into()),
            source: Some("bob".into()),
            json: false,
        };
        let toml = r#"
network = "testnet"
source = "alice"
"#;
        let (network, source, _) = resolve_config(&args, Some(toml)).unwrap();
        assert_eq!(network, "mainnet");
        assert_eq!(source, "bob");
    }

    #[test]
    fn test_resolve_config_toml_fallback() {
        let args = BudgetReportArgs {
            network: None,
            source: None,
            json: false,
        };
        let toml = r#"
network = "testnet"
source = "alice"
"#;
        let (network, source, _) = resolve_config(&args, Some(toml)).unwrap();
        assert_eq!(network, "testnet");
        assert_eq!(source, "alice");
    }

    #[test]
    fn test_resolve_config_missing_both_errors() {
        let args = BudgetReportArgs {
            network: None,
            source: None,
            json: false,
        };
        let err = resolve_config(&args, None).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("--network") || msg.contains("network"));
    }

    #[test]
    fn test_resolve_config_missing_network_errors() {
        let args = BudgetReportArgs {
            network: None,
            source: Some("alice".into()),
            json: false,
        };
        let toml = r#"source = "bob""#;
        let err = resolve_config(&args, Some(toml)).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("network"));
    }

    #[test]
    fn test_resolve_config_function_args() {
        let args = BudgetReportArgs {
            network: Some("testnet".into()),
            source: Some("alice".into()),
            json: false,
        };
        let toml = r#"
network = "testnet"
source = "alice"
[functions.do_work]
args = ["--n", "100"]
"#;
        let (_, _, func_args) = resolve_config(&args, Some(toml)).unwrap();
        assert_eq!(
            func_args.get("do_work"),
            Some(&vec!["--n".into(), "100".into()])
        );
    }

    // -----------------------------------------------------------------------
    // RPC response parsing tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_tx_data_from_rpc_success() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "transactionData": "AAAAAE5PVFJFQUw="
            }
        }"#,
        )
        .unwrap();
        let tx_data = extract_tx_data_from_rpc(&json).unwrap();
        assert_eq!(tx_data, "AAAAAE5PVFJFQUw=");
    }

    #[test]
    fn test_extract_tx_data_from_rpc_error() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32601,
                "message": "method not found"
            }
        }"#,
        )
        .unwrap();
        let err = extract_tx_data_from_rpc(&json).unwrap_err();
        assert!(format!("{err:#}").contains("RPC error"));
    }

    #[test]
    fn test_extract_tx_data_from_rpc_missing_result() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "result": {}
        }"#,
        )
        .unwrap();
        let err = extract_tx_data_from_rpc(&json).unwrap_err();
        assert!(format!("{err:#}").contains("transactionData"));
    }

    #[test]
    fn test_parse_costs_from_decoded_xdr() {
        let decoded: serde_json::Value = serde_json::from_str(
            r#"{
            "resources": {
                "footprint": {"read_write": [], "read_only": []},
                "instructions": 123456,
                "disk_read_bytes": 78901,
                "write_bytes": 23456
            },
            "resource_fee": 100
        }"#,
        )
        .unwrap();
        let costs = parse_costs_from_decoded_xdr(&decoded);
        assert_eq!(costs.instructions, 123456);
        assert_eq!(costs.read_bytes, 78901);
        assert_eq!(costs.write_bytes, 23456);
    }

    #[test]
    fn test_parse_costs_from_decoded_xdr_missing_fields_defaults_zero() {
        let decoded: serde_json::Value = serde_json::from_str(r#"{}"#).unwrap();
        let costs = parse_costs_from_decoded_xdr(&decoded);
        assert_eq!(costs.instructions, 0);
        assert_eq!(costs.read_bytes, 0);
        assert_eq!(costs.write_bytes, 0);
    }

    // -----------------------------------------------------------------------
    // Report formatting tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_format_with_commas_and_units() {
        assert_eq!(
            format_with_commas_and_units(1234, "CPU Instructions"),
            "1,234 inst."
        );
        assert_eq!(
            format_with_commas_and_units(1_000_000, "CPU Instructions"),
            "1,000,000 inst."
        );
        assert_eq!(format_with_commas_and_units(500, "Read Bytes"), "500 B");
        assert_eq!(
            format_with_commas_and_units(12345, "Write Bytes"),
            "12,345 B"
        );
        assert_eq!(
            format_with_commas_and_units(0, "CPU Instructions"),
            "0 inst."
        );
    }

    #[test]
    fn test_build_table_reports() {
        let reports = vec![CostReport {
            package: "pkg-a".into(),
            function: "foo".into(),
            metric: "CPU Instructions",
            value: 1000,
        }];
        let table = build_table_reports(&reports);
        assert!(table.contains("pkg-a"));
        assert!(table.contains("foo"));
        assert!(table.contains("1,000"));
    }

    #[test]
    fn test_build_json_reports() {
        let reports = vec![CostReport {
            package: "pkg-a".into(),
            function: "foo".into(),
            metric: "CPU Instructions",
            value: 1000,
        }];
        let json = build_json_reports(&reports).unwrap();
        assert!(json.contains("pkg-a"));
        assert!(json.contains("\"value\": 1000"));
    }

    #[test]
    fn test_build_table_empty_reports() {
        let table = build_table_reports(&[]);
        assert!(table.contains("WORKSPACE BUDGET REPORT"));
    }

    // -----------------------------------------------------------------------
    // Integration-style test for malformed toml → error
    // -----------------------------------------------------------------------

    /// This test simulates the error path when a malformed TOML is presented.
    /// It does NOT require network access or the stellar CLI.
    #[test]
    fn test_malformed_toml_does_not_panic() {
        let args = BudgetReportArgs {
            network: None,
            source: None,
            json: false,
        };
        // A malformed TOML string will silently be ignored (unwrap_or_default)
        // and then config resolution will fail because both are missing.
        let result = resolve_config(&args, Some("[[[broken toml]]"));
        assert!(result.is_err());
    }
}
