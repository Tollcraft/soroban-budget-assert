# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

- `cargo budget-report --check` flag: enforces per-function `cpu_limit`, `read_limit`, and `write_limit` declared in `budget.toml` against network-verified simulation costs. Prints a pass/fail line per function+metric and exits non-zero on any breach (or on any configured function whose simulation fails). Compiles with `--json` so entries gain `limit` and `pass` fields; the plain text and JSON output stay byte-for-byte identical to previous releases when `--check` is not passed.
- Per-function `cpu_limit`, `read_limit`, and `write_limit` fields on `[functions.<name>]` entries in `budget.toml`. Any field omitted means the metric is reported but not enforced.
- Single-page landing site under `site/` with empirical cost-gap breakdown, two-tier architecture overview, quick-start guide, asciinema demo embed, and project resources.
- Updated GitHub Actions Pages deployment workflow to serve static site files from `./site`.
- Contributors should add a short changelog entry with their pull request when the change is user-visible.

### Changed

- Replaced the stringly-typed `metric` field in `cargo-budget-report` with a typed `Metric` enum. The unit for each metric now comes from the enum variant rather than substring matching on the display name. JSON output keys are unchanged.

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
