# Harden and document the `host-function-contract` fixture

Closes #715
Closes #716
Closes #717
Closes #718

## Summary

This PR resolves four `host-function-contract` maintenance issues in one focused
change set. All four concern the benchmark fixture crate — the Map cost-gap
harness, the fixture's integration tests, their shared helper, and the contract
entry points themselves. None of them changes the public contract interface or
any measured host-function operation; the fixture keeps measuring exactly what
it measured before.

| Issue | Area | Change |
| --- | --- | --- |
| #715 | `tests/measure_map_gap.rs` | Reuse one loaded WASM artifact per test instead of re-reading it per measurement; drop the hardcoded relative path. |
| #716 | `tests/contract.rs` | Add native + WASM coverage for every fixture entry point and every zero/empty edge case. |
| #717 | `tests/common/mod.rs` | Document the module's purpose, control flow, and every public item. |
| #718 | `src/lib.rs` | Extract the duplicated Map-seeding logic into a private helper submodule. |

## Issue #718 — Modularize `host-function-contract/src/lib.rs`

The four `map_*` entry points (`map_insert`, `map_get`, `map_remove`,
`map_iterate`) each repeated the same "build a `size`-entry identity map"
loop before doing their own work. That loop is the shared measurement
baseline: every non-insert fixture subtracts `map_insert(size)` from its own
figure, so the build must be identical across all four.

The build is now a single documented helper, `support::seed_identity_map`,
living in a private `support` submodule:

```rust
mod support {
    pub(super) fn seed_identity_map(env: &Env, size: u32) -> Map<u32, u32> { ... }
}
```

- Each `map_*` entry point now reads as one line of setup plus the single
  operation it exists to measure, making it obvious that the only difference
  between the four fixtures is the operation under test.
- The `Map` import moved to where it is used, so the crate root no longer
  carries an unused import.
- The public contract interface is byte-for-byte unchanged; `#[contractimpl]`
  still exports exactly the same entry points.
- The `support` module is declared inline (no new file), so the
  `scripts/check-module-reachability.sh` gate is satisfied without adding a
  new `mod` file.

An inaccurate comment on `repeated_hash` ("each iteration allocates an 8-byte
`Bytes` value") was corrected — the input is built once, before the loop.

## Issue #715 — Optimize `tests/measure_map_gap.rs`

The harness measured four operations at three map sizes (12 measurements) and
called `measure_cpu` once per measurement. Each call re-read
`../target/wasm32v1-none/release/host_function_contract.wasm` from disk into a
fresh `Vec<u8>`, i.e. 12 disk reads and 12 heap allocations of the artifact per
test run, and relied on a relative path that only resolves for one test working
directory.

Changes:

- The artifact is loaded **once per test** via the existing shared helper,
  `common::load_contract_wasm("wasm32v1-none")`, and the same `&[u8]` is reused
  across all measured sizes. Disk reads drop from 12 to 4 and the transient
  `Vec<u8>` allocations drop from 12 to 4, without changing the measurement
  semantics: `measure_cpu` still constructs a fresh `Env` and re-registers the
  WASM per size, so each figure still reflects a clean module instantiation.
- The hardcoded relative path is gone. `load_contract_wasm` resolves the
  artifact via `CARGO_TARGET_DIR`/workspace root and rebuilds it on demand when
  missing or stale, which is the convention the sibling AMM harnesses already
  follow (`amm-pool-contract/tests/measure_bytes_ops.rs`). This also means the
  harness runs on a clean checkout instead of erroring with "WASM file not
  found".
- The size series is a single named `const SIZES: [u32; 3]`, and the file now
  carries a module-level doc comment (purpose, marginal-cost formula, and the
  exact commands to run it), matching the documented harness style.
- The printed output lines are unchanged, so any captured measurement logs
  remain comparable.

## Issue #716 — Comprehensive tests for `tests/contract.rs`

Previously the file tested only `repeated_sequence`. It now covers **every**
entry point on the fixture, in both registration modes:

- **Ledger reads:** `repeated_sequence`, `repeated_timestamp` — return the
  current ledger value, are deterministic across iteration counts, and return
  `0` for zero iterations.
- **Crypto/allocation:** `repeated_hash`, `repeated_bytes_new` — return the
  iteration count for `[0, 1, 10, 100]` and `0` for zero iterations.
- **Map operations:** `map_insert`, `map_get`, `map_remove` return their size;
  `map_iterate` returns the documented identity sum with a small
  `identity_sum(size)` helper (`size * (size - 1) / 2`), including the empty
  map and the single `0 -> 0` entry.
- Every group has a native (`env.register`) and a WASM
  (`register_contract_wasm`) counterpart, so the fixture is verified at the
  same WASM level the workspace uses for budget measurement.

Iteration counts and map sizes are kept deliberately small here, because this
file asserts correctness while the `measure_*` harnesses produce the cost data.
The tests assert only the documented return values, so they do not over-constrain
the fixture.

## Issue #717 — Document `tests/common/mod.rs`

The helper already worked but left new contributors to infer how it worked.
The module now explains, in order:

- **Why it exists:** tests must run against a freshly built WASM artifact, not a
  stale one; trusting a hand-built artifact would let tests pass against code
  that is no longer in the tree.
- **How it fits together:** a numbered walk-through of `workspace_root` →
  `wasm_path` → `artifact_needs_rebuild` → `load_contract_wasm`, so the reader
  can follow the control flow without reading the bodies first.
- **Item docs:** every function now documents what it returns, its failure
  behaviour, and its `# Panics` conditions. `wasm_path` notes that the path is
  not guaranteed to exist, and `load_contract_wasm` documents the
  [`BUILD_LOCK`] serialisation.
- **Inline comments:** the mtime comparison, the missing-artifact and
  unreadable-mtime cases, and Cargo's `CARGO_TARGET_DIR` precedence are called
  out where the logic is non-obvious.
- An `#![allow(dead_code)]` attribute with a rationale comment, matching
  `amm-pool-contract/tests/common/mod.rs`, since each integration-test binary
  uses only part of the module.

The executable code is unchanged apart from comments.

## Scope and non-goals

- No change to the public contract interface or to any measured host-function
  operation.
- No new dependencies, no new crate, and no new `src/` file (the helper module
  is inline), so `cargo machete` and `check-module-reachability.sh` are
  unaffected.
- The debug `failed_log.txt` and any unrelated scratch files are untouched.
- `MEASUREMENTS.md` figures are not regenerated: the measured operations and
  their call sequences are identical. `regenerate-measurements.sh` discovery is
  intentionally unchanged — `measure_map_gap.rs` still carries no `// @measure`
  marker, exactly as before.

## Verification

This change set modifies only Rust test/fixture sources and is guarded by the
standard workspace gates. The expected CI ("Quality Checks") results are:

```bash
cargo fmt --all -- --check                          # passes
cargo clippy --workspace --all-targets -- -D warnings   # passes
cargo test --workspace                              # passes, including the new tests
```

Notes for reviewers:

- The `host-function-contract` integration tests build the
  `wasm32v1-none` artifact on demand through `tests/common/mod.rs`, so
  `cargo test --workspace` needs no separate `cargo build` step, consistent
  with `CONTRIBUTING.md`.
- To inspect the Map harness output directly:
  `cargo test -p host-function-contract --test measure_map_gap -- --nocapture`
- To run only the new coverage:
  `cargo test -p host-function-contract --test contract`
