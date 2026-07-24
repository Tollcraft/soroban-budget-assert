use anyhow::{Context, Result};
use cargo_metadata::MetadataCommand;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::process::Command;
use tabled::{Table, Tabled};
use wasmparser::Parser as WasmParser;

const CACHE_FILE: &str = ".budget-cache.toml";

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

    #[arg(long, default_value_t = false)]
    force_deploy: bool,

    #[arg(long)]
    package: Vec<String>,

    #[arg(long)]
    function: Vec<String>,
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

fn load_cache() -> BudgetCache {
    std::fs::read_to_string(CACHE_FILE)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_cache(cache: &BudgetCache) {
    let toml_str = toml::to_string_pretty(cache).unwrap_or_default();
    let _ = std::fs::write(CACHE_FILE, toml_str);
}

fn wasm_sha256(wasm_bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(wasm_bytes);
    hex::encode(hasher.finalize())
}

fn get_contract_id_for_package(
    package_name: &str,
    wasm_bytes: &[u8],
    network: &str,
    source: &str,
    wasm_path_str: &str,
    force_deploy: bool,
    cache: &mut BudgetCache,
) -> Result<String> {
    let hash = wasm_sha256(wasm_bytes);

    if !force_deploy {
        if let Some(entry) = cache.package.get(package_name) {
            if entry.wasm_sha256 == hash && entry.network == network {
                eprintln!(
                    "Cache hit for '{}' — reusing contract id {}",
                    package_name, entry.contract_id
                );
                return Ok(entry.contract_id.clone());
            }
        }
    }

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "✔"])
            .template("{spinner:.green} Deploying contract {msg}...")
            .unwrap(),
    );
    spinner.set_message(package_name.to_string());
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

    let deploy_output = Command::new("stellar")
        .args([
            "contract",
            "deploy",
            "--wasm",
            wasm_path_str,
            "--source",
            source,
            "--network",
            network,
        ])
        .output()
        .context("failed to execute stellar-cli deploy")?;

    spinner.finish_and_clear();

    if !deploy_output.status.success() {
        anyhow::bail!(
            "Failed to deploy {}. Ensure your source account is funded.\nError: {}",
            package_name,
            String::from_utf8_lossy(&deploy_output.stderr)
        );
    }

    let contract_id = String::from_utf8_lossy(&deploy_output.stdout)
        .trim()
        .to_string();
    eprintln!("Contract deployed at: {}", contract_id);

    cache.package.insert(
        package_name.to_string(),
        CacheEntry {
            wasm_sha256: hash,
            contract_id: contract_id.clone(),
            network: network.to_string(),
        },
    );
    save_cache(cache);

    Ok(contract_id)
}

fn main() -> Result<()> {
    let CargoCli::BudgetReport(args) = CargoCli::parse();

    let toml_config: BudgetToml = std::fs::read_to_string("budget.toml")
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default();

    let network = args
        .network
        .or(toml_config.network)
        .context("missing --network or budget.toml network field")?;
    let source = args
        .source
        .or(toml_config.source)
        .context("missing --source or budget.toml source field")?;

    let requested_packages: Vec<String> = args.package;
    let requested_functions: Vec<String> = args.function;
    let force_deploy = args.force_deploy;

    let mut cache = load_cache();

    eprintln!("Discovering workspace members...");
    let metadata = MetadataCommand::new()
        .no_deps()
        .exec()
        .context("failed to execute cargo metadata")?;

    let cdylib_names: Vec<&str> = metadata
        .packages
        .iter()
        .filter(|p| {
            p.targets
                .iter()
                .any(|t| t.crate_types.iter().any(|c| c.to_string() == "cdylib"))
        })
        .map(|p| p.name.as_str())
        .collect();

    let mut reports = Vec::new();

    for package in &metadata.packages {
        let is_cdylib = package
            .targets
            .iter()
            .any(|t| t.crate_types.iter().any(|c| c.to_string() == "cdylib"));
        if !is_cdylib {
            continue;
        }

        if !requested_packages.is_empty() && !requested_packages.contains(&package.name) {
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

        let wasm_bytes = std::fs::read(&wasm_path).context("failed to read wasm file")?;
        let mut exported_fns = Vec::new();

        for payload in WasmParser::new(0).parse_all(&wasm_bytes) {
            if let wasmparser::Payload::ExportSection(s) = payload? {
                for export in s {
                    let export = export?;
                    if export.kind == wasmparser::ExternalKind::Func {
                        let name = export.name.to_string();
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

        if !requested_functions.is_empty() {
            let unknown: Vec<&String> = requested_functions
                .iter()
                .filter(|f| !exported_fns.contains(f))
                .collect();
            if !unknown.is_empty() {
                anyhow::bail!(
                    "Unknown function(s) for package '{}': {}\nAvailable functions: {}",
                    package.name,
                    unknown
                        .iter()
                        .map(|name| name.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                    exported_fns.join(", ")
                );
            }
        }

        let contract_id = get_contract_id_for_package(
            &package.name,
            &wasm_bytes,
            &network,
            &source,
            wasm_path.as_str(),
            force_deploy,
            &mut cache,
        )?;

        for function in &exported_fns {
            if !requested_functions.is_empty() && !requested_functions.contains(function) {
                continue;
            }

            eprintln!("Simulating function '{}'...", function);

            let func_config = toml_config.functions.get(function);
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
            let parsed: serde_json::Value =
                serde_json::from_str(&json_str).context("Failed to parse XDR JSON")?;

            let instructions = parsed["resources"]["instructions"].as_u64().unwrap_or(0) as u32;
            let read_bytes = parsed["resources"]["disk_read_bytes"].as_u64().unwrap_or(0) as u32;
            let write_bytes = parsed["resources"]["write_bytes"].as_u64().unwrap_or(0) as u32;

            reports.push(CostReport {
                package: package.name.to_string(),
                function: function.clone(),
                metric: "CPU Instructions",
                value: instructions,
            });
            reports.push(CostReport {
                package: package.name.to_string(),
                function: function.clone(),
                metric: "Read Bytes",
                value: read_bytes,
            });
            reports.push(CostReport {
                package: package.name.to_string(),
                function: function.clone(),
                metric: "Write Bytes",
                value: write_bytes,
            });
        }
    }

    if !requested_packages.is_empty() {
        let unknown_pkgs: Vec<&String> = requested_packages
            .iter()
            .filter(|p| !cdylib_names.contains(&p.as_str()))
            .collect();
        if !unknown_pkgs.is_empty() {
            anyhow::bail!(
                "Unknown package(s): {}\nAvailable cdylib packages: {}",
                unknown_pkgs
                    .iter()
                    .map(|name| name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                cdylib_names.join(", ")
            );
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
        println!("\nSummary: The metrics above represent the total unrefundable network execution costs required to run your contract functions.");
        println!("* Note: These are simulated numbers on testnet and may vary slightly depending on ledger state.");
    }

    Ok(())
}
