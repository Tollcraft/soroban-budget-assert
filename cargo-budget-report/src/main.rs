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
}

#[derive(serde::Deserialize, Default, Debug)]
struct BudgetToml {
    network: Option<String>,
    source: Option<String>,
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
    read_limit: Option<u64>,
    #[serde(default)]
    write_limit: Option<u64>,
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

fn main() -> Result<()> {
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
}
