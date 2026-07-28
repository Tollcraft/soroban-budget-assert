# Protocol Mechanics

## The measured gap

Every Soroban transaction runs against a resource budget. If the budget is exhausted on-chain, the transaction fails. Local tests estimate these costs, but the estimate depends on *how* the contract executes locally — and the error can point in either direction. Measured on the example contract's `do_expensive_work(10_000)`, built with the standard Soroban release profile (`opt-level = "z"`, LTO):

The raw figures and deltas are recorded in the [measurements file](../../MEASUREMENTS.md), which is the single source of truth for empirical cost data across the project. The same page documents the methodology, the build profiles tested, and the operation types not yet measured.

The standard profile is the full `[profile.release]` block used by this repository:

{% code title="Cargo.toml" %}
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
{% endcode %}

These settings materially change the WASM being measured. `opt-level = "z"` and LTO optimize the binary shape, `codegen-units = 1` gives LLVM a broader optimization view, `panic = "abort"` removes unwinding paths, `strip = "symbols"` and `debug = 0` remove non-runtime payload, `debug-assertions = false` keeps release behavior, and `overflow-checks = true` preserves checked arithmetic. Cost figures from another release profile are not comparable: the measurements file records 901,816 local WASM CPU instructions / 756,678 testnet instructions for the size-optimized profile, versus 767,049 local / 832,006 testnet for Cargo's default release profile.

{% hint style="info" %}
The direction of the WASM gap is not stable — see the two build profiles compared in the [existing measurements](../../MEASUREMENTS.md#cpu-instructions), and the [SDK version calibration](../../MEASUREMENTS.md#sdk-version-calibration) which tracks how the gap shifts across soroban-sdk versions.
{% endhint %}

Two conclusions drive the tool's design:

1. Raw Rust estimates are useless for budget decisions. Tests must run the compiled WASM.
2. Even WASM-mode local estimates can miss real network cost by double-digit percentages, in either direction depending on the build profile. The only trustworthy number is a network simulation of the exact WASM you deploy.

## Tier A: Local fast fail (`budget-macros`)

Two attribute macros gate tests on local cost estimates. Each rewrites the test function's body so a cost check against the test's local `env` variable runs on every path that leaves the test.

### `#[budget_cpu_lt(N)]` — CPU instruction assertion

Injects the equivalent of:

{% code title="injected by #[budget_cpu_lt(N)]" %}
```rust
// ... original statements, with each `return e` rewritten to
//     `return { let v = e; <check>; v }` ...

let __budget_value = /* original trailing expression, if any */;
{
    let budget = env.cost_estimate().budget();
    let cpu_cost = budget.cpu_instruction_cost();
    assert!(cpu_cost < N, "CPU instruction cost {} exceeded limit {} - ...", cpu_cost, N);
}
__budget_value
```
{% endcode %}

### `#[budget_mem_lt(N)]` — Memory bytes assertion

Same shape, checks `budget.memory_bytes_cost()`:

{% code title="injected by #[budget_mem_lt(N)]" %}
```rust
let budget = env.cost_estimate().budget();
let mem_cost = budget.memory_bytes_cost();
assert!(mem_cost < N, "Memory bytes cost {} exceeded limit {} - ...", mem_cost, N);
```
{% endcode %}

Binding the trailing expression keeps it as the function's value, so `fn test() -> Result<(), Error>` bodies ending in `Ok(())` compile; rewriting `return` keeps an early exit from skipping the check. The check stays inside the body's scope, so the limit expression still resolves against the body's own bindings. Two paths out of the body are not instrumented: a `?` that propagates an error (the test already fails on that error) and a `return` produced by another macro's expansion. A `return` written directly in macro invocation tokens is a compile error rather than a silent skip — see the [Reference](reference.md).

Both assertions are strict (`<`). If the local estimate reaches the limit, the test panics and `cargo test` fails, which blocks CI. This tier is fast (no network) and deterministic, so it is safe to run on every push and pull request.

### Dynamic limits via environment variables

Either macro can read its limit from an environment variable at test time instead of using a hard-coded integer:

```rust
#[budget_cpu_lt(env = "MY_CPU_LIMIT")]
```

When the variable is unset, the limit defaults to `u64::MAX`, making the assertion pass unconditionally; a set-but-unparsable value panics. This lets you raise or disable limits without recompiling — useful for CI matrix builds where different runners have different budgets.

## Tier B: Network simulation (`cargo-budget-report`)

The CLI measures ground truth. One invocation walks this pipeline:

1. **Discover** — runs `cargo metadata` and selects every workspace package with a `cdylib` target (i.e., every Soroban contract).
2. **Build** — compiles each contract with `cargo build --target wasm32-unknown-unknown --release`, using the workspace's `[profile.release]`.
3. **Scan exports** — parses the `.wasm` binary with `wasmparser` and collects every exported function, skipping internals (names starting with `_`, and `memory`).
4. **Deploy** — deploys the WASM to the configured network with `stellar contract deploy`.
5. **Simulate** — for each exported function, builds an unsigned transaction (`stellar contract invoke --build-only`, with per-function arguments from `budget.toml`), then POSTs it to the Soroban RPC `simulateTransaction` endpoint.
6. **Decode** — decodes the returned `SorobanTransactionData` XDR (`stellar xdr decode`) and extracts `resources.instructions`, `resources.disk_read_bytes`, and `resources.write_bytes`; on Soroban Protocol 22+, additionally reads `result.cost.memBytes` from the JSON-RPC `cost` block (see Memory Bytes below). On older protocol responses the `Memory Bytes` row is simply omitted (absence is not zero).
7. **Report** — aggregates every package/function pair into one table, or JSON with `--json`.

Simulated numbers vary slightly with ledger state, but they are the network's own measurement of the exact WASM you deploy, not a local approximation.

These three figures are resource *amounts*, and they are inputs to the non-refundable resource fee — not the fee itself and not a total cost. Rent, other refundable fees, transaction size, footprint entry counts, and the inclusion fee are outside what the tool measures. See [Measurement scope](reference.md#measurement-scope) for the full boundary and where to find the omitted pieces.

### Memory Bytes (Protocol 22+)

The fourth reported row, `Memory Bytes`, does not come from the `SorobanTransactionData` XDR — Protocol 22 simulations return a separate `result.cost` JSON object alongside the XDR, and it carries per-metric human-readable `cpuInsns` and `memBytes` strings (stringified for `u64` precision over JSON). The CLI reads `result.cost.memBytes` from the JSON-RPC response in addition to the XDR fields, accepting both integer and string forms. This is the Source for the local-vs-network memory-bytes gap measurement series (issue #122).

## How the tiers work together

Tier B tells you what a function really costs on the network. Tier A pins the *local* estimate into your test suite: measure once, assert a limit a few percent above the measured local number, and any change that pushes execution cost past it fails CI before it reaches the network. The example contract's gated test uses exactly this pattern: local WASM estimate 2,654,615, asserted limit 2,800,000, real testnet cost (placeholder — see [SDK version calibration](../../MEASUREMENTS.md#sdk-version-calibration)).

> **Warning:** The local WASM estimate shifts with soroban-sdk version. The SDK version calibration table in [MEASUREMENTS.md](../../MEASUREMENTS.md#sdk-version-calibration) should be regenerated on every SDK bump so Tier A limits are based on current numbers, not stale ones.

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
