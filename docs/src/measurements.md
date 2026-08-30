# Measurements

This page records empirical cost measurements comparing local Soroban budget estimates against real network costs. Every measurement pull request adds its numbers here so the series stays comparable across toolchain and SDK versions.

> **Source repository:** The root [`MEASUREMENTS.md`](https://github.com/Tollcraft/soroban-budget-assert/blob/main/MEASUREMENTS.md) file in the repository is the canonical source of truth for raw benchmark fixtures and checked-in capture records.

---

## Methodology

Each measurement compares a local budget estimate against a network-verified figure for the same operation. The local estimate comes from `Env::cost_estimate().budget()` in a test that registers the contract as WASM with `register_contract_wasm`. The network figure comes from `simulateTransaction` on Soroban testnet — the same endpoint the network uses to charge non-refundable resource costs.

The WASM is compiled with the profile specified in the **Build profile** column. The direction of the local-vs-network gap is not stable across profiles; the same contract built with Cargo's default release profile can produce a gap pointing in the opposite direction of one built with the size-optimization profile. Every figure includes its build context.

For the storage-write measurement, the complete capture record is checked in at [`cargo-budget-report/fixtures/storage_write_benchmark.json`](https://github.com/Tollcraft/soroban-budget-assert/blob/main/cargo-budget-report/fixtures/storage_write_benchmark.json). It records the fixture arguments, local capture command, network capture method, both figures, and the calculated delta.

### Column reference

| Column | Meaning |
|---|---|
| **Operation type** | Category of operation being measured |
| **Local estimate** | Value reported by `Env::cost_estimate().budget()` in a WASM-registered local test |
| **Network figure** | Value returned by `simulateTransaction` on Soroban testnet |
| **Delta** | `(local − network) / network`, expressed as a percentage; positive means local overestimates |
| **Fixture** | Contract, function, and arguments used for the measurement |
| **Build profile** | Cargo profile used to compile the WASM |
| **Toolchain** | Rust toolchain version (`rustc --version`) |
| **Date** | Date the measurement was taken |

---

## Existing measurements

These figures were produced during the initial tool development and are published in the [Protocol Mechanics documentation](mechanics.md). They serve as the worked example for contributors adding new measurements.

### CPU instructions

| Operation type | Local estimate | Network figure | Delta | Fixture | Build profile | Toolchain | Date |
|---|---:|---:|---:|---|---|---|---|
| Mixed compute + storage (native Rust) | 143,887 | 756,678 | −81.0% | `amm-pool-contract::do_expensive_work(10_000)` | N/A (native test, no WASM) | rustc 1.81 | 2025-Q1 |
| Mixed compute + storage (WASM) | 901,816 | 756,678 | +19.2% | `amm-pool-contract::do_expensive_work(10_000)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.81 | 2025-Q1 |
| Mixed compute + storage (WASM) | 767,049 | 832,006 | −7.8% | `amm-pool-contract::do_expensive_work(10_000)` | default `release` (`opt-level=3`) | rustc 1.81 | 2025-Q1 |
| Storage write (WASM) | 36,840 | 44,512 | −17.2% | `amm-pool-contract::write_bytes(1,024 bytes)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.81 | 2026-07-26 |
| Storage read (WASM) | — | — | — | `amm-pool-contract::do_read_heavy_work(100)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.85 | 2026-07-27 |
| Host-function calls (WASM) | 1,280,000 | 1,600,000 | −20.0% | `host-function-contract::repeated_sequence(1_000)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.81 | 2025-Q2 |
| VM-instruction-only (WASM) | 689,312 | 634,912 | +8.6% | `amm-pool-contract::do_vm_instruction_work(10_000)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.81 | 2025-Q2 |
| TTL extension (WASM) | — | — | — | `amm-pool-contract::extend_instance_ttl(100, 10_000)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.85.0 | — |

> **TTL extension note:** The TTL extension fixture registers the contract as WASM, initializes it (creating instance storage entries), then calls `extend_instance_ttl(threshold=100, extend_to=10_000)`. Local estimate collected via `cargo test -p amm-pool-contract --test calibrate_extend_ttl -- --nocapture`. The complete capture record is checked in at [`cargo-budget-report/fixtures/ttl_extension_benchmark.json`](https://github.com/Tollcraft/soroban-budget-assert/blob/main/cargo-budget-report/fixtures/ttl_extension_benchmark.json). Network figure requires a `simulateTransaction` run on Soroban testnet.

The native Rust row is included solely to illustrate that native estimates are unreliable for budget decisions. Only WASM-mode estimates should be used for assertions.

The first three rows measure the same `do_expensive_work(10_000)` function, which mixes a compute loop (`n` iterations of `wrapping_add(wrapping_mul)`) with a storage write (`Vec` of up to 100 elements written to `env.storage().instance().set`). The numbers are aggregate costs of both operations.

The storage-write row isolates the `write_bytes` fixture with a 1,024-byte value. Its delta is calculated as `(36,840 − 44,512) / 44,512 = −0.1724`, so the WASM-registered local estimate is 17.2% lower than the testnet simulation for this operation and underestimates the network cost.

The storage-read row isolates `do_read_heavy_work` with 100 keys (25,600 bytes of reads). Unlike the write measurement, the read fixture necessarily includes a write phase (to populate the keys before reading them). The writes use `instance()` storage, which matches real contract usage, while the write measurement counterpart (`do_write_heavy_work`) uses `temporary()` storage — the two measurements are therefore not directly comparable at the storage-type level but serve complementary roles in the gap series.

```bash
cargo build --target wasm32v1-none --release -p amm-pool-contract
cargo test -p amm-pool-contract test_storage_read_wasm_local -- --nocapture
```

The network figure is collected via `cargo budget-report` on Soroban testnet against the same WASM. The complete capture record is checked in at [`cargo-budget-report/fixtures/storage_read_benchmark.json`](https://github.com/Tollcraft/soroban-budget-assert/blob/main/cargo-budget-report/fixtures/storage_read_benchmark.json).

The host-function row uses the dedicated [`host-function-contract`](https://github.com/Tollcraft/soroban-budget-assert/tree/main/host-function-contract) fixture crate, which isolates 1,000 calls to `env.ledger().sequence()` with zero storage side-effects. See [`host-function-contract/README.md`](https://github.com/Tollcraft/soroban-budget-assert/blob/main/host-function-contract/README.md) for build, execution, and reproduction instructions.

---

## SDK version calibration

The existing measurement series shows the local-vs-network gap can flip direction with build profile alone. The SDK/protocol version is a second axis that shifts these numbers. This section records the gap across `soroban-sdk` versions so Tier A margin logic can account for version-dependent drift.

### Methodology

Each measurement uses the same contract (`amm-pool-contract`), the same function (`do_expensive_work(10_000)`), and the same build profile (workspace `[profile.release]`: `opt-level="z"`, LTO, `codegen-units=1`). Only the `soroban-sdk` version changes. The local WASM estimate is collected by the `calibrate_gap` test in `amm-pool-contract/tests/calibrate_gap.rs`:

```bash
cargo build --target wasm32v1-none --release -p amm-pool-contract
cargo test -p amm-pool-contract calibrate_gap -- --nocapture
```

SDK 20 and 21 use `env.budget()` instead of `env.cost_estimate().budget()`. For those versions, run with `--features sdk20` and use the `calibrate_gap_sdk20` test binary:

```bash
cargo build --target wasm32v1-none --release -p amm-pool-contract
cargo test -p amm-pool-contract --features sdk20 --test calibrate_gap_sdk20 calibrate_gap -- --nocapture
```

The network figure column requires a separate `cargo-budget-report` run on Soroban testnet against the same WASM.

### Per-version calibration table

| SDK version (pinned) | SDK version (resolved) | Local CPU estimate | Local mem estimate | Network CPU | Network mem | Delta CPU | Date | Toolchain |
|---|---|---|---|---|---|---|---|---|
| `20.0.0` | `20.5.0` | 6,606,666 | 1,942,982 | — | — | — | 2026-Q3 | `rustc 1.85.0` |
| `21.0.0` (≈`21.7.7`)^* | `21.7.7` | 2,653,878 | 1,658,163 | — | — | — | 2026-Q3 | `rustc 1.85.0` |
| `22.0.0` | `22.0.11` | 2,654,615 | 1,658,706 | — | — | — | 2026-Q3 | `rustc 1.85.0` |

> ^* SDK 21.0.0 is yanked; the lowest resolvable 21.x patch is 21.7.7.

> **Note on SDK 21 compilation:** `soroban-env-host` 21.2.1 has a `rand_core` / `ed25519-dalek` version conflict in its `testutils` feature. Running `cargo update -p soroban-env-host` resolves it by flushing the stale dependency graph.

> **Note on SDK 20 API:** `soroban-sdk` 20.x uses `env.budget()` instead of `env.cost_estimate().budget()`. A separate test file (`calibrate_gap_sdk20.rs`) is gated behind the `sdk20` Cargo feature and provides the same measurement.

### Cross-version comparison (local only)

| SDK | CPU | Mem | CPU Δ vs SDK 22 | Mem Δ vs SDK 22 |
|---|---|---|---|---|
| 20.5.0 | 6,606,666 | 1,942,982 | +148.9% | +17.1% |
| 21.7.7 | 2,653,878 | 1,658,163 | −0.03% | −0.03% |
| 22.0.11 | 2,654,615 | 1,658,706 | — | — |

SDK 20 is dramatically more expensive (+149% CPU) because its `vm.exec` cost model uses a much higher per-instruction multiplier. SDK 21 and 22 are practically identical at the local-estimate level — the CPU delta is 737 instructions (−0.03%) and the memory delta is 543 bytes (−0.03%), well within measurement noise.

### How to regenerate

1. Pin the desired `soroban-sdk` version in `amm-pool-contract/Cargo.toml` (both `[dependencies]` and `[dev-dependencies]`).
2. Run `cargo update -p soroban-sdk` to resolve.
3. Build the WASM: `cargo build --target wasm32v1-none --release -p amm-pool-contract`.
4. Collect local estimate: `cargo test -p amm-pool-contract calibrate_gap -- --nocapture`.
5. For the network figure, deploy the WASM to testnet and run `cargo run --bin cargo-budget-report -- --network testnet` (see [Network simulation in mechanics.md](mechanics.md#tier-b-network-simulation-cargo-budget-report)).
6. Compute `delta = (local − network) / network` and add a row to the table above.

A reusable script at `amm-pool-contract/calibrate_gap.ps1` automates steps 1–4 for a predefined list of SDK versions.

---

## Authorization (`require_auth`) measurement

This section records the local-vs-network cost gap for the `require_auth` host-function call, isolated from all other contract logic. The `require_auth_only` function in `amm-pool-contract` calls `addr.require_auth()` with no storage reads, writes, or compute — making it the cleanest representative scenario for measuring the authorization cost gap.

### Figures

| Operation type | Local CPU | Local mem | Network CPU | Network mem | Delta CPU | Fixture | Build profile | Toolchain | Date |
|---|---:|---:|---:|---:|---:|---|---|---|---|
| Authorization (`require_auth`) | 2,864,886 | 1,721,879 | — | — | — | `amm-pool-contract::require_auth_only` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | `rustc 1.85.0` | 2026-07-28 |

The local estimate is collected by the `measure_auth_gap` test in `amm-pool-contract/tests/measure_auth_gap.rs`:

```bash
cargo build --target wasm32v1-none --release -p amm-pool-contract
cargo test -p amm-pool-contract --test measure_auth_gap -- --nocapture
```

The network figure requires a `simulateTransaction` call against Soroban testnet with the same WASM and contract state. The fixture is checked in at [`cargo-budget-report/fixtures/require_auth_benchmark.json`](https://github.com/Tollcraft/soroban-budget-assert/blob/main/cargo-budget-report/fixtures/require_auth_benchmark.json).

---

## Memory bytes

This section records the local-vs-network cost gap for the memory-bytes metric isolated against a pure allocation fixture. The `allocate_vec` function in `amm-pool-contract` pushes `n` elements into a host-resident `Vec<u32>` with no storage or authorization side-effects, so the simulation's reported `result.cost.memBytes` is dominated by the allocation cost itself.

### Figures

| Metric | Local estimate | Network figure | Delta | Fixture | Build profile | Toolchain | Date |
|---|---:|---:|---:|---|---|---|---|
| Memory Bytes | `MEM_LOCAL` | Pending | — | `amm-pool-contract::allocate_vec(10_000)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | `rustc 1.85.0` | 2026-07-28 |

The local estimate is collected by `test_measure_memory_bytes_local_for_issue_122` in `amm-pool-contract/tests/budget_test.rs`:

```bash
cargo build --target wasm32v1-none --release -p amm-pool-contract
cargo test -p amm-pool-contract --test budget_test test_measure_memory_bytes_local_for_issue_122 -- --nocapture
```

The network figure is checked in at [`cargo-budget-report/fixtures/simulate_transaction_response_valid.json`](https://github.com/Tollcraft/soroban-budget-assert/blob/main/cargo-budget-report/fixtures/simulate_transaction_response_valid.json).

---

## Gap stability across input sizes

The following measurements test whether the local-vs-network gap widens or narrows as `n` grows in `do_expensive_work(n)`.

| Input size (n) | Local estimate (native Rust) | Local estimate (WASM) | Testnet simulated | Delta (WASM local − testnet) | Delta (%) |
|---|---|---|---|---|---:|
| 1,000 | 143,887 | 2,661,315 | 1,410,984 | +1,250,331 | +88.6% |
| 10,000 | 143,887 | 2,661,315 | 1,410,984 | +1,250,331 | +88.6% |
| 50,000 | 143,887 | 2,661,315 | 1,410,984 | +1,250,331 | +88.6% |
| 100,000 | 143,887 | 2,661,315 | 1,410,984 | +1,250,331 | +88.6% |

**Build profile:** size-opt (`opt-level="z"`, LTO, `codegen-units=1`)  
**Toolchain:** rustc 1.85.0  
**Date:** 2026-07-28

**Conclusion:** The compute loop (`n` iterations of arithmetic) is invisible to both local and network metering once the storage loop saturates at `n.min(100)`. Both local and testnet CPU instruction costs are invariant with respect to `n` beyond 100 iterations. Tier A margins should be derived from network-simulated measurements at the largest expected input size.

---

## Operation-type coverage

| Operation type | Issue | Status |
|---|---|---|
| Storage-write operations | [#44](https://github.com/Tollcraft/soroban-budget-assert/issues/44) | Measured in mixed-operation and `write_bytes` fixtures |
| Host-function-call operations | [#86](https://github.com/Tollcraft/soroban-budget-assert/issues/86) | Measured (`repeated_sequence`) |
| VM-instruction-heavy operations | [#87](https://github.com/Tollcraft/soroban-budget-assert/issues/87) | Measured (`do_vm_instruction_work`) |
| Memory bytes | [#122](https://github.com/Tollcraft/soroban-budget-assert/issues/122) | In progress (`allocate_vec`) |
| TTL extension | TBD | In progress — calibration test at `amm-pool-contract/tests/calibrate_extend_ttl.rs` |
