# Protocol Mechanics

The tool operates on two tiers to provide accurate budget assertions.

### Tier A: Local Fast Fail (`budget-macros`)
The macro injects cost-checking logic directly into standard `#[test]` functions. It calls `env.cost_estimate().budget().cpu_instruction_cost()` and asserts it strictly against the developer's provided limit. If the local estimate exceeds the limit, the test fails, blocking CI.

### Tier B: Network Simulation (`cargo-budget-report`)
The CLI compiles the workspace to `wasm32-unknown-unknown`, parses the WASM binary to discover exported functions, and fires `simulateTransaction` RPC requests to a live Soroban RPC node (e.g., testnet). It extracts `instructions`, `disk_read_bytes`, and `write_bytes` from the XDR response and outputs an aggregated table.
