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

/// Conservative default for `--concurrency` (functions in flight per
/// package). See the flag help text for the rationale.
pub const DEFAULT_CONCURRENCY: usize = 4;

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

    #[arg(long)]
    pub source: Option<String>,

    /// Permit the run to build and deploy against a non-disposable network.
    ///
    /// `cargo budget-report` deploys a contract and simulates calls against
    /// it. Against testnet, futurenet, or a local network that is free and
    /// throwaway. Against Stellar Mainnet it funds a source account and
    /// pushes a contract using real funds. Without this flag the run stops
    /// before building anything when the resolved network is Mainnet — or
    /// when it cannot be recognised as disposable, which is treated the same
    /// way rather than assumed safe.
    #[arg(long, default_value_t = false)]
    pub allow_mainnet: bool,

    #[arg(long, default_value_t = false, conflicts_with = "csv")]
    pub json: bool,

    /// Emit the report as Markdown instead of a table, JSON, or CSV.
    #[arg(long, default_value_t = false)]
    pub markdown: bool,

    /// Enforce per-function limits declared in `budget.toml`.
    ///
    /// When set, each measured metric is compared against its configured
    /// `cpu_limit` / `read_limit` / `write_limit`. A missing limit means the
    /// metric is reported but **not** enforced. The process exits with a
    /// non-zero status when any limit is breached, or when a function that
    /// has a `budget.toml` entry fails to simulate. Functions that are not
    /// declared in `budget.toml` are reported only.
    #[arg(long, default_value_t = false)]
    pub check: bool,

    /// Emit the report as CSV instead of a table or JSON.
    #[arg(long, default_value_t = false)]
    pub csv: bool,

    /// Write a new resource-usage baseline snapshot to this path and exit.
    #[arg(long, conflicts_with = "check_baseline")]
    pub record_baseline: Option<String>,

    /// Check current measurements against an existing baseline snapshot at
    /// this path, applying the configured regression tolerance.
    #[arg(long, conflicts_with = "record_baseline")]
    pub check_baseline: Option<String>,

    /// Override the regression tolerance (e.g. "0.10" for 10%). Takes
    /// precedence over `tolerance` in `budget.toml`.
    #[arg(long)]
    pub tolerance: Option<String>,

    /// Drop rows whose value is unchanged from the baseline in the
    /// `--check-baseline` comparison.
    ///
    /// In the Markdown output the default instead collapses unchanged rows
    /// into a `<details>` block; this flag omits them from both formats.
    #[arg(long, default_value_t = false)]
    pub hide_unchanged: bool,

    /// Suppress non-essential progress messages and warnings on stderr.
    ///
    /// The final report (table, JSON, or CSV) is still printed to stdout.
    /// Fatal errors from child-process spawn failures or hard contract
    /// build failures are not suppressed — they always go to stderr
    /// regardless of this flag.
    #[arg(long, default_value_t = false)]
    pub quiet: bool,

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
    pub validate: bool,

    /// Cargo build profile to use when compiling the contract WASM.
    ///
    /// Defaults to `release` when not provided. Custom profiles (e.g.
    /// `release-opt`) must be defined in the project's `Cargo.toml`.
    #[arg(long)]
    pub profile: Option<String>,

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
    pub derive_limits: Option<String>,

    /// Source Tier B JSON report for `--derive-limits`. Use `-` to
    /// read JSON from stdin (so `cargo budget-report --json | cargo
    /// budget-report --derive-limits tier-a-limits.env` composes).
    #[arg(long, value_name = "PATH")]
    pub from: Option<String>,

    /// Per-metric multiplier applied to Tier B CPU values. Required
    /// unless `[margin].cpu_margin` is set in `budget.toml`; no
    /// default is applied because the project deliberately treats the
    /// margin as data (issue #45) and silently picking a value would
    /// defeat the audit trail.
    #[arg(long, value_name = "F")]
    pub margin_cpu: Option<String>,

    /// Per-metric multiplier applied to Tier B memory values.
    #[arg(long, value_name = "F")]
    pub margin_memory: Option<String>,

    /// Per-metric multiplier applied to Tier B read-bytes values.
    #[arg(long, value_name = "F")]
    pub margin_read: Option<String>,

    /// Per-metric multiplier applied to Tier B write-bytes values.
    #[arg(long, value_name = "F")]
    pub margin_write: Option<String>,

    /// Path to write the Markdown provenance table next to the env
    /// file. Defaults to `<OUT>` with `.env` replaced by `.md` (e.g.
    /// `tier-a-limits.provenance.md` for `tier-a-limits.env`).
    #[arg(long, value_name = "PATH")]
    pub provenance_out: Option<String>,

    /// Maximum number of attempts (including the first) for deploy,
    /// invoke-build, and simulate-RPC calls before giving up. `1`
    /// disables retry entirely. Overrides `retry.max_attempts` in
    /// `budget.toml`; defaults to 4.
    #[arg(long, value_name = "N")]
    pub max_retry_attempts: Option<u32>,

    /// Initial backoff, in seconds, before the first retry. Doubles on
    /// each subsequent attempt (2 → 4 → 8). Overrides
    /// `retry.initial_backoff_secs` in `budget.toml`; defaults to 2.
    #[arg(long, value_name = "SECS")]
    pub retry_backoff_secs: Option<u64>,

    /// Emit the report as a single self-contained HTML page instead of a
    /// table, JSON, or CSV.
    ///
    /// The page has no external CSS, scripts, or fonts, so it renders
    /// correctly from a `file://` URL and from a downloaded CI artifact.
    /// Each row shows the same values as `--json` for the same run; in
    /// `--check` mode rows also show their limit and pass/fail status.
    #[arg(long, default_value_t = false)]
    pub html: bool,

    /// Record every transport response (deploy, invoke-build, and
    /// simulate RPC) into a replayable fixture file at this path.
    ///
    /// The run itself still talks to the network; the fixture it writes
    /// lets a later `--replay` run reproduce the same report offline.
    #[arg(long, value_name = "PATH", conflicts_with = "replay")]
    pub record: Option<String>,

    /// Replay a run from a fixture file written by `--record`.
    ///
    /// The whole report pipeline runs offline: no `stellar` CLI, no
    /// `curl`, no network access. Deploy, invoke-build and simulate RPC
    /// responses are served from the fixture. `--record` and `--replay`
    /// are mutually exclusive.
    #[arg(long, value_name = "PATH", conflicts_with = "record")]
    pub replay: Option<String>,

    /// When to colourise the plain-text `--check` report.
    ///
    /// Breaching rows are rendered red so they stand out when scanning a
    /// mixed pass/fail table. The status is also carried as plain text
    /// (`PASS`/`FAIL` markers), so no information is lost when colour is
    /// disabled. CSV, JSON, and HTML output are never coloured.
    #[arg(long, value_enum, default_value_t = ColorChoice::Auto)]
    pub color: ColorChoice,

    /// Watch the workspace for file changes and re-measure on save.
    ///
    /// When set, the tool enters a loop: it watches the workspace for
    /// changes to source files, and on each change rebuilds and re-measures
    /// only the affected packages. Each run prints a comparison against the
    /// previous run so the delta is visible.
    ///
    /// Edits that arrive while a run is in flight are coalesced, not queued.
    /// A build failure prints the error and keeps watching. Ctrl-C exits
    /// cleanly.
    ///
    /// Refuses to start when stdout is not a terminal (CI guard).
    #[arg(long, default_value_t = false)]
    pub watch: bool,

    /// Custom Soroban RPC endpoint to simulate against (#49).
    ///
    /// Overrides the built-in `testnet` / `futurenet` endpoints so a local
    /// standalone RPC node (e.g. `http://localhost:8000/soroban/rpc` from a
    /// Docker quickstart image) can be targeted to avoid public-network rate
    /// limits or to exercise custom fee settings. `--network-passphrase` is
    /// required whenever this is set.
    #[arg(long, value_name = "URL", requires = "network_passphrase")]
    pub rpc_url: Option<String>,

    /// Network passphrase for `--rpc-url` (#49).
    ///
    /// Must match the passphrase the target RPC node was started with, e.g.
    /// `"Standalone Network ; February 2017"` for a local quickstart node.
    /// Only meaningful together with `--rpc-url`.
    #[arg(long, value_name = "PASSPHRASE")]
    pub network_passphrase: Option<String>,

    /// Skip the on-disk deploy cache for this run and redeploy every
    /// contract from scratch (#79).
    ///
    /// The cache (`.budget-cache.toml`) keys deployed contract ids on the
    /// compiled wasm hash, the network, and the source account; any change
    /// to those redeploys automatically. Use this flag to force a redeploy
    /// even on an unchanged build (e.g. the cached contract was reclaimed by
    /// ledger state). Deleting `.budget-cache.toml` has the same effect.
    #[arg(long, default_value_t = false)]
    pub no_deploy_cache: bool,

    /// Secret seed (`S...`) of the source account, for native deploy/submit
    /// without the `stellar` CLI key store (#123).
    ///
    /// Falls back to the `STELLAR_SECRET_KEY` environment variable. When
    /// neither is set, deploy and invoke still go through the `stellar` CLI,
    /// which must be installed (checked at preflight).
    #[arg(long, value_name = "S...", env = "STELLAR_SECRET_KEY")]
    pub source_secret: Option<String>,
}

