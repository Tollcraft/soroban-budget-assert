# End-User Guide

This guide is for Soroban developers who want budget assertions in an existing contract workspace. The workflow: measure real costs once (Tier B), then pin them into tests that run on every CI push (Tier A).

## Prerequisites

- Rust with the `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`)
- The `stellar` CLI
- A funded testnet identity: `stellar keys generate alice --network testnet --fund`

## Step 1: Install the CLI

From this repository's root:

```bash
cargo install --path cargo-budget-report
```

## Step 2: Configure your workspace

Create `budget.toml` in your workspace root. Supply arguments for any contract function that requires them — functions are discovered and simulated automatically, but the tool can't invent argument values:

{% code title="budget.toml" %}
```toml
network = "testnet"
source = "alice"

[functions.do_expensive_work]
args = ["--n", "10000"]
```
{% endcode %}

## Step 3: Measure network resource usage

```bash
cargo budget-report
```

The CLI finds every contract in the workspace, builds it to WASM, deploys to testnet, simulates every exported function, and prints one table of CPU instructions, read bytes, and write bytes. Use `--json` if you want to feed the numbers to a script.

{% hint style="warning" %}
This is not your transaction fee. The three metrics are inputs to the non-refundable resource fee; rent, refundable fees, transaction size, footprint entry counts, and the inclusion fee are not measured. If you are budgeting what users will actually pay — especially for a contract that writes persistent state, where rent often dominates — read [Measurement scope](reference.md#measurement-scope) first.
{% endhint %}

## Step 4: Pin the costs into tests

Add the macro crate to your contract's dev-dependencies, then gate a test. The macro asserts the *local* WASM estimate, so set the limit from a local measurement: run the test once unlimited, note the printed cost, and pin ~5% above it. Keep the Step 3 network number alongside it in a comment — local and network costs can differ by double-digit percentages in either direction, and the network number is the one that decides whether your transaction succeeds:

```rust
use budget_macros::budget_cpu_lt;
use soroban_sdk::Env;

#[test]
#[budget_cpu_lt(950000)] // local WASM ~901,816; testnet ~756,678
fn test_expensive_function_budget() {
    let env = Env::default();

    let wasm = std::fs::read(
        "../target/wasm32-unknown-unknown/release/my_contract.wasm",
    ).expect("build the WASM first");
    let contract_id = env.register_contract_wasm(None, wasm.as_slice());
    let client = MyContractClient::new(&env, &contract_id);

    env.cost_estimate().budget().reset_unlimited();
    client.do_expensive_work(&10_000);
}
```

Two details matter:

{% hint style="warning" %}
- **Run the WASM, not raw Rust.** Raw Rust estimates ran ~81% below real network cost in our measurements; a limit asserted against them protects nothing.
- **`reset_unlimited()` before the call**, so the default test budget doesn't cap the measurement.
{% endhint %}

Re-measure (Steps 3–4) whenever you change the release profile or bump the SDK — both shift local and network costs, and not by the same amount.

## Step 5: Block regressions in CI

Build the WASM, then run the tests, on every push and pull request:

```yaml
- name: Build contracts
  run: cargo build -p my-contract --release --target wasm32-unknown-unknown

- name: Budget assertions
  run: cargo test
```

If a change pushes a function past its asserted budget, the test fails with the actual cost and the limit in the message. Re-run `cargo budget-report` to re-measure, then either optimize the function or consciously raise the limit.
