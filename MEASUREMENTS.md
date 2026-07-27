# Measurements

This file records empirical cost measurements comparing local Soroban budget estimates against real network costs. Every measurement PR adds its numbers here so the series stays comparable and in one place.

## Methodology

Each measurement compares a local budget estimate against a network-verified figure for the same operation. The local estimate comes from `Env::cost_estimate().budget()` in a test that registers the contract as WASM with `register_contract_wasm` (except where noted). The network figure comes from `simulateTransaction` on Soroban testnet — the same endpoint the network uses to charge non-refundable resource costs.

The WASM is compiled with the profile specified in the **Build profile** column. The direction of the local-vs-network gap is not stable across profiles; the same contract built with Cargo's default release profile can produce a gap pointing in the opposite direction of one built with the size-optimization profile. Every figure includes its build context.

For the storage-write measurement, the complete capture record is checked in at [`cargo-budget-report/fixtures/storage_write_benchmark.json`](cargo-budget-report/fixtures/storage_write_benchmark.json). It records the fixture arguments, local capture command, network capture method, both figures, and the calculated delta.

### Column reference

| Column | Meaning |
|---|---|
| **Operation type** | Category of operation being measured |
| **Local estimate** | Value reported by `Env::cost_estimate().budget()` in a WASM-registered local test |
| **Network figure** | Value returned by `simulateTransaction` on Soroban testnet (ground truth) |
| **Delta** | (local − network) / network, expressed as a percentage; positive means local overestimates |
| **Fixture** | Contract, function, and arguments used for the measurement |
| **Build profile** | Cargo profile used to compile the WASM |
| **Toolchain** | Rust toolchain version (`rustc --version`) |
| **Date** | Date the measurement was taken |

## Existing measurements

These figures were produced during the initial tool development and are published in the [Protocol Mechanics documentation](docs/src/mechanics.md). They serve as the worked example for contributors adding new measurements.

### CPU instructions

| Operation type | Local estimate | Network figure | Delta | Fixture | Build profile | Toolchain | Date |
|---|---:|---:|---:|---|---|---|---|
| Mixed compute + storage (native Rust) | 143,887 | 756,678 | −81.0% | `amm-pool-contract::do_expensive_work(10_000)` | N/A (native test, no WASM) | rustc 1.81 | 2025-Q1 |
| Mixed compute + storage (WASM) | 901,816 | 756,678 | +19.2% | `amm-pool-contract::do_expensive_work(10_000)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.81 | 2025-Q1 |
| Mixed compute + storage (WASM) | 767,049 | 832,006 | −7.8% | `amm-pool-contract::do_expensive_work(10_000)` | default `release` (`opt-level=3`) | rustc 1.81 | 2025-Q1 |
| Storage write (WASM) | 36,840 | 44,512 | −17.2% | `amm-pool-contract::write_bytes(1,024 bytes)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.81 | 2026-07-26 |

The native Rust row is included solely to illustrate that native estimates are unreliable for budget decisions. Only WASM-mode estimates should be used for assertions.

The first three rows measure the same `do_expensive_work(10_000)` function, which mixes a compute loop (`n` iterations of `wrapping_add(wrapping_mul)`) with a storage write (`Vec` of up to 100 elements written to `env.storage().instance().set`). The numbers are aggregate costs of both operations.

The storage-write row isolates the `write_bytes` fixture with a 1,024-byte value. Its delta is calculated as `(36,840 − 44,512) / 44,512 = −0.1724`, so the WASM-registered local estimate is 17.2% lower than the testnet simulation for this operation and underestimates the network cost.

## SDK version calibration

The existing measurement series (above) shows the local-vs-network gap can flip direction with build profile alone. The SDK/protocol version is a second axis that shifts these numbers. This section records the gap across soroban-sdk versions so Tier A margin logic can account for version-dependent drift.

### Methodology

Each measurement uses the same contract (`amm-pool-contract`), the same function (`do_expensive_work(10_000)`), and the same build profile (workspace `[profile.release]`: `opt-level="z"`, LTO, `codegen-units=1`). Only the soroban-sdk version changes. The local WASM estimate is collected by the `calibrate_gap` test in `amm-pool-contract/tests/calibrate_gap.rs`:

```
cargo build --target wasm32-unknown-unknown --release -p amm-pool-contract
cargo test -p amm-pool-contract calibrate_gap -- --nocapture
```

SDK 20 and 21 use `env.budget()` instead of `env.cost_estimate().budget()`.  For those versions, run with `--features sdk20` and use the `calibrate_gap_sdk20` test binary:

```
cargo build --target wasm32-unknown-unknown --release -p amm-pool-contract
cargo test -p amm-pool-contract --features sdk20 --test calibrate_gap_sdk20 calibrate_gap -- --nocapture
```

The network figure column requires a separate `cargo-budget-report` run on Soroban testnet against the same WASM, and is a placeholder until the measurement can be taken on a live network.

### Per-version calibration table

