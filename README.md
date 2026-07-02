# soroban-budget-assert

Empirical-measurement component of the cost-tooling research track.

This crate measures the gap between local test estimates and real network cost.

## Scope

### Tier A (local, fast, CI-blocking)
- `#[budget_cpu_lt(N)]` / `#[budget_mem_lt(N)]` macros.
- Contracts under test registered via `env.register_contract_wasm(...)`.
- Explicit failures that say: "local estimate, underestimates real network cost".

### Tier B (network-verified, reporting only)
- `cargo budget-report` for fully automated simulated transaction reporting across the entire workspace.
- Supports `budget.toml` for centralized configuration of test arguments.
- Reports actual non-refundable (CPU, read/write bytes) breakdown.
- Output is a table for humans, or `--json` for CI/CD environments.

## Milestones

1. **Measure the delta:** Run a deliberately expensive function through raw-Rust local, WASM-mode local, and simulateTransaction on testnet. 
   - **Findings**:
     - Raw Rust Local: CPU instructions ~143,887
     - WASM Local: CPU instructions ~767,049
     - Testnet Network Simulation: CPU instructions ~832,006
     - **Conclusion**: WASM local estimate underestimates real network cost by about ~8% (and raw Rust by ~83%).
2. **Tier A macro:** Working against a soroban-examples contract, gating on a deliberate regression, using the discovered margin.
3. **Tier B report command:** Producing readable output for 2–3 example contracts.

## Progress tracker
- [x] Initializing Rust environment.
- [x] Milestone 1: Measure the delta
- [x] Milestone 2: Tier A macro (implemented and gating regressions based on ~10% margin limit)
- [x] Milestone 3: Tier B report command (`cargo budget-report`)
- [x] **MVP Upgrade**: Automated workspace discovery and function simulation
- [x] **MVP Upgrade**: `budget.toml` config and JSON CI/CD output
