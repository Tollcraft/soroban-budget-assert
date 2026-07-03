# Protocol Mechanics

## The measured gap

Every Soroban transaction runs against a resource budget. If the budget is exhausted on-chain, the transaction fails. Local tests estimate these costs, but the estimate depends on *how* the contract executes locally. Measured on the example contract's `do_expensive_work(10_000)`:

| Execution mode | CPU instructions | Gap vs. testnet |
|---|---|---|
| Raw Rust (native test) | 143,887 | underestimates by ~83% |
| Local WASM (`register_contract_wasm`) | 767,049 | underestimates by ~8% |
| Testnet simulation (`simulateTransaction`) | 832,006 | ground truth |

Two conclusions drive the tool's design:

1. Raw Rust estimates are useless for budget decisions. Tests must run the compiled WASM.
2. Even WASM-mode local estimates run ~8% under real network cost. Assertions need headroom above the local number.

## Tier A: Local fast fail (`budget-macros`)

`#[budget_cpu_lt(N)]` and `#[budget_mem_lt(N)]` are procedural attribute macros. Each one rewrites the test function's body: the original statements run first, then the macro appends a cost check against the test's local `env` variable:

```rust
let budget = env.cost_estimate().budget();
let cpu_cost = budget.cpu_instruction_cost();
assert!(cpu_cost < N, "CPU instruction cost {} exceeded limit {} - ...", cpu_cost, N);
```

The assertion is strict (`<`). If the local estimate reaches the limit, the test panics and `cargo test` fails, which blocks CI. This tier is fast (no network) and deterministic, so it is safe to run on every push and pull request.

## Tier B: Network simulation (`cargo-budget-report`)

The CLI measures ground truth. One invocation walks this pipeline:

1. **Discover** — runs `cargo metadata` and selects every workspace package with a `cdylib` target (i.e., every Soroban contract).
2. **Build** — compiles each contract with `cargo build --target wasm32-unknown-unknown --release`.
3. **Scan exports** — parses the `.wasm` binary with `wasmparser` and collects every exported function, skipping internals (names starting with `_`, and `memory`).
4. **Deploy** — deploys the WASM to the configured network with `stellar contract deploy`.
5. **Simulate** — for each exported function, builds an unsigned transaction (`stellar contract invoke --build-only`, with per-function arguments from `budget.toml`), then POSTs it to the Soroban RPC `simulateTransaction` endpoint.
6. **Decode** — decodes the returned `SorobanTransactionData` XDR (`stellar xdr decode`) and extracts `resources.instructions`, `resources.disk_read_bytes`, and `resources.write_bytes`.
7. **Report** — aggregates every package/function pair into one table, or JSON with `--json`.

Simulated numbers vary slightly with ledger state, but they are the numbers the network will charge — non-refundable resource costs, not local approximations.

## How the tiers work together

Tier B tells you what a function really costs. Tier A pins that number (plus margin) into your test suite, so a regression that blows the budget fails CI before it fails on the network. The example contract's gated test uses exactly this pattern: measured testnet cost ~832,006, asserted limit 850,000.
