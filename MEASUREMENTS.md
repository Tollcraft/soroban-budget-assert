# Measurements

This file records empirical cost measurements comparing local Soroban budget estimates against real network costs. Every measurement PR adds its numbers here so the series stays comparable and in one place.

## Methodology

Each measurement compares a local budget estimate against a network-verified figure for the same operation. The local estimate comes from `Env::cost_estimate().budget()` in a test that registers the contract as WASM with `register_contract_wasm` (except where noted). The network figure comes from `simulateTransaction` on Soroban testnet — the same endpoint the network uses to charge non-refundable resource costs.

The WASM is compiled with the profile specified in the **Build profile** column. The direction of the local-vs-network gap is not stable across profiles; the same contract built with Cargo's default release profile can produce a gap pointing in the opposite direction of one built with the size-optimization profile. Every figure includes its build context.

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

### Memory bytes

| Operation type | Local estimate | Network figure | Delta | Fixture | Build profile | Toolchain | Date |
|---|---|---|---|---|---|---|---|
| Vec<u32> growth + instance-storage write (WASM) | **WASM 403,430,257**; raw-Rust 401,482,058 (baseline only) | pending testnet capture — see Commands section below | pending | `amm-pool-contract::allocate_vec(10_000)` | workspace `[profile.release]` (`opt-level="z"`, LTO, `codegen-units=1`) applied automatically by `--target wasm32v1-none` | rustc 1.85.0 | 2026-07-28 (chore/measure-memory-bytes-gap) |

### Commands (issue #122)

The local figures above came from `cargo test -p amm-pool-contract
--test budget_test test_measure_memory_bytes_local_for_issue_122 --
--nocapture`, which prints both a raw-Rust figure and the figure
under the WASM registration path used by every `--check` test. The
network figure comes from the same fixture run through the testnet
RPC:

```bash
# Build the WASM in the same configuration CI uses
cargo build -p amm-pool-contract --release --target wasm32v1-none

# Deploy to Soroban testnet (source account must be funded)
stellar contract deploy \
  --wasm target/wasm32v1-none/release/amm_pool_contract.wasm \
  --source alice \
  --network testnet

# Capture the contract id from the deploy output and invoke the fixture
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source alice \
  --network testnet \
  --build-only \
  -- allocate_vec --n 10000

# POST the resulting XDR to simulateTransaction; read result.cost.memBytes
curl -s -X POST \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"simulateTransaction","params":{"transaction":"<XDR>"}}' \
  https://soroban-testnet.stellar.org:443 | jq '.result.cost.memBytes'
```

The plain-text or `--json` output of `cargo budget-report` against
the same fixture also surfaces the figure as the `Memory Bytes`
row of the function row. The `delta = (local − network) / network`
percentage belongs in the table above once testnet capture completes.

The local figure for the WASM row is captured by reading the
`=== ISSUE 122 — WASM LOCAL MEMORY BYTES ===` line from `cargo test
test_measure_memory_bytes_local_for_issue_122 -- --nocapture`. The
network figure is the `Memory Bytes` cell of the same fixture row in
the plain text or `--json` output of `cargo budget-report` after a
fresh testnet deploy — `cargo build -p amm-pool-contract --release
--target wasm32v1-none` (the target the project's CI workflow uses,
see `.github/workflows/budget.yml`); then `stellar contract deploy
--wasm target/wasm32v1-none/release/amm_pool_contract.wasm --source
alice --network testnet`; capture the contract id; then `stellar
contract invoke --id <CONTRACT_ID> --source alice --network testnet
-- allocate_vec --n 10000` followed by posting the resulting XDR to
`simulateTransaction` on `https://soroban-testnet.stellar.org:443`.
Both numbers plus the computed `(local − network) / network`
percentage belong in the cell above once the testnet capture
completes; the format mirrors the CPU-instructions rows below so the
two series stay comparable.

### CPU instructions

| Operation type | Local estimate | Network figure | Delta | Fixture | Build profile | Toolchain | Date |
|---|---|---|---|---|---|---|---|
| Mixed compute + storage (native Rust) | 143,887 | 756,678 | −81.0% | `amm-pool-contract::do_expensive_work(10_000)` | N/A (native test, no WASM) | rustc 1.81 | 2025-Q1 |
| Mixed compute + storage (WASM) | 901,816 | 756,678 | +19.2% | `amm-pool-contract::do_expensive_work(10_000)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.81 | 2025-Q1 |
| Mixed compute + storage (WASM) | 767,049 | 832,006 | −7.8% | `amm-pool-contract::do_expensive_work(10_000)` | default `release` (`opt-level=3`) | rustc 1.81 | 2025-Q1 |

The native Rust row is included solely to illustrate that native estimates are unreliable for budget decisions. Only WASM-mode estimates should be used for assertions.

All three rows measure the same `do_expensive_work(10_000)` function, which mixes a compute loop (`n` iterations of `wrapping_add(wrapping_mul)`) with a storage write (`Vec` of up to 100 elements written to `env.storage().instance().set`). The numbers are aggregate costs of both operations.

## Unmeasured operation types

The following operation types have open measurement issues and no published figures yet. When adding a measurement, follow the column format above and include the build profile and toolchain.

| Operation type | Issue | Status |
|---|---|---|
| Storage-write operations | [#44](https://github.com/Tollcraft/soroban-budget-assert/issues/44) | Open |
| Host-function-call operations | [#86](https://github.com/Tollcraft/soroban-budget-assert/issues/86) | Open |
| VM-instruction-heavy operations | [#87](https://github.com/Tollcraft/soroban-budget-assert/issues/87) | Open |
| Memory bytes | [#122](https://github.com/Tollcraft/soroban-budget-assert/issues/122) | Measured (this PR); numbers pending testnet capture |
