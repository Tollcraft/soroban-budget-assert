<div align="center">
  <h1>🛡️ Soroban Budget Assert</h1>
  <p><strong>Empirical cost measurement and assertion tooling for Soroban smart contracts.</strong></p>
  
  [![Build Status](https://github.com/Tollcraft/soroban-budget-assert/actions/workflows/budget.yml/badge.svg)](https://github.com/Tollcraft/soroban-budget-assert/actions)
  [![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
  <p>
    <a href="https://tollcraft.gitbook.io/docs/budget-assert"><strong>Documentation</strong></a> ·
    <a href="https://asciinema.org/a/qqC0RysuCDBvfUXC"><strong>Demo</strong></a>
  </p>
</div>

---

## 📖 Overview

`soroban-budget-assert` is a developer tool that measures the gap between local Soroban test estimates and real network costs. It allows developers to assert budget limits during testing and automatically generate detailed cost reports across an entire workspace.

### 🏗️ Architecture

The tool is split into two primary components:

1. **`budget-macros` (Tier A - Local, Fast, CI-Blocking)**
   - Rust macros (`#[budget_cpu_lt(N)]`, `#[budget_mem_lt(N)]`) applied directly to your test functions.
   - Fails the test the moment measured cost crosses your pinned limit, so cost regressions are caught in CI instead of on the network.

2. **`cargo-budget-report` (Tier B - Network-Verified, Reporting)**
   - A CLI tool that automatically discovers all contracts in your workspace.
   - Compiles WASM, simulates execution on testnet, and reports actual non-refundable costs (CPU instructions, read/write bytes).
   - Configurable via a central `budget.toml` file.

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
```rust
use budget_macros::budget_cpu_lt;

#[test]
#[budget_cpu_lt(800000)]
fn test_expensive_function() {
    let env = Env::default();
    // ... test logic ...
}
```

---

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