| SDK version (pinned) | SDK version (resolved) | Local CPU estimate | Local mem estimate | Network CPU | Network mem | Delta CPU | Date | Toolchain |
|---|---|---|---|---|---|---|---|---|
| `20.0.0` | `20.5.0` | 6,606,666 | 1,942,982 | — | — | — | 2026-Q3 | `rustc 1.85.0` |
| `21.0.0` (≈`21.7.7`)^* | `21.7.7` | 2,653,878 | 1,658,163 | — | — | — | 2026-Q3 | `rustc 1.85.0` |
| `22.0.0` | `22.0.11` | 2,654,615 | 1,658,706 | — | — | — | 2026-Q3 | `rustc 1.85.0` |

> ^* SDK 21.0.0 is yanked; the lowest resolvable 21.x patch is 21.7.7.

> **Note on SDK 21 compilation.** soroban-env-host 21.2.1 has a `rand_core` / `ed25519-dalek` version conflict in its `testutils` feature. Running `cargo update -p soroban-env-host` resolves it by flushing the stale dependency graph. This is a one-time workaround needed when first pinning to SDK 21.

> **Note on SDK 20 API.** soroban-sdk 20.x uses `env.budget()` instead of `env.cost_estimate().budget()`. A separate test file (`calibrate_gap_sdk20.rs`) is gated behind the `sdk20` Cargo feature and provides the same measurement.

> **Temporary workspace constraints.** The workspace's `cargo-budget-report` crate requires `stellar-xdr ^22.1.0`, which limits long-term compatible soroban-sdk versions to the 22.0.x line. SDK 20 and 21 are tested by temporarily loosening the `stellar-xdr` constraint and regenerating the lockfile. These constraints are workspace-specific and should be re-evaluated when the project upgrades to a newer SDK baseline.

### How to regenerate

1. Pin the desired soroban-sdk version in `amm-pool-contract/Cargo.toml` (both `[dependencies]` and `[dev-dependencies]`).
2. Run `cargo update -p soroban-sdk` to resolve.
3. Build the WASM: `cargo build --target wasm32-unknown-unknown --release -p amm-pool-contract`.
4. Collect local estimate: `cargo test -p amm-pool-contract calibrate_gap -- --nocapture`.
5. For the network figure, deploy the WASM to testnet and run `cargo run --bin cargo-budget-report -- --network testnet` (see [Network simulation in mechanics.md](docs/src/mechanics.md#tier-b-network-simulation-cargo-budget-report)).
6. Compute delta = (local − network) / network and add a row to the table above.

A reusable script at `amm-pool-contract/calibrate_gap.ps1` automates steps 1–4 for a predefined list of SDK versions.

### Cross-version comparison (local only)

| SDK | CPU | Mem | CPU Δ vs SDK 22 | Mem Δ vs SDK 22 |
|---|---|---|---|---|
| 20.5.0 | 6,606,666 | 1,942,982 | +148.9% | +17.1% |
| 21.7.7 | 2,653,878 | 1,658,163 | −0.03% | −0.03% |
| 22.0.11 | 2,654,615 | 1,658,706 | — | — |

SDK 20 is dramatically more expensive (+149% CPU) because its `vm.exec` cost model uses a much higher per-instruction multiplier. SDK 21 and 22 are practically identical at the local-estimate level — the CPU delta is 737 instructions (−0.03%) and the memory delta is 543 bytes (−0.03%), well within measurement noise.

### Conclusion

The local WASM estimate for the size-opt profile at soroban-sdk 22.0.11 is **2,654,615** CPU instructions, up from 901,816 in the earlier measurement (rustc 1.81, SDK 22.0.0-era toolchain). The difference is attributable to changes in the Rust toolchain (1.81 → 1.85) and the SDK's internal host environment crate versions between patches. This confirms that the gap is unstable across both toolchain and SDK axes, reinforcing the architectural decision to derive Tier A margins from a network-simulated baseline rather than from local estimates alone.

**SDK 20 is a special case.** The 149% CPU overhead means assertions written against SDK 22+ local estimates will fail by a wide margin when run against SDK 20-compiled WASM. If the production network runs a pre-21 protocol version, Tier A margins must be widened accordingly or the contract must be compiled with an SDK 21+ toolchain.

**SDK 21 vs 22.** The local estimates are indistinguishable (~0.03% delta), so the same margin can be used for both. This also means the SDK 22 measurement can serve as a proxy for SDK 21 when deriving network-gap corrections.

**Recommendation:** regenerate this table on every SDK bump. A margin computed against a stale SDK baseline is no better than a guess.

## Unmeasured operation types

The following operation types have open measurement issues and no published figures yet. When adding a measurement, follow the column format above and include the build profile and toolchain.

| Operation type | Issue | Status |
|---|---|---|
| Host-function-call operations | [#86](https://github.com/Tollcraft/soroban-budget-assert/issues/86) | Open |
| VM-instruction-heavy operations | [#87](https://github.com/Tollcraft/soroban-budget-assert/issues/87) | Open |
| Memory bytes | [#122](https://github.com/Tollcraft/soroban-budget-assert/issues/122) | Open |
