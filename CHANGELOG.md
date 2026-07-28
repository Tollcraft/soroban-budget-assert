# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

- **Tier A limit derivation from a Tier B report (`feat/derive-tier-a-limits-from-tier-b`).** `cargo budget-report --derive-limits <OUT> --from <TIER_B_JSON>` reads the JSON emitted by `cargo budget-report --json` (or stdin), applies per-metric `cpu` / `memory` / `read` / `write` margins supplied on the CLI or via a `[margin]` block in `budget.toml`, and emits a `KEY=VALUE` artifact whose env keys a Tier A test can read directly. A sidecar `<OUT>.provenance.md` records each `tier_b_value × margin = tier_a_limit` row alongside a header in `<OUT>` itself. Both writes are atomic. The derivation does not need the `stellar` CLI, a built WASM, or network access, so it slots into CI alongside `--record-baseline` / `--check-baseline`.
- **`env_file = "PATH"` form of the budget macros.** Companion to the existing `env = "VAR"` / `config = "key"` / integer-literal forms. Tests can read a checked-in `KEY=VALUE` file at runtime, so a single `tier-a-limits.env` at the workspace root drives every Tier A assertion without per-test env-var plumbing. Per-assertion reads make this form thread-safe in concurrent test runners, no `unsafe std::env::set_var` is needed.
- **`[scenarios.<name>]` block in `budget.toml`.** Maps a single Tier A scenario (e.g. "full_workflow = deposit + swap + withdraw") to the component functions whose Tier B values are summed into one Tier A limit. The derived key prefixes with `SCENARIO__<name>` so it cannot collide with a per-function limit on the same package.
- Workspace-root `tier-a-limits.env` and `tier-a-limits.provenance.md` artifacts for the AMM pool fixture (`amm-pool-contract/tests/budget_test.rs`). The nine hand-tuned limits in that test are replaced by `env_file = "../tier-a-limits.env", env = "..."` annotations, and the stale hand-written reconciliation comments (`// Re-measured: WASM local 2770850…` / `// WASM local 901816, actual testnet ~756678…`) are dropped in favour of the auto-generated provenance sidecar.
- New `cargo-budget-report/fixtures/tier_b_report_sample.json` fixture used as the input to `cargo budget-report --derive-limits` in tests and ad-hoc CI smoke runs.
- **New `module_25` — hash-optimized internal state tracking (`optimize-internal-state-tracking-structures`, issue #233).** Adds a [`module_25`] to `soroban-budget-assert-core` that tracks per-operation cost snapshots via a [`StateTracker`] trait with two pluggable backends: [`LinearStateTracker`] (pre-optimization `Vec<(String, CostSnapshot)>` baseline) and [`HashedStateTracker`] (post-optimization `HashMap<String, CostSnapshot>`). Both backends share an identical public API — `record` / `lookup` / `contains` / `len` / `is_empty` / `total_cpu` / `total_memory` / `ops` / `clear` — so call sites swap by changing one type. A `compare_backends` function runs a paired before/after benchmark (`record_speedup`, `lookup_speedup`) that quantifies the optimization on the same workload. 22 new unit tests cover correctness, parity between backends, and the optimization invariant (`hashed` beats `linear` at every scale).

### Changed

- Tier A reconciliation comments in `amm-pool-contract/tests/budget_test.rs` are now auto-generated from `tier-a-limits.provenance.md`, not transcribed by hand. Re-derive the artifact instead of editing the test inline when a limit needs to change.
- `src/lib.rs` re-exports `module_25` alongside the existing `module_1`.

### Notes

- The per-metric split (cpu / memory / read / write), not per-operation-type, is the grain of this change. **Issue #45** (per-operation-type margin) would slot in alongside `[scenarios]` as `keys: HashMap<FunctionKey, Margin>` in `DerivationConfig`; no macro change required.
- **Issue #10** (baseline / regression mode) already coexists: `--record-baseline` captures the Tier B pin, `--check-baseline` enforces tolerance, `--derive-limits` produces the Tier A artifact consumers. The suggested workflow is `--derive-limits` first (Tier A), then `--record-baseline` once the Tier A lands so the next rerun sees both sides.

## Unreleased (prior)

### Added

- Retry mechanism for friendbot funding during contract deployment: `cargo budget-report` now automatically retries `stellar contract deploy` up to 3 additional times (4 total attempts) with exponential backoff (2s → 4s → 8s) when friendbot funding is suspected to have failed transiently due to rate-limiting or network latency. This reduces CI flakes and manual re-runs when using testnet.

- `cargo budget-report --csv` flag: emits the budget report as CSV instead of a table or JSON. Without `--check`, produces four columns (`package`, `function`, `metric`, `value`); with `--check`, produces six columns (`package`, `function`, `metric`, `value`, `limit`, `pass`). Includes simulation-failure rows in `--check` mode so CI consumers see every configured function. Composes with `--check` and can replace `--json` in shell pipelines that prefer CSV. enforces per-function `cpu_limit`, `read_limit`, and `write_limit` declared in `budget.toml` against network-verified simulation costs. Prints a pass/fail line per function+metric and exits non-zero on any breach (or on any configured function whose simulation fails). Compiles with `--json` so entries gain `limit` and `pass` fields; the plain text and JSON output stay byte-for-byte identical to previous releases when `--check` is not passed.
- Per-function `cpu_limit`, `read_limit`, and `write_limit` fields on `[functions.<name>]` entries in `budget.toml`. Any field omitted means the metric is reported but not enforced.
- Single-page landing site under `site/` with empirical cost-gap breakdown, two-tier architecture overview, quick-start guide, and project resources.
- Updated GitHub Actions Pages deployment workflow to serve static site files from `./site`.
- Budget macros now support reading thresholds from a `budget.json` config file via the `config = "key"` attribute syntax, e.g. `#[budget_cpu_lt(config = "cpu_instructions")]`. Falls back to `u64::MAX` when the file is missing or the key is not found.
- Comprehensive unit tests for the cost-value formatter covering zero, single digits, thousands/millions boundaries, and `u32::MAX` across both unit suffixes.
- Contributors should add a short changelog entry with their pull request when the change is user-visible.
- Budget assertion tests for `require_auth` host calls: isolated `require_auth_only` contract function with CPU/memory budget assertions, plus per-operation deposit/swap/withdraw granular budget checks.
- Budget assertion tests for `extend_ttl` operations: isolated `extend_instance_ttl` contract function with CPU/memory budget assertions and deliberate-regression fixtures, demonstrating how to budget-test ledger-rent operations.

### Fixed

- Dynamic env-var budget limits (`env = "VAR"`) now panic with a clear message when the variable is set but contains an unparseable value (e.g. `1_000_000` or `"800000 "`), instead of silently falling back to `u64::MAX` and disabling the assertion.

## [0.1.0] - 2026-07-24

### Added

- Budget assertion macros for local test-time cost checks:
  - `#[budget_cpu_lt(N)]`
  - `#[budget_mem_lt(N)]`
- A workspace reporting CLI, `cargo budget-report`, that discovers Soroban contracts, builds them to WASM, deploys them to the configured network, simulates exported functions, and reports actual non-refundable execution costs.
- `budget.toml` support for configuring the target network, source account, and per-function invoke arguments.
- JSON output support for CI and automation workflows.
- GitHub Actions integration for publishing budget history data to the repository's `gh-pages` history dataset.

### Changed

- Improved the user-facing CLI output to surface the network-verified execution metrics the project uses for budget decisions.

### Notes

- The current crate version numbers declared in the workspace manifests are `0.1.0`, so the initial changelog entry uses the same version number.

