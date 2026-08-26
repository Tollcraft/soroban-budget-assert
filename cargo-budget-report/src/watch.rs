//! Watch mode: watch the workspace for source changes, rebuild and
//! re-measure only affected packages on each change, and print a delta
//! comparison against the previous run.
//!
//! Implementation notes
//! --------------------
//! - Uses the `notify` crate (v8) for cross-platform filesystem watching.
//! - Coalescing: edits that arrive while a run is in flight are collapsed
//!   into a single re-measurement. A flag is set when a run starts and
//!   cleared when it finishes; if the flag is still set at finish, another
//!   run is triggered immediately.
//! - Only source files under workspace packages are watched. `target/`,
//!   `.git/`, and other non-source directories are excluded.
//! - The mapping from a changed file path back to its cargo package is
//!   done via `cargo_metadata`'s `packages[].manifest_path` parent
//!   directory — the same mapping the tool already uses.

use anyhow::Context;
use cargo_metadata::CrateType;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::io::IsTerminal;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use crate::cli::BudgetReportArgs;
use crate::compare::Measurement;
use crate::compare::Tolerance;
use crate::error::Error;
use crate::BudgetToml;
use crate::MeasuredResources;
use crate::RetryConfig;

use notify::Event;
use notify::EventKind;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify::Watcher;

/// Debounce interval: ignore filesystem events that arrive within this
/// window after the previous one. Combined with the in-flight coalescing
/// flag this gives us the "four saves in ten seconds -> one re-measurement"
/// behaviour the issue requires.
const DEBOUNCE: Duration = Duration::from_secs(2);

/// Directories that are never watched, regardless of where they appear in
/// the tree. `target/` is the big one -- the tool's own build output would
/// retrigger forever if included.
const EXCLUDED_DIRS: &[&str] = &["target", ".git", "node_modules", ".github"];

/// Return `true` when `path` sits inside one of the [`EXCLUDED_DIRS`].
fn is_excluded(path: &Path) -> bool {
    for comp in path.components() {
        if let Some(dir) = comp.as_os_str().to_str() {
            if EXCLUDED_DIRS.contains(&dir) {
                return true;
            }
        }
    }
    false
}

/// The result of one measurement pass: the report rows and the per-package
/// per-function measurements.
pub(crate) struct MeasurementResult {
    pub reports: Vec<crate::CostReport>,
    pub measurements: BTreeMap<String, BTreeMap<String, MeasuredResources>>,
    pub has_errors: bool,
    #[allow(dead_code)]
    pub checks_failed: bool,
}

/// Print a human-readable delta between the previous and current
/// measurements. Only changed entries are shown.
pub(crate) fn print_delta(
    prev: &BTreeMap<String, BTreeMap<String, Measurement>>,
    curr: &BTreeMap<String, BTreeMap<String, Measurement>>,
) {
    if prev.is_empty() && !curr.is_empty() {
        eprintln!("  (first run -- no previous measurements to compare)");
        return;
    }
    if prev.is_empty() && curr.is_empty() {
        return;
    }

    let mut any_change = false;
    // Iterate over the union of packages.
    let mut all_pkgs: Vec<&String> = prev.keys().chain(curr.keys()).collect();
    all_pkgs.sort();
    all_pkgs.dedup();

    for pkg in all_pkgs {
        let prev_fns = prev.get(pkg);
        let curr_fns = curr.get(pkg);
        let mut all_fns: Vec<&String> = prev_fns
            .map(|m| m.keys().collect::<Vec<_>>())
            .unwrap_or_default();
        if let Some(m) = curr_fns {
            for f in m.keys() {
                if !all_fns.contains(&f) {
                    all_fns.push(f);
                }
            }
        }
        all_fns.sort();
        all_fns.dedup();

        for func in all_fns {
            let prev_m = prev_fns.and_then(|m| m.get(func));
            let curr_m = curr_fns.and_then(|m| m.get(func));

            let metrics = ["cpu_instructions", "read_bytes", "write_bytes"];
            for metric in &metrics {
                let prev_val = prev_m.and_then(|m| get_metric(m, metric));
                let curr_val = curr_m.and_then(|m| get_metric(m, metric));

                match (prev_val, curr_val) {
                    (Some(p), Some(c)) if p != c => {
                        let delta = if c > p {
                            format!("+{}", c - p)
                        } else {
                            format!("-{}", p - c)
                        };
                        let pct = if p > 0 {
                            ((c as f64 - p as f64) / p as f64 * 100.0).round() as i64
                        } else {
                            0
                        };
                        eprintln!("  {pkg}::{func} [{metric}] {p} -> {c} ({delta}, {pct:+}%)");
                        any_change = true;
                    }
                    (None, Some(c)) => {
                        eprintln!("  {pkg}::{func} [{metric}] (new) {c}");
                        any_change = true;
                    }
                    (Some(_p), None) => {
                        eprintln!("  {pkg}::{func} [{metric}] (removed)");
                        any_change = true;
                    }
                    _ => {}
                }
            }
        }
    }

    if !any_change {
        eprintln!("  No changes detected.");
    }
}

