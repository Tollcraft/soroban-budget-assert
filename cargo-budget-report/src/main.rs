use anyhow::{Context, Result};
use cargo_budget_report::{
    build_json_reports, build_package, build_table_reports, decode_xdr, deploy_contract,
    discover_workspace, extract_tx_data_from_rpc, find_wasm_path, parse_costs_from_decoded_xdr,
    parse_exported_functions, resolve_config, send_rpc_request, simulate_function, CargoCli,
    CostReport,
};
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

    let metadata = discover_workspace()?;
    let mut reports = Vec::new();

    for package in &metadata.packages {
        let is_cdylib = package
            .targets
            .iter()
            .any(|t| t.crate_types.iter().any(|c| c.to_string() == "cdylib"));
        if !is_cdylib {
            continue;
        }

        build_package(&package.name)?;

        let wasm_path = match find_wasm_path(&metadata, &package.name) {
            Some(p) => p,
            None => {
                eprintln!("Warning: WASM not found for {}", package.name);
                continue;
            }
        };

        let wasm_bytes = std::fs::read(&wasm_path).context("failed to read wasm file")?;
        let exported_fns = parse_exported_functions(&wasm_bytes)?;

        if exported_fns.is_empty() {
            eprintln!("No exported functions found in {}", package.name);
            continue;
        }

        eprintln!("Deploying contract '{}'...", package.name);
        let contract_id = deploy_contract(&wasm_path, &source, &network)?;
        eprintln!("Contract deployed at: {contract_id}");

        for function in &exported_fns {
            eprintln!("Simulating function '{function}'...");

            let func_args_list = func_args.get(function).cloned().unwrap_or_default();

            let b64_xdr =
                match simulate_function(&contract_id, function, &func_args_list, &source, &network)
                {
                    Ok(x) => x,
                    Err(e) => {
                        eprintln!("Warning: Simulation failed for {function}: {e}");
                        continue;
                    }
                };

            let rpc_payload = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "simulateTransaction",
                "params": {
                    "transaction": b64_xdr
                }
            });

            let rpc_resp = send_rpc_request(&rpc_payload)?;

            let tx_data_b64 = match extract_tx_data_from_rpc(&rpc_resp) {
                Ok(d) => d.to_string(),
                Err(e) => {
                    eprintln!("Warning: RPC error for {function}: {e}");
                    continue;
                }
            };

            let decoded = decode_xdr(&tx_data_b64)?;
            let costs = parse_costs_from_decoded_xdr(&decoded);

            reports.push(CostReport {
                package: package.name.to_string(),
                function: function.clone(),
                metric: "CPU Instructions",
                value: costs.instructions,
            });
            reports.push(CostReport {
                package: package.name.to_string(),
                function: function.clone(),
                metric: "Read Bytes",
                value: costs.read_bytes,
            });
            reports.push(CostReport {
                package: package.name.to_string(),
                function: function.clone(),
                metric: "Write Bytes",
                value: costs.write_bytes,
            });
        }
    }

    if reports.is_empty() {
        eprintln!("No successful simulations to report.");
        return Ok(());
    }

    if args.json {
        println!("{}", build_json_reports(&reports)?);
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
}
