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

    #[arg(long)]
    pub source: Option<String>,

    #[arg(long, default_value_t = false)]
    pub json: bool,

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
    #[arg(long)]
    pub record_baseline: Option<String>,

    /// Check current measurements against an existing baseline snapshot at
    /// this path, applying the configured regression tolerance.
    #[arg(long)]
    pub check_baseline: Option<String>,

    /// Override the regression tolerance (e.g. "0.10" for 10%). Takes
    /// precedence over `tolerance` in `budget.toml`.
    #[arg(long)]
    pub tolerance: Option<String>,

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
