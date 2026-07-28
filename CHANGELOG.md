# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

- `Memory Bytes` metric in `cargo budget-report`: now extracts `result.cost.memBytes` from the `simulateTransaction` JSON-RPC payload (Soroban protocol 22's `SimulateTransactionResponse.cost.memBytes`) alongside the existing XDR-derived instructions / read / write metrics. The row is omitted when an older RPC server does not return a `cost` object so the three proven metrics are never substituted with a proxy.
- New optional `mem_limit` per-function limit on `[functions.<name>]` entries in `budget.toml`, enforced by `cargo budget-report --check` against the network's `result.cost.memBytes` figure. Missing field means reporting-only, matching the existing `cpu_limit` / `read_limit` / `write_limit` semantic.
- New `allocate_vec(env, n)` function on `ConstantProductPool` in `amm-pool-contract`: builds a `Vec<u32>` of length `n` and persists it under instance storage. No auth required, so it's `cargo budget-report`-callable end-to-end on Soroban testnet. Used as the memory-allocation fixture for the #122 measurement series.
- Local-measurement test `test_measure_memory_bytes_local_for_issue_122` in `amm-pool-contract/tests/budget_test.rs` that prints `Env::cost_estimate().budget().memory_bytes_cost()` for both raw-Rust and WASM forms of `allocate_vec(100_000)`. Run with `cargo test -- --nocapture` to capture the figure that pairs with the network-side `cost.memBytes` returned by `cargo budget-report`.
- Retry mechanism for friendbot funding during contract deployment: `cargo budget-report` now automatically retries `stellar contract deploy` up to 3 additional times (4 total attempts) with exponential backoff (2s → 4s → 8s) when friendbot funding is suspected to have failed transiently due to rate-limiting or network latency. This reduces CI flakes and manual re-runs when using testnet.

- `cargo budget-report --csv` flag: emits the budget report as CSV instead of a table or JSON. Without `--check`, produces four columns (`package`, `function`, `metric`, `value`); with `--check`, produces six columns (`package`, `function`, `metric`, `value`, `limit`, `pass`). Includes simulation-failure rows in `--check` mode so CI consumers see every configured function. Composes with `--check` and can replace `--json` in shell pipelines that prefer CSV. enforces per-function `cpu_limit`, `read_limit`, and `write_limit` declared in `budget.toml` against network-verified simulation costs. Prints a pass/fail line per function+metric and exits non-zero on any breach (or on any configured function whose simulation fails). Compiles with `--json` so entries gain `limit` and `pass` fields; the plain text and JSON output stay byte-for-byte identical to previous releases when `--check` is not passed.
- Per-function `cpu_limit`, `read_limit`, and `write_limit` fields on `[functions.<name>]` entries in `budget.toml`. Any field omitted means the metric is reported but not enforced.
- Single-page landing site under `site/` with empirical cost-gap breakdown, two-tier architecture overview, quick-start guide, asciinema demo embed, and project resources.
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
