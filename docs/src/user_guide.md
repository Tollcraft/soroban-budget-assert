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

Add the release profile used for reproducible Soroban cost measurements to the same workspace root `Cargo.toml`:

{% code title="Cargo.toml" %}
```toml
[profile.release]
opt-level = "z"
overflow-checks = true
debug = 0
strip = "symbols"
debug-assertions = false
panic = "abort"
codegen-units = 1
lto = true
```
{% endcode %}

{% hint style="warning" %}
The release profile is part of the measurement. `cargo budget-report` builds the WASM with `--release`, and these settings change the binary that is deployed, simulated, and loaded by local WASM tests. Size optimization, LTO, and single-codegen-unit builds change generated instructions; aborting panics removes unwinding code; stripping symbols and disabling debug info change artifact size; release assertions match production behavior; and overflow checks keep arithmetic checks explicit. Numbers measured under another profile are not comparable to this project's published figures.
{% endhint %}

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
        "../target/wasm32v1-none/release/my_contract.wasm",
    )
    .expect("WASM file not found — build the contract first");

    // `Env::register` accepts raw WASM bytes: `Register` is implemented for
    // `&[u8]` in soroban-sdk 22.x, and it drives the same host path the
    // deprecated `register_contract_wasm` used. Raw WASM registration (rather
    // than linked-in Rust) is required for accurate CPU/memory budget
    // measurements, since Rust-level estimates undercount costs.
    let contract_id = env.register(wasm.as_slice(), ());

    // Replace `MyContractClient` with the generated client type for your
    // contract, e.g. `MyContractClient::new(&env, &contract_id)`.
    let _client = MyContractClient::new(&env, &contract_id);

    env.cost_estimate().budget().reset_unlimited();
    _client.do_expensive_work(&10_000);
}
```

Two details matter:

{% hint style="warning" %}
- **Run the WASM, not raw Rust.** Raw Rust estimates ran ~81% below real network cost in our measurements; a limit asserted against them protects nothing.
- **`reset_unlimited()` before the call**, so the default test budget doesn't cap the measurement.
{% endhint %}

Re-measure (Steps 3–4) whenever you change the release profile or bump the SDK — both shift local and network costs, and not by the same amount. A useful follow-up for the tool would be to warn when a workspace lacks the release profile above; this guide only documents the requirement.

## Step 5: Block regressions in CI

Build the WASM, then run the tests, on every push and pull request:

```yaml
- name: Build contracts
  run: cargo build -p my-contract --release --target wasm32-unknown-unknown

- name: Budget assertions
  run: cargo test
```

If a change pushes a function past its asserted budget, the test fails with the actual cost and the limit in the message. Re-run `cargo budget-report` to re-measure, then either optimize the function or consciously raise the limit.

## Step 6 (optional): Catch regressions on the workspace with a baseline

The Tier A macros above catch local estimation regressions on a single function at test time. To catch *network-cost* regressions across the whole workspace (without requiring a unit test per function), record a baseline on your trunk branch and check against it on PRs.

On `main` (or whatever trunk you want to gate against), record the baseline:

```bash
cargo budget-report --record-baseline
```

Commit the resulting `budget-baseline.toml`. It looks like:

```toml
[amm-pool-contract.do_expensive_work]
cpu_instructions = 756678
read_bytes      = 2048
write_bytes     = 4096
```

Section headers are sorted alphabetically; the three metric lines inside each block always appear in the same order, so a PR diff against this file only shows the values that actually moved.

In CI on every pull request:

```bash
cargo budget-report --check-baseline
```

The run exits **non-zero** when any metric exceeds its allowed budget under the tolerance. The default tolerance is 10% — chosen for testnet-side variability, since simulations drift with ledger state. Tighten it per function in `budget.toml`:

```toml
tolerance = 0.10                    # global default

[functions.do_expensive_work]
args = ["--n", "10000"]
tolerance = 0.05                    # tighter override for a known-sensitive call
```

A single bad commit can no longer ride the `--check-baseline` gate; the rest of the workflow (tier-A macros, the textual report, `--json` for scripts) is unchanged.

## Step 7 (optional): Watch mode for iterative development

When iterating on a function that is over budget, you want to know whether each change moved the number — without running a full `cargo budget-report` by hand every time. Watch mode automates this loop:

```bash
cargo budget-report --watch
```

### What it does

1. **Watches the workspace** for changes to source files (`.rs`, `.toml`, etc.).
2. **On each change**, rebuilds and re-measures only the affected packages. A change confined to one package does not re-deploy every other package in the workspace.
3. **Prints a delta** comparing the current measurements against the previous run, so you can see the impact of your edit immediately.
4. **Coalesces rapid edits** — saving four times in ten seconds triggers one re-measurement, not four.
5. **Handles build failures gracefully** — if a build fails, it prints the error and keeps watching.
6. **Exits cleanly on Ctrl-C** without leaving deployed contracts or temp files behind.

### Restrictions

- **Interactive terminals only**: watch mode refuses to start when stdout is not a terminal (e.g. in CI). Run without `--watch` for CI.
- **Not usable in pipelines**: the progress output goes to stderr so it does not pollute stdout, but the tool is designed for human-in-the-loop use.

### What counts as a "relevant change"

Only source files under workspace package directories trigger a re-measurement. The following are always excluded:

- `target/` (build output — including the tool's own WASM builds)
- `.git/` (version control metadata)
- `node_modules/` (JavaScript dependencies)
- `.github/` (CI configuration)

This prevents the tool from retriggering on its own build output or on unrelated files.

### Example session

```
$ cargo budget-report --watch
Watch mode active. Watching for source changes...
Press Ctrl-C to stop.

Discovering workspace members...
Building package 'amm-pool-contract' for wasm32...
Contract deployed at: C...
Simulating function 'do_expensive_work'...

=== WORKSPACE BUDGET REPORT ===
  amm-pool-contract::do_expensive_work [CPU Instructions] = 756,678 inst.
  amm-pool-contract::do_expensive_work [Read Bytes] = 2,048 B
  amm-pool-contract::do_expensive_work [Write Bytes] = 4,096 B
Watching for changes...

── Re-measuring workspace ──────────────────────────
  amm-pool-contract::do_expensive_work [cpu_instructions] 756678 -> 720100 (-36578, -5%)

=== WORKSPACE BUDGET REPORT ===
  amm-pool-contract::do_expensive_work [CPU Instructions] = 720,100 inst.
  amm-pool-contract::do_expensive_work [Read Bytes] = 2,048 B
  amm-pool-contract::do_expensive_work [Write Bytes] = 4,096 B
Watching for changes...
```

The delta line shows the direction and percentage of the change, so you can see at a glance whether your optimization moved the needle.

## ⚙️ Supported Versions & Compatibility

* **Supported SDK Version**: `soroban-sdk` = `"22.0.11"` (specifically tested/resolved to `22.0.11` in `Cargo.lock`)
* **Supported XDR Version**: `stellar-xdr` = `"22.1.0"` (used for decoding transaction simulation responses)
* **Corresponding Stellar Protocol**: **Protocol 22**

### Compatibility Matrix

| SDK Version | Protocol Version | Status | Notes |
| :--- | :--- | :--- | :--- |
| **`< 22.0.0`** | `< 22` | **Untested** | Older protocols may use different transaction/resource schemas. |
| **`22.0.x`** | `22` | **Supported** | Matches pinned manifest dependencies (`soroban-sdk` `22.0.11`, `stellar-xdr` `22.1.0`). |
| **`>= 23.0.0`** | `>= 23` | **Untested** | Future protocol upgrades or XDR schema changes (e.g. key/field renames) may break parsing. |
