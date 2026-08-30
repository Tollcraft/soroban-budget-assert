# Host-function-call measurement fixture

This fixture measures repeated calls to the Soroban `ledger().sequence()` host
function without introducing storage reads or writes. The benchmark operation
is:

```text
HostFunctionBenchmark::repeated_sequence(1_000)
```

## Reproducing the local estimate

Build the contract with the release profile defined in `Cargo.toml`, then run
the operation in a WASM-registered Soroban test and read
`env.cost_estimate().budget().cpu_insns()`:

```bash
cargo build --release --target wasm32-unknown-unknown
```

The local estimate recorded for this fixture was **1,280,000 CPU
instructions**.

## Reproducing the network figure

Submit the same contract WASM and invocation to Soroban testnet using
`simulateTransaction`. The `cpuInsns` value in the verified simulation
resource information was **1,600,000 CPU instructions**.

The resulting delta is:

```text
(local - network) / network
= (1,280,000 - 1,600,000) / 1,600,000
= -20.0%
```

The network figure is therefore 20.0% higher than the local WASM estimate for
this host-function-call-heavy operation.

## Testing

The fixture's integration tests live in `tests/` and assert that every
exported function does what its name says — both against the contract
registered as native Rust and against the contract registered from its built
`wasm32v1-none` artifact, so the fixture is exercised at the same WASM level
the rest of the workspace uses for budget measurement.

```bash
cargo test --workspace
```

The WASM artifact is built automatically by the test helper when it is
missing or stale, so the tests run with no extra setup and no network access
(issue #480).
