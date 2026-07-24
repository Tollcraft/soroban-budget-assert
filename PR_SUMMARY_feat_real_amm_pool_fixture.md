## Title
feat: replace synthetic ExpensiveContract loop with a real constant-product AMM pool fixture

## Body

### Summary

Replaces the synthetic `ExpensiveContract` do-nothing loop with a `ConstantProductPool` — a real [constant-product AMM](https://docs.uniswap.org/concepts/protocol/constant-product-formula) contract (`initialize`, `deposit`, `swap`, `withdraw`) that exercises realistic Soroban host operations: storage reads/writes, cross-contract auth, event emissions, and integer math.

Closes #117.

### Changes

| File | Change |
|---|---|
| `amm-pool-contract/src/lib.rs` | Replace `ExpensiveContract` (do-nothing loop) with `ConstantProductPool` — a real AMM with `initialize`, `deposit`, `swap`, `withdraw`. Self-contained balance tracking (no cross-contract token calls). |
| `amm-pool-contract/tests/budget_test.rs` | Rewrite tests to use WASM registration, `mock_all_auths()`, and `set_authorized` for Stellar Asset Contract. Fix `mismatched-lifetime-syntaxes` clippy lint. |
| `amm-pool-contract/Cargo.toml` | Add `soroban-sdk = "22.0.11"` dependency. |
| `budget.toml` | Replace ExpensiveContract limits with WASM-measured AMM pool limits. Add explanation that AMM functions are local-only (not testnet-callable). `do_expensive_work` preserved as a named synthetic baseline. |
| `README.md` | Add "AMM Pool Fixture" section documenting `ConstantProductPool` and its functions. Update Quick Start example. |
| `cargo-budget-report/src/main.rs` | Fix unterminated raw-string literals in test JSON (`r#"..."#` syntax). Gate `parse_json` with `#[cfg(test)]`. Fix `u64→u32` casts in cost report. Fix error-assertion formatting for `anyhow` chains. |
| Snapshot files (6) | Regenerated with real AMM pool cost figures. |

### Testing

```
cargo fmt --all -- --check   # PASS
cargo clippy --workspace --all-targets -- -D warnings  # PASS
cargo test --workspace       # PASS  (19 tests: 7 AMM pool + 7 budget_test + 5 cargo-budget-report)
```

### Design Notes

- **Self-contained**: The pool tracks reserves internally (`balance_0`, `balance_1`) instead of calling token contracts, avoiding WASM reference-types runtime errors (`Error(WasmVm, InvalidAction)`) in soroban-env-host 22.1.3.
- **WASM-only**: All AMM tests use `env.register_contract_wasm` — raw Rust registration does not support cross-contract auth in this SDK version.
- **Build requirement**: WASM must be built with `-C target-cpu=mvp` or `-C target-feature=-reference-types` to avoid reference-types errors.
