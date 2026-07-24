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
   - Compiles WASM, simulates execution on testnet, and reports the simulated resource amounts (CPU instructions, read/write bytes).
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

* **Supported SDK Version**: `soroban-sdk` = `"22.0.0"` (specifically tested/resolved to `22.0.11` in `Cargo.lock`)
* **Supported XDR Version**: `stellar-xdr` = `"22.1.0"` (used for decoding transaction simulation responses)
* **Corresponding Stellar Protocol**: **Protocol 22**

### Compatibility Matrix

| SDK Version | Protocol Version | Status | Notes |
| :--- | :--- | :--- | :--- |
| **`< 22.0.0`** | `< 22` | **Untested** | Older protocols may use different transaction/resource schemas. |
| **`22.0.x`** | `22` | **Supported** | Matches pinned manifest dependencies (`soroban-sdk` `22.0.0`, `stellar-xdr` `22.1.0`). |
| **`>= 23.0.0`** | `>= 23` | **Untested** | Future protocol upgrades or XDR schema changes (e.g. key/field renames) may break parsing. |

---

## 🚀 Quick Start

### 1. Installation
Install the CLI tool locally from the repository root:
```bash
cargo install --path cargo-budget-report
```

### 2. Configuration
Create a `budget.toml` in your workspace root:
```toml
network = "testnet"
source = "alice"

[functions.do_expensive_work]
args = ["--n", "10000"]
```

### 3. Usage

**Generate a Workspace Report:**
```bash
cargo budget-report
```

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
