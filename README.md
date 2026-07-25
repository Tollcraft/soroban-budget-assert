<div align="center">
  <h1>🛡️ Soroban Budget Assert</h1>
  <p><strong>Empirical cost measurement and assertion tooling for Soroban smart contracts.</strong></p>
  
  [![Build Status](https://github.com/Tollcraft/soroban-budget-assert/actions/workflows/budget.yml/badge.svg)](https://github.com/Tollcraft/soroban-budget-assert/actions)
  [![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
  <p>
    <a href="https://tollcraft.gitbook.io/docs/budget-assert"><strong>Documentation</strong></a> ·
    <a href="https://tollcraft.github.io/soroban-budget-assert/dashboard.html"><strong>Dashboard</strong></a> ·
    <a href="https://asciinema.org/a/qqC0RysuCDBvfUXC"><strong>Demo</strong></a>
  </p>
</div>

---

## 📖 Overview

`soroban-budget-assert` is a developer tool that measures the gap between local Soroban test estimates and real network costs. It allows developers to assert budget limits during testing and automatically generate detailed execution-resource reports across an entire workspace.

### 🏗️ Architecture

The tool is split into two primary components:

1. **`budget-macros` (Tier A - Local, Fast, CI-Blocking)**
   - Rust macros (`#[budget_cpu_lt(N)]`, `#[budget_mem_lt(N)]`) applied directly to your test functions.
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

Every push to `main` runs [`budget.yml`](.github/workflows/budget.yml), whose `record-history` job appends a `{commit, timestamp, data}` entry to `history.json` on the `gh-pages` branch. The static dashboard at [`site/dashboard.html`](site/dashboard.html) (published by [`deploy-site.yml`](.github/workflows/deploy-site.yml)) fetches that file at page load and plots per-function trend lines, so a regression like "`do_expensive_work` got 12% more expensive over the last ten commits" is visible at a glance.

**How the pieces fit together:**
1. `record-history` job → appends to `history.json` on `gh-pages`.
2. `deploy-site.yml` → publishes `site/**` to `gh-pages` with `keep_files: true`, so `history.json` is never wiped.
3. The dashboard page fetches `history.json` same-origin and pivots it client-side into `package → function → metric` series — no backend, no build-time data baking.

**Using this on your own repo:** copy the `record-history` job pattern and the `site/` folder into your repo, then open the dashboard with query params:
- `?history=URL` — where to fetch `history.json` from (default `./history.json`, same-origin).
- `?repo=owner/name` — links each point to its commit on GitHub (auto-detected on `<owner>.github.io/<repo>/` URLs; set explicitly for custom domains/forks).
- `?limit=N` — how many recent commits to render (default 200).

Example: `https://your-org.github.io/your-repo/dashboard.html?limit=100`.

## ⚙️ Supported Versions & Compatibility

* **Supported SDK Version**: `soroban-sdk` = `"22.0.11"` (specifically tested/resolved to `22.0.11` in `Cargo.lock`)
* **Supported XDR Version**: `stellar-xdr` = `"22.1.0"` (used for decoding transaction simulation responses)
* **Corresponding Stellar Protocol**: **Protocol 22**

### Compatibility Matrix

| SDK Version | Protocol Version | Status | Notes |
| :--- | :--- | :--- | :--- |
| **`< 22.0.0`** | `< 22` | **Untested** | Older protocols may use different transaction/resource schemas. |
| **`22.0.x`** | `22` | **Supported** | Matches pinned manifest dependencies (`soroban-sdk` `22.0.11`, `stellar-xdr` `22.1.0`). |
| **`>= 23.0.0`** | `>= 23` | **Untested** | Future protocol upgrades or XDR schema changes (e.g. key/field renames) may break parsing. |

---

## 🚀 Quick Start

### 1. Installation
Install the CLI tool locally from the repository root:
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

### 3. Usage

**Generate a Workspace Report:**
```bash
cargo budget-report
```

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
```

### 📊 Network Resource Limits & Percentage Reporting

`cargo-budget-report` reports each measured metric both as a raw number and as a percentage share of Soroban's per-transaction network resource limits:

- **CPU Instructions**: 100,000,000 inst. (`100_000_000`)
- **Read Bytes**: 200,000 B (`200_000`)
- **Write Bytes**: 100,000 B (`100_000`)

By default, these resource limits match **Soroban Protocol 21/22** per-transaction ceilings.

#### Visual Warning Threshold (`⚠️`)
Functions using at or above a configurable share of any network resource limit (default: **80.0%**) are visually flagged in table output with a `⚠️` warning indicator (e.g. `85,000,000 inst. (85.00% ⚠️)`).

#### Configuring Network Limits & Warning Threshold
You can customize network resource limits or the warning threshold via `budget.toml` or CLI flags:

```toml
# budget.toml
network = "testnet"
source = "alice"

[network_limits]
cpu_instructions = 100000000
read_bytes = 200000
write_bytes = 100000
protocol_version = "Protocol 22"
```

Or via CLI arguments:
```bash
# Override network limits and set warning threshold to 75%
cargo budget-report --max-instructions 100000000 --max-read-bytes 200000 --warning-threshold 75.0
```

#### Additive `--json` Output
The `--json` output includes additive `network_limit` and `pct_network_limit` fields for each reported metric:

```json
{
  "package": "amm-pool-contract",
  "function": "swap",
  "metric": "CPU Instructions",
  "value": 5000000,
  "network_limit": 100000000,
  "pct_network_limit": 5.0
}
```

### 🛡️ Blocking Network-Cost Regressions in CI

```yaml
# .github/workflows/budget.yml
- name: Build contracts
  run: cargo build -p amm-pool-contract --release --target wasm32-unknown-unknown

- name: Enforce budget limits against network-verified costs
  # Exits non-zero on any limit breach or on any configured function
  # whose simulation fails (so a broken sim cannot look like a pass).
  run: cargo run --bin cargo-budget-report -- budget-report --check --json
```

A pull request that pushes `do_expensive_work` past its limit — for example by
adding an unbounded loop — fails the job with output similar to:

```text
=== BUDGET CHECKS ===
amm-pool-contract::do_expensive_work [CPU Instructions] value=5,400,123 inst. limit=5,000,000 inst. FAIL
amm-pool-contract::do_expensive_work [Read Bytes] value=2,048 B limit=5,000 B PASS
amm-pool-contract::do_expensive_work [Write Bytes] value=1,024 B limit=1,000 B FAIL
Summary: 1 check(s) passed, 2 failed
```

CI surfaces the exact metric and limit on the failing run. Re-measure with
`cargo budget-report` and either optimize the function or consciously raise
the limit.

**Use Macros in Tests:**

The macros (`budget_cpu_lt`, `budget_mem_lt`) are attribute macros for test functions. They require a local variable named **`env`** — the generated code reads `env.cost_estimate().budget()` by name.

```rust
use budget_macros::{budget_cpu_lt, budget_mem_lt};
use soroban_sdk::Env;

// CPU instruction assertion using the AMM pool fixture
#[test]
#[budget_cpu_lt(2500000)] // local WASM ~2,307,555
fn test_cpu_budget() {
    let env = Env::default();
    let contract_id = env.register(ConstantProductPool, ());
    let client = ConstantProductPoolClient::new(&env, &contract_id);

    client.initialize();

    env.cost_estimate().budget().reset_unlimited();
    client.deposit(&user, &10_000_i128, &10_000_i128);
    client.swap(&user, &true, &100_i128, &90_i128);
    client.withdraw(&user, &1_000_i128, &900_i128, &900_i128);
}

// Memory assertion — same shape
#[test]
#[budget_mem_lt(2000000)] // local WASM ~1,589,080
fn test_mem_budget() {
    let env = Env::default();
    // register, initialize, reset_unlimited, deposit + swap + withdraw
}
```

---

## 📊 Measurements

The [MEASUREMENTS.md](MEASUREMENTS.md) file at the repository root records all empirical cost measurements comparing local Soroban budget estimates against real network costs. The [Protocol Mechanics documentation](https://tollcraft.gitbook.io/docs/budget-assert/protocol-mechanics) cites this file as the source of truth for measured figures.

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
