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
