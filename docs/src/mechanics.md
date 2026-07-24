# Protocol Mechanics

## The measured gap

Every Soroban transaction runs against a resource budget. If the budget is exhausted on-chain, the transaction fails. Local tests estimate these costs, but the estimate depends on *how* the contract executes locally — and the error can point in either direction. Measured on the example contract's `do_expensive_work(10_000)`, built with the standard Soroban release profile (`opt-level = "z"`, LTO):

The raw figures and deltas are recorded in the [measurements file](../../MEASUREMENTS.md), which is the single source of truth for empirical cost data across the project. The same page documents the methodology, the build profiles tested, and the operation types not yet measured.

{% hint style="info" %}
The direction of the WASM gap is not stable — see the two build profiles compared in the [existing measurements](../../MEASUREMENTS.md#cpu-instructions).
{% endhint %}

Two conclusions drive the tool's design:

1. Raw Rust estimates are useless for budget decisions. Tests must run the compiled WASM.
2. Even WASM-mode local estimates can miss real network cost by double-digit percentages, in either direction depending on the build profile. The only trustworthy number is a network simulation of the exact WASM you deploy.

## Tier A: Local fast fail (`budget-macros`)

`#[budget_cpu_lt(N)]` and `#[budget_mem_lt(N)]` are procedural attribute macros. Each one rewrites the test function's body: the original statements run first, then the macro appends a cost check against the test's local `env` variable:

{% code title="appended by #[budget_cpu_lt(N)]" %}
```rust
let budget = env.cost_estimate().budget();
let cpu_cost = budget.cpu_instruction_cost();
assert!(cpu_cost < N, "CPU instruction cost {} exceeded limit {} - ...", cpu_cost, N);
```
{% endcode %}

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

Simulated numbers vary slightly with ledger state, but they are the network's own measurement of the exact WASM you deploy, not a local approximation.

These three figures are resource *amounts*, and they are inputs to the non-refundable resource fee — not the fee itself and not a total cost. Rent, other refundable fees, transaction size, footprint entry counts, and the inclusion fee are outside what the tool measures. See [Measurement scope](reference.md#measurement-scope) for the full boundary and where to find the omitted pieces.

## How the tiers work together

Tier B tells you what a function really costs on the network. Tier A pins the *local* estimate into your test suite: measure once, assert a limit a few percent above the measured local number, and any change that pushes execution cost past it fails CI before it reaches the network. The example contract's gated test uses exactly this pattern: local WASM estimate 901,816, asserted limit 950,000, real testnet cost 756,678 known from Tier B.