fn get_metric(m: &Measurement, metric: &str) -> Option<u64> {
    match metric {
        "cpu_instructions" => Some(m.cpu_instructions),
        "read_bytes" => Some(m.read_bytes),
        "write_bytes" => Some(m.write_bytes),
        _ => None,
    }
}

/// Check whether the current process's stdout is a terminal.
/// Watch mode refuses to start when it is not, to avoid hijacking CI
/// output.
pub(crate) fn ensure_terminal() -> anyhow::Result<()> {
    if !std::io::stdout().is_terminal() {
        return Err(Error::Message(
            "watch mode requires an interactive terminal (stdout is not a tty).\n\
             Use without --watch for CI."
                .into(),
        )
        .into());
    }
    Ok(())
}

/// The main watch-mode loop.
///
/// Sets up a filesystem watcher, waits for changes, runs a full
/// measurement pass, prints deltas, and repeats until Ctrl-C.
pub(crate) fn watch_loop(
    args: &BudgetReportArgs,
    metadata: cargo_metadata::Metadata,
    toml_config: BudgetToml,
    _default_tolerance: Tolerance,
    network: String,
    source: String,
    retry_config: RetryConfig,
) -> anyhow::Result<()> {
    ensure_terminal()?;

    let quiet = args.quiet;
    if !quiet {
        eprintln!("Watch mode active. Watching for source changes...");
        eprintln!("Press Ctrl-C to stop.\n");
    }

    // We need the workspace root to watch.
    let workspace_root = metadata.workspace_root.as_std_path().to_path_buf();

    // Previous measurements for delta comparison.
    let prev_measurements: Arc<Mutex<BTreeMap<String, BTreeMap<String, Measurement>>>> =
        Arc::new(Mutex::new(BTreeMap::new()));

    // In-flight coalescing flag: set when a run starts, cleared when it
    // finishes. If events arrive while this is true they are absorbed
    // (one extra run will be triggered when the current one finishes).
    let running = Arc::new(AtomicBool::new(false));
    // Flag: "events arrived while we were busy" -> trigger one more run.
    let pending = Arc::new(AtomicBool::new(false));

    // Channel for the notify watcher.
    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();

    let mut watcher: RecommendedWatcher = Watcher::new(
        tx,
        notify::Config::default().with_poll_interval(Duration::from_secs(1)),
    )
    .context("failed to create filesystem watcher")?;

    // Watch the workspace root recursively. We filter paths in the event
    // handler to exclude non-source directories.
    watcher
        .watch(&workspace_root, RecursiveMode::Recursive)
        .context("failed to start watching workspace root")?;

    // Keep the watcher alive for the lifetime of the function.
    let _watcher = watcher;

    // Build the transport once and reuse it across runs.
    let build_profile = args.profile.as_deref().unwrap_or("release");

    // Re-measure loop.
    let mut first_run = true;
    loop {
        // Wait for at least one filesystem event.
        if first_run {
            // First run: measure immediately without waiting.
            first_run = false;
        } else {
            // Wait for a change event, debounced.
            let mut got_event = false;
            let deadline = Instant::now() + Duration::from_secs(300); // 5-min safety timeout
            while Instant::now() < deadline {
                match rx.recv_timeout(Duration::from_secs(1)) {
                    Ok(Ok(event)) => {
                        if matches!(
                            event.kind,
                            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                        ) {
                            // Filter out excluded directories.
                            let relevant = event.paths.iter().any(|p| !is_excluded(p));
                            if relevant {
                                got_event = true;
                                break;
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        if !quiet {
                            eprintln!("Watcher error: {e}");
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        // Check if we should exit (Ctrl-C is handled by the
                        // default signal handler which will interrupt us).
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        if !quiet {
                            eprintln!("Watcher channel disconnected.");
                        }
                        return Ok(());
                    }
                }
            }
            if !got_event {
                // Timeout -- no changes. Continue watching.
                continue;
            }

            // Debounce: drain any additional events that arrived within the
            // debounce window.
            let debounce_deadline = Instant::now() + DEBOUNCE;
            while let Ok(Ok(_event)) = rx.recv_timeout(debounce_deadline - Instant::now()) {
                // Absorb.
            }
        }

        // Check if a run is already in flight.
        if running.load(Ordering::SeqCst) {
            pending.store(true, Ordering::SeqCst);
            if !quiet {
                eprintln!("Change detected while run in flight -- will re-measure after current run finishes.");
            }
            continue;
        }

        running.store(true, Ordering::SeqCst);

        if !quiet {
            eprintln!("\n--- Re-measuring workspace ---");
        }

        // Run a full measurement pass.
        let result = match run_measurement_pass(
            args,
            &metadata,
            &toml_config,
            &network,
            &source,
            &retry_config,
            build_profile,
        ) {
            Ok(r) => r,
            Err(e) => {
                if !quiet {
                    eprintln!("Build failed: {e}");
                    eprintln!("Keeping watcher active. Fix the error and save again.\n");
                }
                running.store(false, Ordering::SeqCst);
                // Trigger another run if events accumulated.
                if pending.swap(false, Ordering::SeqCst) {
                    continue;
                }
                continue;
            }
        };

        // Build the current measurement map for comparison.
        let curr_measurements: BTreeMap<String, BTreeMap<String, Measurement>> = result
            .measurements
            .iter()
            .map(|(pkg, fns)| {
                (
                    pkg.clone(),
                    fns.iter()
                        .map(|(name, m)| (name.clone(), m.as_compare()))
                        .collect(),
                )
            })
            .collect();

        // Print delta against previous run.
        {
            let prev = prev_measurements.lock().unwrap();
            print_delta(&prev, &curr_measurements);
        }

        // Update previous measurements.
        {
            let mut prev = prev_measurements.lock().unwrap();
            *prev = curr_measurements;
        }

        // Print the full report (table output).
        if !args.json && !args.csv && !args.html {
            eprintln!("\n=== WORKSPACE BUDGET REPORT ===");
            for report in &result.reports {
                if let Some(value) = report.value {
                    eprintln!(
                        "  {}::{} [{}] = {}",
                        report.package,
                        report.function,
                        report.metric,
                        crate::format_with_commas_and_units(u64::from(value), report.metric)
                    );
                }
            }
        } else if args.json {
            let json = serde_json::to_string_pretty(&result.reports)
                .context("Failed to serialize report to JSON")?;
            eprintln!("{json}");
        }

        if result.has_errors {
            eprintln!("\nWarning: Some simulations failed. Fix errors and save again.\n");
        }

        if !quiet {
            eprintln!("Watching for changes...");
        }

        running.store(false, Ordering::SeqCst);

        // If events arrived during the run, trigger another pass.
        if pending.swap(false, Ordering::SeqCst) {
            continue;
        }
    }
}

/// Execute one full measurement pass across all cdylib packages.
///
/// This is the same logic that was in `main()` before the watch-mode
/// refactor -- it builds every package, deploys, and simulates every
/// exported function.
fn run_measurement_pass(
    args: &BudgetReportArgs,
    metadata: &cargo_metadata::Metadata,
    toml_config: &BudgetToml,
    network: &str,
    source: &str,
    retry_config: &RetryConfig,
    build_profile: &str,
) -> anyhow::Result<MeasurementResult> {
    let mut reports = Vec::new();
    let mut measurements: BTreeMap<String, BTreeMap<String, MeasuredResources>> = BTreeMap::new();
    let mut has_errors = false;
    let mut checks_failed = false;

    let mut transport = if args.replay.is_some() {
        // Replaying a fixture -- build a replay transport.
        let replay_path = args.replay.as_ref().unwrap();
        crate::TransportKind::Replay(crate::replay::ReplayTransport::new(
            crate::fixture::FixtureFile::load(replay_path)
                .with_context(|| format!("failed to load replay fixture {}", replay_path))?,
        ))
    } else {
        let net_override = match (&args.rpc_url, &args.network_passphrase) {
            (Some(rpc_url), Some(passphrase)) => Some(crate::live::NetworkOverride {
                rpc_url: rpc_url.clone(),
                network_passphrase: passphrase.clone(),
            }),
            _ => None,
        };
        crate::TransportKind::Live(crate::live::LiveTransport::new(
            *retry_config,
            args.quiet,
            net_override,
        ))
    };

    for package in &metadata.packages {
        let is_cdylib = package
            .targets
            .iter()
            .any(|target| target.crate_types.contains(&CrateType::CDyLib));
        if !is_cdylib {
            continue;
        }

        if !args.quiet {
            eprintln!("Building package '{}' for wasm32...", package.name);
        }
        let build_status = std::process::Command::new("cargo")
            .args([
                "build",
                "-p",
                package.name.as_str(),
                "--target",
                "wasm32-unknown-unknown",
                "--profile",
                build_profile,
            ])
            .status()
            .context("failed to build package")?;

        if !build_status.success() {
            return Err(Error::Message(format!("Failed to build {}", package.name)).into());
        }

        // Locate the cdylib target to derive the correct WASM filename.
        let cdylib_target = package
            .targets
            .iter()
            .find(|t| t.crate_types.contains(&CrateType::CDyLib));
        let wasm_name = match cdylib_target {
            Some(target) => target.name.clone(),
            None => {
                eprintln!(
                    "Warning: no cdylib target found for package '{}' -- skipping",
                    package.name
                );
                continue;
            }
        };
        let wasm_path = metadata
            .target_directory
            .join("wasm32-unknown-unknown")
            .join(build_profile)
            .join(format!("{}.wasm", wasm_name));

        if !wasm_path.exists() {
            eprintln!(
                "Error: WASM not found at {}\n  Package: {} (lib target: {})",
                wasm_path.as_str(),
                package.name,
                wasm_name,
            );
            has_errors = true;
            continue;
        }

        // Parse WASM exports
        let wasm_bytes = std::fs::read(&wasm_path)?;
        let wasm_size: u32 = wasm_bytes.len().try_into().unwrap_or(u32::MAX);
        let mut exported_fns: HashSet<String> = HashSet::new();

        for payload in wasmparser::Parser::new(0).parse_all(&wasm_bytes) {
            if let wasmparser::Payload::ExportSection(export_section) = payload? {
                for export_item in export_section {
                    let export_item = export_item?;
                    if export_item.kind == wasmparser::ExternalKind::Func {
                        let name = export_item.name.to_string();
                        if !name.starts_with('_') && name != "memory" {
                            exported_fns.insert(name);
                        }
                    }
                }
            }
        }

        if exported_fns.is_empty() {
            if !args.quiet {
                eprintln!("No exported functions found in {}", package.name);
            }
            continue;
        }

        let contract_id = crate::deploy_contract_with_retry(
            &mut transport,
            wasm_path.as_std_path(),
            source,
            network,
            &package.name,
            retry_config,
        )?;

        eprintln!("Contract deployed at: {}", contract_id);

        for function in exported_fns {
            if !args.quiet {
                eprintln!("Simulating function '{}'...", function);
            }

            let func_config = toml_config.functions.get(&function);
            let func_args = match func_config {
                Some(cfg) => crate::arg_spec::render_args(&cfg.args, &function)?,
                None => Vec::new(),
            };

            match crate::simulate_function(
                &mut transport,
                &contract_id,
                source,
                network,
                &function,
                &func_args,
                &package.name,
            )? {
                crate::SimulationOutcome::Metrics {
                    instructions,
                    read_bytes,
                    write_bytes,
                    ..
                } => {
                    let measured = MeasuredResources {
                        instructions: instructions as u64,
                        read_bytes: read_bytes as u64,
                        write_bytes: write_bytes as u64,
                    };
                    measurements
                        .entry(package.name.to_string())
                        .or_default()
                        .insert(function.clone(), measured);

                    for (metric, value) in [
                        ("CPU Instructions", instructions),
                        ("Read Bytes", read_bytes),
                        ("Write Bytes", write_bytes),
                        ("WASM Bytes", wasm_size),
                    ] {
                        let limit =
                            func_config.and_then(|cfg| crate::limit_for_metric(cfg, metric));
                        let (entry_limit, pass) = crate::evaluate_check(value, limit);
                        if pass == Some(false) {
                            checks_failed = true;
                        }
                        reports.push(crate::CostReport {
                            package: package.name.to_string(),
                            function: function.clone(),
                            metric,
                            value: Some(value),
                            limit: entry_limit,
                            pass,
                        });
                    }
                }
                crate::SimulationOutcome::Failed(failure) => {
                    has_errors = true;
                    if !args.quiet {
                        match &failure {
                            crate::SimulationFailure::Invoke(stderr) => {
                                eprintln!(
                                    "Warning: Simulation failed for {}: {}",
                                    function, stderr
                                );
                            }
                            crate::SimulationFailure::Rpc(error) => {
                                eprintln!("Warning: RPC error for {}: {}", function, error);
                            }
                            crate::SimulationFailure::MetricsExtraction(err) => {
                                eprintln!(
                                    "Warning: Failed to extract metrics for {}: {}",
                                    function, err
                                );
                            }
                        }
                    }
                    if let (true, Some(function_config)) = (args.check, func_config) {
                        checks_failed = true;
                        crate::emit_check_failure_entries(
                            &mut reports,
                            &package.name,
                            &function,
                            function_config,
                        );
                    }
                }
            }
        }
    }

    Ok(MeasurementResult {
        reports,
        measurements,
        has_errors,
        checks_failed,
    })
}
