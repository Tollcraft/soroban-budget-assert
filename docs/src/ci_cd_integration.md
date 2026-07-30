# CI/CD Integration with GitHub Actions

This page is a reference for integrating **soroban-budget-assert** into a **GitHub Actions CI/CD pipeline**. It covers the complete example workflow, explains each step, and provides guidance on customization and troubleshooting.

For a step-by-step tutorial that walks through the full setup from scratch, see the [End-to-End CI Tutorial](ci_tutorial.md).

---

## Purpose

### Why budget assertions should run in CI

A contract that passes tests locally can still exceed resource limits on the network. The gap between local Soroban WASM estimates and real network costs means that a cost regression can go unnoticed until a transaction fails on testnet or, worse, on mainnet. Running budget assertions in CI catches regressions before they reach the network.

### Benefits of automated budget regression detection

- **Early feedback**: a pull request that pushes a function past its budget fails the CI check immediately, with the exact metric and limit in the log.
- **Audit trail**: the measured costs are captured as artifacts or step summaries, so the review history includes the budget data.
- **Consistent baselines**: Tier A macro assertions (`#[budget_cpu_lt]`, `#[budget_mem_lt]`) pin a specific limit into `cargo test`, making the pass/fail boundary reviewable in the same diff as the contract change.

### When to include budget validation

Include budget validation whenever the contract source, the release profile, or the Soroban SDK version changes. The usual pipeline rules apply:

- **Every push to `main`**: record the budget report for the cost-over-time dashboard.
- **Every pull request targeting `main`**: run Tier A assertions as a required status check.
- **Periodically (or on SDK bumps)**: re-derive Tier A limits from a fresh Tier B network report.

---

## GitHub Actions Example

The following workflow runs both tiers of budget validation in a single CI pipeline. It is the same workflow this repository uses on every push and pull request.

### Example Workflow File

Save this as `.github/workflows/budget.yml` in your repository:

{% code title=".github/workflows/budget.yml" %}
```yaml
name: Soroban Budget Check

on:
  push:
    branches: ["main"]
  pull_request:
    branches: ["main"]

permissions:
  contents: read

jobs:
  budget-check:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4.2.2
        with:
          fetch-depth: 0

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: 1.85.0
          targets: wasm32v1-none wasm32-unknown-unknown

      - name: Cache Rust Dependencies
        uses: Swatinem/rust-cache@v2

      - name: Install System Dependencies
        run: sudo apt-get update && sudo apt-get install -y libdbus-1-dev pkg-config libudev-dev

      - name: Install Stellar CLI
        run: |
          curl -sL https://github.com/stellar/stellar-cli/releases/download/v21.5.3/stellar-cli-21.5.3-x86_64-unknown-linux-gnu.tar.gz | tar -xz
          mv stellar ~/.cargo/bin/

      - name: Configure Stellar Identity
        run: stellar keys add alice --secret-key "${{ secrets.ALICE_SECRET_KEY }}"
        env:
          STELLAR_ACCOUNT: alice

      - name: Check formatting
        run: cargo fmt --all -- --check

      - name: Run Clippy
        run: cargo clippy --workspace --all-targets -- -D warnings

      - name: Build Contracts
        run: cargo build -p amm-pool-contract --release --target wasm32v1-none

      - name: Run Budget Macros Test (Tier A)
        run: cargo test --workspace

      - name: Run Budget Report (Tier B)
        run: |
          cargo run --bin cargo-budget-report -- budget-report --json > current_report.json

      - name: Publish Step Summary
        run: |
          cargo run --bin cargo-budget-report -- budget-report --format md >> "$GITHUB_STEP_SUMMARY"

      - name: Upload Budget Report
        uses: actions/upload-artifact@v4.2.0
        with:
          name: budget-report
          path: current_report.json
```
{% endcode %}

---

## Explanation of Each Step

### `actions/checkout@v4`
Checks out the repository so subsequent steps have access to the source code. `fetch-depth: 0` ensures full Git history is available for operations like commit comparison and `record-history`.

### `dtolnay/rust-toolchain`
Installs the Rust toolchain pinned to the version in your `rust-toolchain.toml`. The `targets` argument installs `wasm32v1-none` (Soroban target) and `wasm32-unknown-unknown` (fallback target). The toolchain version must match the one in `rust-toolchain.toml` or you may get cryptic build errors.

### `Swatinem/rust-cache`
Caches compiled Rust dependencies between runs. Speeds up subsequent workflow executions significantly. The cache key is derived from `Cargo.lock` and the toolchain version, so it invalidates automatically when dependencies change.

### Install System Dependencies
Installs `libdbus-1-dev`, `pkg-config`, and `libudev-dev`. These are required by the Soroban SDK's system dependencies on Linux. Without them, `cargo build` fails with linker errors.

