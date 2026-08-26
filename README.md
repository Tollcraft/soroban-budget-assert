<div align="center">
  <h1>🛡️ Soroban Budget Assert</h1>
  <p><strong>Empirical cost measurement and assertion tooling for Soroban smart contracts.</strong></p>
  
  [![Build Status](https://github.com/Tollcraft/soroban-budget-assert/actions/workflows/budget.yml/badge.svg)](https://github.com/Tollcraft/soroban-budget-assert/actions/workflows/budget.yml)
  [![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
  <p>
    <a href="https://tollcraft.gitbook.io/docs/budget-assert"><strong>Documentation</strong></a> ·
    <a href="https://tollcraft.github.io/soroban-budget-assert/dashboard.html"><strong>Dashboard</strong></a> ·
    <a href="https://asciinema.org/a/qqC0RysuCDBvfUXC"><strong>Demo</strong></a>
  </p>
</div>

---

## 📖 Overview

[![asciicast](https://asciinema.org/a/qqC0RysuCDBvfUXC.svg)](https://asciinema.org/a/qqC0RysuCDBvfUXC)

`soroban-budget-assert` is a developer tool that measures the gap between local Soroban test estimates and real network costs. It allows developers to assert budget limits during testing and automatically generate detailed execution-resource reports across an entire workspace.

### 🏗️ Architecture

The tool is split into two primary components:

1. **`budget-macros` (Tier A - Local, Fast, CI-Blocking)**
   - Rust macros (`#[budget_cpu_lt(N)]`, `#[budget_mem_lt(N)]`, `#[budget_read_bytes_lt(N)]`, `#[budget_write_bytes_lt(N)]`) applied directly to your test functions.
   - Fails the test the moment measured cost crosses your pinned limit, so cost regressions are caught in CI instead of on the network.

2. **`cargo-budget-report` (Tier B - Network-Verified, Reporting)**
   - A CLI tool that automatically discovers all contracts in your workspace.
   - Compiles WASM, simulates execution on testnet, and reports the simulated resource amounts (CPU instructions, read/write bytes) plus the compiled WASM binary size.
   - These are inputs to the non-refundable resource fee — not a total cost. Rent, refundable fees, transaction size, footprint entry counts, and the inclusion fee are not measured; see [Measurement scope](https://tollcraft.gitbook.io/docs/budget-assert/reference#measurement-scope).
   - Configurable via a central `budget.toml` file.

### 🧪 Test Fixture: Constant-Product AMM Pool

The workspace includes `amm-pool-contract`, a constant-product AMM pool fixture that replaces the original `ExpensiveContract` synthetic loop. It exercises the operations that dominate real Soroban costs:

- **Multiple persistent storage keys** — reserves, balances, LP shares, per-user state
- **Authorization** — `require_auth()` on every state-changing operation
- **Event emission** — deposit, swap, and withdraw events
- **Realistic computation** — constant-product math with slippage checks
- **Simulated token flows** — internal balance tracking across pool operations

The fixture is a benchmark, not a product. It implements `initialize`, `deposit`, `swap`, and `withdraw` — enough to produce meaningful cost numbers but small enough to stay readable.

**`do_expensive_work`** is retained as a deliberately named synthetic baseline. Its CPU-bound loop exercises almost none of the host functions that drive real contract costs, making it useful as a comparison point to measure the gap between synthetic benchmarks and realistic contract operations.

## 📊 Cost-over-time Dashboard

Every push to `main` runs [`budget.yml`](.github/workflows/budget.yml), whose `record-history` job appends a `{commit, timestamp, data}` entry to `history.json` on the `gh-pages` branch — but only when the uploaded report is a genuine network-measured measurement. The job inspects the report itself (every recorded function must carry all four metric rows with numeric values, and the known demo placeholder is rejected verbatim); anything else is declined without failing the run. Entries already in `history.json` that fail the same check (legacy mocked points) are purged on the next push to `main`. The static dashboard at [`site/dashboard.html`](site/dashboard.html) (published by [`deploy-site.yml`](.github/workflows/deploy-site.yml)) fetches that file at page load and plots per-function trend lines, so a regression like "`do_expensive_work` got 12% more expensive over the last ten commits" is visible at a glance — and every point on it is real.

**How the pieces fit together:**
1. `record-history` job → verifies the report is a real measurement, purges non-measured legacy entries, then appends to `history.json` on `gh-pages`.
2. `deploy-site.yml` → publishes `site/**` to `gh-pages` with `keep_files: true`, so `history.json` is never wiped.
3. The dashboard page fetches `history.json` same-origin and pivots it client-side into `package → function → metric` series — no backend, no build-time data baking.

**Using this on your own repo:** copy the `record-history` job pattern and the `site/` folder into your repo, then open the dashboard with query params:
- `?history=URL` — where to fetch `history.json` from (default `./history.json`, same-origin).
- `?repo=owner/name` — links each point to its commit on GitHub (auto-detected on `<owner>.github.io/<repo>/` URLs; set explicitly for custom domains/forks).
- `?limit=N` — how many recent commits to render (default 200).

Example: `https://your-org.github.io/your-repo/dashboard.html?limit=100`.

## ⚙️ Supported Versions & Compatibility

The workspace does not pin a single `soroban-sdk` version — the two contract fixtures pin different ones, and the reporting CLI depends on the XDR decoder rather than the SDK. The manifest dependencies (the source of truth for intent; `Cargo.lock` only records resolution) are:

| Crate | Manifest dependency |
| :--- | :--- |
| `amm-pool-contract` ([manifest](amm-pool-contract/Cargo.toml)) | `soroban-sdk` = `"22.0.11"` (also as a dev-dependency with the `testutils` feature) |
| `host-function-contract` ([manifest](host-function-contract/Cargo.toml)) | `soroban-sdk` = `"22.0.0"` |
| `cargo-budget-report` ([manifest](cargo-budget-report/Cargo.toml)) | `stellar-xdr` = `"22.1.0"` (used for decoding transaction simulation responses; no direct `soroban-sdk` dependency) |

* **Corresponding Stellar Protocol**: **Protocol 22**

### What "Supported" means here

A row marked **Supported** means: every workspace crate builds and passes `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` in CI on each push/PR to `main` ([quality.yml](.github/workflows/quality.yml), [budget.yml](.github/workflows/budget.yml)) — which compiles both pinned `soroban-sdk` versions and runs the Tier A macro test suite against the `amm-pool-contract` fixture. Additionally, the network-side figures in [MEASUREMENTS.md](MEASUREMENTS.md) were captured manually against Soroban testnet with these pins (one-time verification, not continuous CI). A row marked **Untested** has neither CI coverage nor manual verification.

### Compatibility Matrix

| SDK Version | Protocol Version | Status | Notes |
| :--- | :--- | :--- | :--- |
| **`< 22.0.0`** | `< 22` | **Untested** | Older protocols may use different transaction/resource schemas. |
| **`22.0.x`** | `22` | **Supported** | Matches all pinned manifest dependencies: `soroban-sdk` `22.0.11` (`amm-pool-contract`), `soroban-sdk` `22.0.0` (`host-function-contract`), `stellar-xdr` `22.1.0` (`cargo-budget-report`). Note that the two fixtures pin different patch versions of the same minor line; whether to unify them is a separate concern not covered by this table. |
| **`>= 23.0.0`** | `>= 23` | **Untested** | Future protocol upgrades or XDR schema changes (e.g. key/field renames) may break parsing. Migration to newer SDK/XDR versions is actively worked on — see [#382](https://github.com/Tollcraft/soroban-budget-assert/issues/382) (soroban-sdk/stellar-xdr migration) and [#383](https://github.com/Tollcraft/soroban-budget-assert/issues/383) (wasmparser migration) — so the project is not stuck on protocol 22. |

---

## 🚀 Quick Start

### 1. Installation

Install from [crates.io](https://crates.io/crates/cargo-budget-report) (recommended):
```bash
cargo install cargo-budget-report
```

Alternatively, build from source:
```bash
cargo install --path cargo-budget-report
```

### 2. Configuration
Scaffold a `budget.toml` in your workspace root:
```bash
cargo budget-report --init
```

This writes a commented template with all available fields and an example
function entry. Review and adjust the values for your project.

To overwrite an existing file, add `--force`:
```bash
cargo budget-report --init --force
```

The `budget.toml` file is shared between both Tollcraft tools —
`cargo-budget-report` and `soroban-cost-linter` — so a single file at the
workspace root serves both tools. Each tool silently ignores sections it
does not own. Unknown keys inside `[functions.*]` blocks produce an error
pointing to the offending key.

Full shared schema:

```toml
# -- cargo-budget-report configuration ----------------------------------------
network = "testnet"           # Target network: "testnet", "futurenet", "local"
source = "alice"              # Stellar source account keypair name

[functions.do_expensive_work]
args = ["--n", "10000"]       # CLI arguments forwarded to the function
cpu_limit = 5000000           # Optional CPU instruction limit (--check)
read_limit = 5000             # Optional read-bytes limit (--check)
write_limit = 1000            # Optional write-bytes limit (--check)

# -- soroban-cost-linter configuration ----------------------------------------
[lints]                       # Consumed by soroban-cost-linter; silently
complexity = "warn"           # accepted by cargo-budget-report.
```

### 3. Usage

**Generate a Workspace Report:**
```bash
cargo budget-report
```

**Use the same release profile for comparable numbers:**

`cargo budget-report` builds contracts with `cargo build --release --target wasm32-unknown-unknown`, so the workspace's `[profile.release]` changes the WASM that gets deployed and simulated. The figures published by this project use the Soroban size-optimized release profile below; copy it into the workspace root before comparing your results to this repo's measurements:

```toml
[profile.release]
opt-level = "z"
overflow-checks = true
debug = 0
strip = "symbols"
debug-assertions = false
panic = "abort"
codegen-units = 1
lto = true
```

These settings are measurement inputs, not cosmetic preferences. `opt-level = "z"` and `lto = true` optimize the generated WASM for size and cross-crate inlining; `codegen-units = 1` gives LLVM a whole-program optimization view; `panic = "abort"` removes unwinding code; `strip = "symbols"` and `debug = 0` remove symbol/debug payload from the artifact; `debug-assertions = false` matches production release behavior; and `overflow-checks = true` keeps arithmetic checks explicit when the release build is measured. Changing any of them can change CPU instructions, memory usage, read/write bytes, or WASM size.

Figures produced under a different release profile are different builds and are not comparable to this project's published cost figures. In the existing fixture, `do_expensive_work(10_000)` measured 901,816 local WASM CPU instructions and 756,678 testnet instructions with the size-optimized profile, but 767,049 local WASM CPU instructions and 832,006 testnet instructions with Cargo's default release profile. A follow-up worth considering is a tool warning when `cargo budget-report` runs in a workspace that lacks these settings.

**Enforce Regression Limits (`--check`):**

Add per-function `cpu_limit`, `read_limit`, and/or `write_limit` to `budget.toml`.
Then run `cargo budget-report --check` — the measured metrics are compared against
the configured limits, a clear pass/fail line is printed per function+metric, and
the process exits non-zero on any breach (or on any configured function whose
simulation fails to run). Functions not declared in `budget.toml` are still
reported but never checked.

```toml
# budget.toml
network = "testnet"
source = "alice"

[functions.do_expensive_work]
args = ["--n", "10000"]
cpu_limit = 5000000
read_limit = 5000
write_limit = 1000
```

```bash
# Plain text report + per-check pass/fail:
cargo budget-report --check

# Same, with machine-readable JSON entries that include `limit` and `pass`
# fields per configured function+metric:
cargo budget-report --check --json

# Exit on the first violation instead of collecting all results:
cargo budget-report --check --fail-fast
```

**Target a local / standalone RPC node (`--rpc-url`):**

By default the tool simulates against the public `testnet` / `futurenet`
endpoints. To point it at a local standalone Soroban RPC node instead — to
avoid public-network rate limits, or to exercise custom network fee settings
— pass `--rpc-url` together with `--network-passphrase` (both are required
together):

```bash
cargo budget-report \
  --rpc-url http://localhost:8000/soroban/rpc \
  --network-passphrase "Standalone Network ; February 2017"
```

When set, `simulateTransaction` is POSTed to that endpoint, and the
`stellar` deploy / invoke-build calls are pointed at it with `--rpc-url` /
`--network-passphrase` in place of `--network`. `--network` / the
`budget.toml` `network` field are then optional; the passphrase is used as
the network label (deploy-cache key).

**Reuse deployments between runs (deploy cache):**

Deployed contract ids are cached in `.budget-cache.toml` (git-ignored),
keyed on the compiled wasm's SHA-256, the network, and the source account.
An unchanged build is not redeployed; any change to the wasm/network/source
redeploys automatically. Force a redeploy with `cargo budget-report
--no-deploy-cache` or by deleting `.budget-cache.toml`.

**Signing key without the CLI key store (`--source-secret`):**

`--source-secret` (or the `STELLAR_SECRET_KEY` env var) supplies the source
account's `S...` seed directly. It is validated at preflight. Deploy and
invoke-build still go through the `stellar` CLI today; native RPC
deploy/invoke that consume this key are a work in progress (issue #123).


**Use Macros in Tests:**

The macros (`budget_cpu_lt`, `budget_mem_lt`, `budget_read_bytes_lt`, `budget_write_bytes_lt`) are attribute macros for test functions. They require a local variable named **`env`** — the generated code reads `env.cost_estimate().budget()` by name.

```rust
use budget_macros::{budget_cpu_lt, budget_mem_lt, budget_read_bytes_lt, budget_write_bytes_lt};
use soroban_sdk::Env;

// CPU instruction assertion. The limit is read at test runtime from a
// `KEY=VALUE` file generated by `cargo budget-report --derive-limits`
// (see the "Deriving Tier A limits from a Tier B report" section below).
#[test]
#[budget_cpu_lt(env_file = "../tier-a-limits.env",
               env = "TIER_A__AMM_POOL_CONTRACT__SCENARIO__FULL_WORKFLOW__CPU")]
fn test_cpu_budget() {
    let env = Env::default();
    let contract_id = env.register(ConstantProductPool, ());
    let client = ConstantProductPoolClient::new(&env, &contract_id);
    // ... initialize + reset_unlimited + deposit + swap + withdraw ...
}
```

The macros also accept a literal integer, an `env = "VAR"` (process
environment), and `config = "key"` (a `budget.json` file in the
working directory); see `budget-macros/src/lib.rs` rustdoc for the full
form catalogue. The `env_file` form is the recommended form for
network-derived limits because it is thread-safe and review-friendly.

---

## 📊 Measurements

The [MEASUREMENTS.md](MEASUREMENTS.md) file at the repository root records all empirical cost measurements comparing local Soroban budget estimates against real network costs. The [Protocol Mechanics documentation](https://tollcraft.gitbook.io/docs/budget-assert/mechanics) cites this file as the source of truth for measured figures.


## 🤝 Community & Maintainers

Join the discussion and get support:
* **Community Link**: [Stellar Developer Discord](https://discord.gg/5aprtMSyR)

| Maintainer | Role | Telegram |
|------------|------|----------|
| Tollcraft Team | Core Developers | [@tollcraft](https://t.me/+Gflo5jZStw1jMjE0) |

---

## 🛠️ Contributing

We welcome contributions! Please see our [CONTRIBUTING.md](CONTRIBUTING.md) for details on how to get started, and our [SECURITY.md](SECURITY.md) for reporting vulnerabilities.

### 🧑‍💻 Contributors

[![Contributors](https://contrib.rocks/image?repo=Tollcraft/soroban-budget-assert)](https://github.com/Tollcraft/soroban-budget-assert/graphs/contributors)
