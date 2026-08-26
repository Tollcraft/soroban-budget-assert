# Host-Function Call Measurement Fixture (`host-function-contract`)

`host-function-contract` is a dedicated benchmark fixture crate within this repository. Its primary purpose is to isolate and measure the budget cost of repeated Soroban host-function invocations (specifically `env.ledger().sequence()`) without introducing storage reads, storage writes, or complex arithmetic computation.

## Why This Crate Exists

Soroban smart contracts incur budget charges both from WASM instruction execution and from host-function calls. To establish accurate Tier A and Tier B budget margins, we must measure the local-vs-network cost gap for isolated operation types:

- **AMM Pool Contract (`amm-pool-contract`):** Measures mixed compute + storage, storage writes, storage reads, memory allocations, and authorization calls.
- **Host Function Contract (`host-function-contract`):** Isolates repeated host-function call overhead (`env.ledger().sequence()`) with zero storage or side effects.

By isolating host-function calls in a zero-storage fixture, measurements reflect only the host-function invocation cost and WASM loop mechanics.

## Benchmark Operation

The core benchmark function exposed by this fixture is:

```rust
HostFunctionBenchmark::repeated_sequence(env: Env, iterations: u32) -> u32
```

Calling `repeated_sequence(1_000)` invokes `env.ledger().sequence()` 1,000 times in a simple loop and returns the final sequence number to prevent compiler dead-code elimination.

## Workspace Membership

> **Note on Workspace Membership:**  
> This crate was originally omitted from `[workspace.members]` in the root `Cargo.toml` due to an oversight during initial fixture setup. It has since been formally added to `workspace.members`. It is a standard workspace member, ensuring it is included in workspace builds (`cargo build`), workspace test runs (`cargo test --workspace`), formatting (`cargo fmt`), and static analysis (`cargo clippy`).

## How to Build and Run

### Building from a Clean Checkout

To compile the contract to WASM target:

```bash
cargo build -p host-function-contract --target wasm32-unknown-unknown --release
```

Or for newer Soroban toolchain targets (`wasm32v1-none`):

```bash
cargo build -p host-function-contract --target wasm32v1-none --release
```

### Running Tests

To run the package unit tests:

```bash
cargo test -p host-function-contract
```

To run all workspace tests including this crate:

```bash
cargo test --workspace
```

## Measurement Data and Reproducing Results

This fixture is used to record figures in [`MEASUREMENTS.md`](../MEASUREMENTS.md) and [`docs/src/measurements.md`](../docs/src/measurements.md).

### Local Budget Estimate

Running the contract in a WASM-registered Soroban local test via `env.cost_estimate().budget().cpu_insns()` produces:
- **Local WASM Estimate:** `1,280,000` CPU instructions (1,000 iterations).

### Network Simulation Figure

Submitting the compiled WASM binary and invocation to Soroban testnet via `simulateTransaction` produces:
- **Network Figure:** `1,600,000` CPU instructions.

### Calculated Gap (Delta)

$$\text{Delta} = \frac{\text{Local} - \text{Network}}{\text{Network}} = \frac{1,280,000 - 1,600,000}{1,600,000} = -20.0\%$$

The negative delta (-20.0%) demonstrates that local WASM estimates underestimate actual network execution costs for host-function-heavy workloads. Tier A margin calculations use this baseline gap to safeguard assertions against under-budgeting failures on-chain.

