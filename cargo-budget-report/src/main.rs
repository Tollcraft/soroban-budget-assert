use anyhow::{Context, Result};
use cargo_budget_report::{
    build_json_reports, build_package, build_table_reports, decode_xdr, deploy_contract,
    discover_workspace, extract_tx_data_from_rpc, find_wasm_path, parse_costs_from_decoded_xdr,
    parse_exported_functions, resolve_config, send_rpc_request, simulate_function, CargoCli,
    CostReport,
};
use clap::Parser;

fn main() -> Result<()> {
    let CargoCli::BudgetReport(args) = CargoCli::parse();

    let toml_content = std::fs::read_to_string("budget.toml").ok();
    let (network, source, func_args) = resolve_config(&args, toml_content.as_deref())?;

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
        print!("{}", build_table_reports(&reports));
    }

    Ok(())
}