### Install Stellar CLI
Downloads and installs the prebuilt `stellar` CLI binary. The workflow uses a tarball rather than `cargo install` to avoid a multi-minute Rust build. The version must match the SDK version this project's contracts were built against.

### Configure Stellar Identity
Imports the testnet secret key as a Stellar CLI identity named `alice`. This identity is used by `cargo budget-report` to deploy and invoke contracts on testnet. The secret key is stored as a GitHub Actions secret (`ALICE_SECRET_KEY`).

### Check formatting
Runs `cargo fmt --all -- --check` to verify code formatting. Fails the build if the code does not match the repository's `rustfmt.toml` configuration. This enforces consistent code style across all contributors.

### Run Clippy
Runs `cargo clippy --workspace --all-targets -- -D warnings` to lint the entire workspace. Every warning is treated as an error (`-D warnings`), so no lint slips through. This enforces the project's code quality standards.

### Build Contracts
Builds the contract WASM with the release profile. This step compiles the Soroban contract(s) to WASM so that:
- The Tier A macro tests can load and execute the WASM (not raw Rust).
- The Tier B budget report can simulate deployment on testnet.

Use `cargo build -p <your-package> --release --target wasm32v1-none` for a single package, or `cargo build --workspace --release --target wasm32v1-none` for multiple.

### Run Budget Macros Test (Tier A)
Runs `cargo test --workspace` to execute all tests, including those annotated with `#[budget_cpu_lt]` and `#[budget_mem_lt]`. This is the fast, local, CI-blocking gate: if a function's measured CPU or memory exceeds its pinned limit, the test fails with the exact metric and limit in the output.

### Run Budget Report (Tier B)
Runs `cargo run --bin cargo-budget-report -- budget-report --json` to produce a network-verified resource cost report. The JSON output is saved to `current_report.json`. This step deploys the contract WASM to testnet and simulates each configured function, returning the real resource costs.

The `--json` flag produces machine-readable output suitable for artifact upload and further processing. Use `--check` to enforce limits configured in `budget.toml`.

### Publish Step Summary
Runs `cargo run --bin cargo-budget-report -- budget-report --format md` and appends the Markdown table to `$GITHUB_STEP_SUMMARY`. The table appears inline on the workflow run page and, on pull requests, in the PR's Summary section. This makes the budget data visible without downloading an artifact.

