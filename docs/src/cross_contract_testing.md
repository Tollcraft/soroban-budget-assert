# Testing Contracts With Cross-Contract Calls

Soroban contracts frequently call other contracts. When using `budget-macros`, budget assertions can be applied to tests involving cross-contract calls just as they would to single-contract tests — the macro reads the total cost accumulated by `env`, which includes all cross-contract invocations.

## Pattern Overview

A cross-contract call test follows the same workflow as a single-contract test, with two additional steps:

1. Register **both** contracts (the caller and the callee) in the test environment.
2. Convert the callee's contract ID to an `Address` and pass it to the caller.

Both contracts can come from the same compiled WASM (if they live in the same crate) or from separate WASM files.

## Example: Two contracts in the same WASM

This example mirrors the `amm-pool-contract` setup, where `ConstantProductPool::do_cross_contract_work` calls `HelperContract::multiply` via `env.invoke_contract`.

### Contracts

{% code title="src/lib.rs" %}
```rust
#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Vec};

#[contract]
pub struct HelperContract;

#[contractimpl]
impl HelperContract {
    pub fn multiply(env: Env, a: u32, b: u32) -> u32 {
        a.wrapping_mul(b)
    }
}

#[contract]
pub struct ConstantProductPool;

#[contractimpl]
impl ConstantProductPool {
    pub fn do_expensive_work(env: Env, n: u32) -> u32 {
        // ... single-contract logic ...
    }

    pub fn do_cross_contract_work(env: Env, other: Address, n: u32) -> u32 {
        let mut result: u32 = 0;
        for i in 0..n {
            let product: u32 = env.invoke_contract(
                &other,
                &symbol_short!("multiply"),
                (i, i),
            );
            result = result.wrapping_add(product);
        }
        result
    }
}
```
{% endcode %}

### Test: Raw Rust Mode

Register both contracts with `env.register()`. Useful for fast iteration, but budget numbers are unreliable (see [Protocol Mechanics](./mechanics.md)).

```rust
#[test]
fn test_cross_contract_raw_rust() {
    let env = Env::default();
    let helper_address: Address = env.register(HelperContract, ());
    let contract_address: Address = env.register(ConstantProductPool, ());
    let client = ConstantProductPoolClient::new(&env, &contract_address);

    env.cost_estimate().budget().reset_unlimited();
    client.do_cross_contract_work(&helper_address, &10_000);

    let budget = env.cost_estimate().budget();
    println!("CPU instructions: {}", budget.cpu_instruction_cost());
}
```

### Test: WASM Mode With Budget Assertion

Run the compiled WASM through `env.register_contract_wasm()` and gate with `#[budget_cpu_lt]`. The macro checks the **total** cost — including the helper contract's execution — after the call returns.

```rust
use budget_macros::budget_cpu_lt;
use soroban_sdk::{Address, Env};

#[test]
#[budget_cpu_lt(2500000)]
fn test_cross_contract_macro_gated() {
    let env = Env::default();

    let wasm = std::fs::read(
        "../target/wasm32-unknown-unknown/release/my_contract.wasm",
    ).expect("build the WASM first");

    #[allow(deprecated)]
    let helper_address: Address = env.register_contract_wasm(None, wasm.as_slice());
    #[allow(deprecated)]
    let contract_address: Address = env.register_contract_wasm(None, wasm.as_slice());
    let client = ConstantProductPoolClient::new(&env, &contract_address);

    env.cost_estimate().budget().reset_unlimited();
    client.do_cross_contract_work(&helper_address, &10_000);
}
```

Both contracts are deployed from the same WASM blob because they compile into a single `cdylib`. When the contracts live in separate crates, read two WASM files and register each one separately.

## Separate crates pattern

If the callee contract lives in a different crate, register each from its own WASM path:

```rust
let helper_wasm = std::fs::read(
    "../target/wasm32-unknown-unknown/release/helper_contract.wasm",
).expect("build the helper WASM first");

let caller_wasm = std::fs::read(
    "../target/wasm32-unknown-unknown/release/caller_contract.wasm",
).expect("build the caller WASM first");

#[allow(deprecated)]
let helper_address: Address = env.register_contract_wasm(None, helper_wasm.as_slice());
#[allow(deprecated)]
let caller_address: Address = env.register_contract_wasm(None, caller_wasm.as_slice());
```

## Key points

{% hint style="warning" %}
- **The budget is cumulative.** The cost measured by `env.cost_estimate().budget()` after a cross-contract call includes all sub-invocations. The macro limit must cover the **entire** call chain.
- **`reset_unlimited()` before the call.** The default test budget caps measurement; without resetting, the assertion limit and the actual cost may not match.
- **Use the `Address` returned by `env.register()` or `env.register_contract_wasm()` directly.** In Soroban SDK 22+, these methods return `Address` instead of `BytesN<32>`.
- **Both contracts must be registered.** A cross-contract call from an unregistered contract panics at test time.
- **Set limits based on local WASM measurements.** Run the test once unlimited to see the cross-contract cost, then pin the limit ~5% above that number. See the [End-User Guide](./user_guide.md) for the full workflow.
{% endhint %}

## Budget report for cross-contract contracts

The `cargo budget-report` CLI discovers every contract in the workspace automatically. Add per-function arguments in `budget.toml` for any exported function that needs them:

```toml
[functions.do_cross_contract_work]
args = ["--other", "CC...", "--n", "10000"]
```

Run the report to get the network-simulated cost of the full cross-contract call chain — the same number that determines whether the transaction succeeds on-chain.