/// Colour policy for the plain-text `--check` output.
#[derive(clap::ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorChoice {
    /// Colour only when stdout is a terminal and `NO_COLOR` is unset or
    /// empty (the no-color.org convention).
    #[default]
    Auto,
    /// Always emit colour, even into pipes and files.
    Always,
    /// Never emit colour.
    Never,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;

    #[test]
    fn json_and_csv_are_mutually_exclusive() {
        let err = CargoCli::try_parse_from(["cargo", "budget-report", "--json", "--csv"])
            .expect_err("--json and --csv together should be rejected");
        assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
    }

    #[test]
    fn json_alone_is_accepted() {
        let result = CargoCli::try_parse_from(["cargo", "budget-report", "--json"]);
        assert!(result.is_ok(), "--json alone should parse: {result:?}");
    }

    #[test]
    fn csv_alone_is_accepted() {
        let result = CargoCli::try_parse_from(["cargo", "budget-report", "--csv"]);
        assert!(result.is_ok(), "--csv alone should parse: {result:?}");
    }

    fn parse_args(argv: &[&str]) -> Result<BudgetReportArgs, clap::Error> {
        let mut full = vec!["cargo", "budget-report"];
        full.extend_from_slice(argv);
        CargoCli::try_parse_from(full).map(|CargoCli::BudgetReport(a)| a)
    }

    #[test]
    fn rpc_url_requires_network_passphrase() {
        let err = parse_args(&["--rpc-url", "http://localhost:8000/soroban/rpc"])
            .expect_err("--rpc-url without --network-passphrase should be rejected");
        assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn rpc_url_with_passphrase_parses_and_overrides() {
        let args = parse_args(&[
            "--rpc-url",
            "http://localhost:8000/soroban/rpc",
            "--network-passphrase",
            "Standalone Network ; February 2017",
        ])
        .expect("--rpc-url + --network-passphrase should parse");
        assert_eq!(
            args.rpc_url.as_deref(),
            Some("http://localhost:8000/soroban/rpc")
        );
        assert_eq!(
            args.network_passphrase.as_deref(),
            Some("Standalone Network ; February 2017")
        );
    }

    #[test]
    fn no_deploy_cache_defaults_off_and_parses_on() {
        assert!(!parse_args(&[]).unwrap().no_deploy_cache);
        assert!(parse_args(&["--no-deploy-cache"]).unwrap().no_deploy_cache);
    }

    #[test]
    fn source_secret_parses_from_flag() {
        let args = parse_args(&["--source-secret", "SXXXXXXXX"]).unwrap();
        assert_eq!(args.source_secret.as_deref(), Some("SXXXXXXXX"));
    }

    #[test]
    fn record_baseline_and_check_baseline_are_mutually_exclusive() {
        let err = CargoCli::try_parse_from([
            "cargo",
            "budget-report",
            "--record-baseline",
            "out.json",
            "--check-baseline",
            "base.json",
        ])
        .expect_err("--record-baseline and --check-baseline together should be rejected");
        assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
    }
}