### Upload Budget Report
Uploads `current_report.json` as a workflow artifact named `budget-report`. The artifact can be downloaded from the run page for manual inspection or downstream processing (e.g., the cost-over-time dashboard's `record-history` job).

---

## Customization

### Running on multiple Rust versions

If your project supports multiple Rust toolchains, use a build matrix:

```yaml
strategy:
  matrix:
    toolchain: ["1.85.0", "stable"]

steps:
  - name: Install Rust
    uses: dtolnay/rust-toolchain@stable
    with:
      toolchain: ${{ matrix.toolchain }}
      targets: wasm32v1-none wasm32-unknown-unknown
```

Note that changing the Rust version may produce different WASM and therefore different budget numbers. The pinned version in `rust-toolchain.toml` is what this project's measurements are based on.

### Limiting execution to pull requests

To run budget checks only on pull requests (not on every push to `main`), remove the `push` trigger:

```yaml
on:
  pull_request:
    branches: ["main"]
```

### Running only on selected branches

To limit execution to specific branches:

```yaml
on:
  push:
    branches: ["main", "develop"]
  pull_request:
    branches: ["main", "develop"]
```

### Integrating with existing CI pipelines

You can merge the budget check into an existing workflow file by copying the relevant steps. The minimum required steps for Tier A (local, CI-blocking) are:

```yaml
- name: Build Contracts
  run: cargo build -p your-contract --release --target wasm32v1-none
- name: Run Budget Macros Test
  run: cargo test --workspace
```

Add the Tier B steps when you need network-verified cost measurements and have configured a testnet identity with the `ALICE_SECRET_KEY` secret.

### Fork-safe fallback for Tier B

Pull requests from forks do not have access to repository secrets. To keep the workflow green on fork PRs while still running Tier B on push events, split the report step:

```yaml
- name: Run Budget Report (push)
  if: github.event_name == 'push'
  env:
    ALICE_SECRET_KEY: ${{ secrets.ALICE_SECRET_KEY }}
  run: cargo run --bin cargo-budget-report -- budget-report --json > current_report.json

- name: Run Budget Report (fork / pull request)
  if: github.event_name != 'push'
  run: echo '[{"package":"your-contract","function":"your_function","metric":"CPU Instructions","value":0}]' > current_report.json
```

---

## Best Practices

### Fail builds on budget regressions

Make the `budget-check` job (or its Tier A equivalent) a required status check in your branch protection rules. This prevents merging any pull request that would push a function past its budget.

### Keep budget baselines current

Re-run `cargo budget-report --json` and re-derive Tier A limits whenever:
- The contract source changes.
- The release profile in `Cargo.toml` changes.
- The Soroban SDK version changes.
- The `[margin]` block in `budget.toml` changes.

### Avoid unnecessary workflow duplication

If you already have a CI workflow that builds and tests your contracts, add the budget steps to that existing workflow rather than creating a separate one. The Tier A steps (`cargo build` + `cargo test`) integrate naturally into any Rust CI pipeline.

### Validate changes before merging

Require the `budget-check` job to pass before merging. The branch protection rule for `main` in this repository already requires the `Quality Checks` status check. Add `budget-check` to that list so a budget regression blocks the merge alongside formatting, Clippy, and test failures.

### Use the same release profile

Always build WASM with the same `[profile.release]` settings locally and in CI. The published measurements in this repository use the size-optimized profile (`opt-level = "z"`, `lto = true`, etc.). Numbers from a different profile are not comparable. Copy the profile from `Cargo.toml` into your workspace before recording or comparing budget figures.

---

## Troubleshooting

### Missing toolchain

**Symptom**: The `dtolnay/rust-toolchain` step fails with `error: toolchain 'X.Y.Z' is not installed`.

**Fix**: Verify that the `toolchain:` version in the workflow matches the `channel` in your `rust-toolchain.toml`. If they differ, `rustup` will try to use two different toolchains and may fail. Pin both to the same version.

### Dependency caching problems

**Symptom**: The `Swatinem/rust-cache` step takes a long time or produces a cache miss on every run.

**Fix**: Ensure `Cargo.lock` is checked into the repository. The cache key is derived from `Cargo.lock` contents. Without it, the cache cannot detect dependency changes efficiently. If cache entries grow stale, clear the cache from the GitHub Actions UI (Settings → Actions → Caches).

### Failing budget assertions

**Symptom**: The `cargo test` step fails with a message like:

```
CPU instruction cost 5,400,123 exceeded limit 5,000,000 - local estimate,
real network cost may differ significantly in either direction
```

**Fix**: Re-measure the function's cost with `cargo budget-report` and update the limit in your `budget.toml` or macro annotation. If the increase is expected (e.g., you added a feature), raise the limit consciously. If it is a regression, optimize the function.

### Formatting failures

**Symptom**: The `cargo fmt --all -- --check` step exits non-zero with a diff of formatting changes.

**Fix**: Run `cargo fmt --all` locally, commit the formatting changes, and push again. To catch this before CI, install the pre-commit hook from `scripts/install-hooks.sh`.

### Clippy failures

**Symptom**: The `cargo clippy` step exits non-zero with warnings promoted to errors.

**Fix**: Read the Clippy output to identify the lint violation. Run `cargo clippy --workspace --all-targets` locally to reproduce, fix the issue, and commit. If the lint is a false positive, add an `#[allow(...)]` attribute with a brief justification.

### Test failures

**Symptom**: `cargo test --workspace` fails with test errors unrelated to budget assertions.

**Fix**: Check whether the WASM was built before running tests (`cargo build -p <contract> --release --target wasm32v1-none`). Tests that load contract WASM will fail if the WASM artifact is missing or stale. Rebuild and re-run. If the failure is in a non-budget test, it is a real test break — fix the test or the code it exercises.

### Unfunded or reset testnet accounts

**Symptom**: `cargo budget-report` exits non-zero with `source account may be unfunded` or `txInsufficientBalance` in the error chain.

**Fix**: Re-fund the testnet identity:

```bash
stellar keys fund alice --network testnet
```

Friendbot-funded accounts are reset periodically. If the workflow has been idle for a week, re-fund before debugging further. This is the most common cause of Tier B failures unrelated to the contract code.

### stellar CLI missing on the runner

**Symptom**: The `Install Stellar CLI` step fails with a download error, or `stellar` is not found later in the workflow.

**Fix**: The workflow installs Stellar CLI from a prebuilt tarball. Verify the URL in the `curl` command points to a valid release. Bump the version number when you upgrade the SDK. The tarballs are published under each GitHub release at `https://github.com/stellar/stellar-cli/releases`.

---

## See also

- [End-to-End CI Tutorial](ci_tutorial.md) — step-by-step guide for setting up the full pipeline.
- [End-User Guide](user_guide.md) — installing the tool, configuring `budget.toml`, and writing gated tests.
- [Protocol Mechanics](mechanics.md) — why local estimates differ from network costs.
- [Tool Reference](reference.md) — every CLI flag and macro signature.
- [Developer Guide](developer_guide.md) — building and extending the tool itself.
- [MEASUREMENTS.md](../../MEASUREMENTS.md) — the measured gap between local and network costs.
