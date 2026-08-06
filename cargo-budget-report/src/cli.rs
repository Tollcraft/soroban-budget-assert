use clap::Parser;

/// Top-level CLI entry point for `cargo budget-report`.
///
/// Wraps the binary in a `cargo <subcommand>` compatible enum so it can be
/// invoked as `cargo budget-report [OPTIONS]`.
#[derive(Parser, Debug)]
#[command(name = "cargo", bin_name = "cargo")]
pub enum CargoCli {
    BudgetReport(BudgetReportArgs),
}

/// CLI arguments for `cargo budget-report`.
///
/// All fields are optional; missing values fall back to the corresponding
/// `budget.toml` configuration when available.
#[derive(Parser, Debug)]
pub struct BudgetReportArgs {
    /// Scaffold a commented `budget.toml` template and exit.
    #[arg(long)]
    pub init: bool,

    /// Allow `--init` to overwrite an existing `budget.toml`.
    #[arg(long)]
    pub force: bool,

    #[arg(long)]
    pub network: Option<String>,

    /// Override the RPC endpoint used for transaction simulation.
    ///
    /// When supplied, `--network-passphrase` is required and the named
    /// network defaults are not used.
    #[arg(long, requires = "network_passphrase", value_name = "URL")]
    pub rpc_url: Option<String>,

    /// Passphrase for the network served by `--rpc-url`.
    #[arg(long, value_name = "PASSPHRASE")]
    pub network_passphrase: Option<String>,

    #[arg(long)]
    pub source: Option<String>,

    #[arg(long, default_value_t = false)]
    pub json: bool,

    /// Enforce per-function limits declared in `budget.toml`.
    #[arg(long, default_value_t = false)]
    pub check: bool,

    /// Emit the report as CSV instead of a table or JSON.
    #[arg(long, default_value_t = false)]
    pub csv: bool,

    /// Write a new resource-usage baseline snapshot to this path and exit.
    #[arg(long)]
    pub record_baseline: Option<String>,

    /// Check current measurements against an existing baseline snapshot at
    /// this path, applying the configured regression tolerance.
    #[arg(long)]
    pub check_baseline: Option<String>,

    /// Override the regression tolerance.
    #[arg(long)]
    pub tolerance: Option<String>,

    /// Suppress non-essential progress messages and warnings on stderr.
    #[arg(long, default_value_t = false)]
    pub quiet: bool,

    /// Validate reported metrics with the Stellar CLI's XDR decoder.
    #[arg(long, default_value_t = false)]
    pub validate: bool,

    /// Cargo build profile to use when compiling contract WASM.
    #[arg(long)]
    pub profile: Option<String>,

    /// Derive local test limits from a Tier B JSON report and write them to
    /// this output path.
    #[arg(long, value_name = "OUT")]
    pub derive_limits: Option<String>,

    /// Source Tier B JSON report for --derive-limits.
    #[arg(long, value_name = "PATH")]
    pub from: Option<String>,

    /// CPU margin multiplier used by --derive-limits.
    #[arg(long)]
    pub margin_cpu: Option<f64>,

    /// Memory margin multiplier used by --derive-limits.
    #[arg(long)]
    pub margin_memory: Option<f64>,

    /// Read-bytes margin multiplier used by --derive-limits.
    #[arg(long)]
    pub margin_read: Option<f64>,

    /// Write-bytes margin multiplier used by --derive-limits.
    #[arg(long)]
    pub margin_write: Option<f64>,
}

impl BudgetReportArgs {
    /// Returns whether simulation should use an explicitly configured RPC
    /// endpoint rather than a named Stellar network.
    pub fn uses_custom_rpc(&self) -> bool {
        self.rpc_url.is_some()
    }

    /// Returns the selected named network when simulation is not using a
    /// custom RPC endpoint.
    pub fn selected_network(&self) -> Option<&str> {
        if self.uses_custom_rpc() {
            None
        } else {
            self.network.as_deref()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parses_custom_rpc_and_passphrase_without_named_network() {
        let cli = CargoCli::try_parse_from([
            "cargo",
            "budget-report",
            "--rpc-url",
            "http://127.0.0.1:8000/rpc",
            "--network-passphrase",
            "Standalone Network ; February 2017",
            "--network",
            "testnet",
        ])
        .expect("custom RPC arguments should parse");

        let CargoCli::BudgetReport(args) = cli;
        assert_eq!(args.rpc_url.as_deref(), Some("http://127.0.0.1:8000/rpc"));
        assert_eq!(
            args.network_passphrase.as_deref(),
            Some("Standalone Network ; February 2017")
        );
        assert!(args.uses_custom_rpc());
        assert_eq!(args.selected_network(), None);
    }

    #[test]
    fn custom_rpc_requires_network_passphrase() {
        let result = CargoCli::try_parse_from([
            "cargo",
            "budget-report",
            "--rpc-url",
            "http://127.0.0.1:8000/rpc",
        ]);

        assert!(result.is_err());
    }
}
