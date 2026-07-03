# Tool Reference

## CLI: `cargo-budget-report`

Usage: `cargo budget-report [--network <network>] [--source <source>] [--json]`

**Parameters**:
- `--network`: Network to simulate against (e.g., `testnet`).
- `--source`: Source account used for simulation fees.
- `--json`: Output raw JSON for CI/CD integration instead of a human-readable table.

## Macros: `budget_macros`

### `#[budget_cpu_lt(N)]`
Asserts that the CPU instructions used by the test's `env` are strictly less than `N`.

### `#[budget_mem_lt(N)]`
Asserts that the memory bytes used by the test's `env` are strictly less than `N`.
