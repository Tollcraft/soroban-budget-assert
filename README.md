<div align="center">
  <h1>🛡️ Soroban Budget Assert</h1>
  <p><strong>Empirical cost measurement and assertion tooling for Soroban smart contracts.</strong></p>
  
  [![Build Status](https://github.com/Tollcraft/soroban-budget-assert/actions/workflows/budget.yml/badge.svg)](https://github.com/Tollcraft/soroban-budget-assert/actions/workflows/budget.yml)
  [![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
  <p>
    <a href="https://tollcraft.gitbook.io/docs/budget-assert"><strong>Documentation</strong></a> ·
    <a href="https://tollcraft.github.io/soroban-budget-assert/dashboard.html"><strong>Dashboard</strong></a>
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
   - Configurable via a central `budget.toml` file.

## 🚀 Quick Start

### Installation

```bash
cargo install --path cargo-budget-report
```

### Configuration

Scaffold a `budget.toml` in your workspace root:

```bash
cargo budget-report --init
```

The default report uses the configured `network` alias, such as `testnet` or `futurenet`. The source account can be selected with `source` in `budget.toml` or with `--source`.

### Local or Standalone RPC Nodes

To simulate against a local standalone Soroban RPC node, provide both its URL and the corresponding network passphrase:

```bash
cargo budget-report \
  --rpc-url http://localhost:8000/soroban/rpc \
  --network-passphrase "Standalone Network ; February 2025"
```

`--rpc-url` overrides the built-in `testnet`/`futurenet` RPC endpoints. `--network-passphrase` is required whenever `--rpc-url` is used and must match the passphrase configured by the standalone network. These options are useful for local Docker nodes, custom fee settings, and testing without public-network rate limits.

The same options can be combined with the other report flags, for example:

```bash
cargo budget-report \
  --rpc-url http://localhost:8000/soroban/rpc \
  --network-passphrase "Standalone Network ; February 2025" \
  --check --json
```

### Usage

Generate a workspace report:

```bash
cargo budget-report
```

Enforce regression limits declared in `budget.toml`:

```bash
cargo budget-report --check
```

The complete command-line interface is also available with:

```bash
cargo budget-report --help
```

## 🤝 Community & Maintainers

Join the discussion and get support:

* **Community Link**: [Stellar Developer Discord](https://discord.gg/5aprtMSyR)

## 🛠️ Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details.
