# End-to-End CI Tutorial

This page is the "wire it up and forget it" guide. It takes you from a contract repo with no automated cost checks to a CI pipeline that fails a pull request the moment a change pushes a function past its budget. The pipeline we're going to build is the one this repository itself runs — every YAML snippet below is copied verbatim from `.github/workflows/budget.yml`, and every command is one we use locally.

If you have not installed the tool or written your first gated test, start with the [End-User Guide](user_guide.md), then come back. This tutorial assumes you have already:

- Run `cargo install --path cargo-budget-report` from this repo.
- Created `budget.toml` at the workspace root.
- Added this repository's `[profile.release]` to the workspace root `Cargo.toml` before recording or comparing budget figures:

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

  The profile is part of the measurement: optimization, LTO, codegen units, panic behavior, strip/debug settings, release assertions, and overflow checks all change the WASM that `cargo budget-report` builds and the local WASM tests load. Numbers from another release profile are not comparable to this project's published figures.
- Run `cargo budget-report` at least once, locally.

## What the two tiers buy you

A one-paragraph recap of the architecture. The full conceptual model lives in [Protocol Mechanics](mechanics.md).

- **Tier A — `#[budget_cpu_lt]` / `#[budget_mem_lt]`**: a local test-time check that runs on every CI push. Fast (no network), deterministic, safe to gate merges on. Wiring it up is two workflow steps: build the contract WASM, run `cargo test`.
- **Tier B — `cargo budget-report`**: a workspace report of *real* testnet-simulated resource costs. Slower (network calls), sensitive to ledger state, and only reliable with a funded identity accessible from CI. The workflow treats this as a *measurement* job: its JSON is uploaded as an artifact rather than wired as a pass/fail gate.

## Prerequisites

- A Soroban contract repo with at least one `cdylib` package.
- Rust and the `wasm32-unknown-unknown` target installed locally.
- Push access to `.github/workflows/*` on the target repo.
- For Tier B only: an account on testnet funded with native XLM (enough to deploy contracts and pay simulation fees), plus permissions to add a GitHub repository secret for the signing key.

## Tier A — gate PRs with budget assertions

The Tier A side of the pipeline is deliberately small: one step to build the WASM, one step to run the gated tests. If `cargo test` passes, the asserted limits held; if it fails, the failure message names the function, the actual cost, and the limit.

### Step 1: Pick a test function and pin a limit

If you've followed the [End-User Guide](user_guide.md), you already have a gated test. The shape you're aiming for is in this repo's `amm-pool-contract/tests/budget_test.rs`:

```rust
#[test]
#[budget_cpu_lt(2_500_000)] // Re-measured: WASM local 2,307,555; network ground truth lives in MEASUREMENTS.md
fn test_budget_macro_gated() {
    let env = soroban_sdk::Env::default();
    let (client, user) = setup_wasm(&env);

    client.deposit(&user, &10_000_i128, &10_000_i128);
    client.swap(&user, &true, &100_i128, &90_i128);
    client.withdraw(&user, &1_000_i128, &900_i128, &900_i128);
}
```

Two refinements matter for CI:

1. **Run the WASM, not raw Rust.** The macro checks the local estimate, and only the WASM estimate is in the right ballpark of network cost. If you `env.register(MyContract, ())` instead of going through `env.register_contract_wasm(...)`, the assertion passes against numbers that can be off by double-digit percentages — see [The measured gap](mechanics.md#the-measured-gap).
2. **Pin the limit from a *local* measurement, and keep the network number in a comment.** Run the test once unlimited, note what it prints, set the limit ~5% above the local number, and write the network number in the same comment. The comment is the only place a reader sees "what the network thinks this function costs".

### Step 2: Add the `budget-check` workflow file

The minimal workflow to gate a PR on a budget regression is the `budget-check` job from this repo's `.github/workflows/budget.yml`, verbatim. It already covers both tiers in one file: Tier A gates the PR (`cargo test`), Tier B produces the network-tracked measurement artifact. The full job, verbatim:

```yaml
name: Soroban Budget Check

on:
  push:
    branches: ["main"]
  pull_request:
    branches: ["main"]

permissions:
  contents: read

jobs:
  budget-check:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4.2.2
        with:
          fetch-depth: 0

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: 1.93.0
          target: wasm32-unknown-unknown

      - name: Install System Dependencies
        run: sudo apt-get update && sudo apt-get install -y libdbus-1-dev pkg-config libudev-dev

      - name: Build Contracts
        run: cargo build -p amm-pool-contract --release --target wasm32-unknown-unknown

      - name: Run Budget Macros Test (Tier A)
        run: cargo test

      - name: Run Budget Report (Tier B)
        run: |
          # Optionally, this uses budget.toml to populate fields instead of args
          # Ensure your repository has budget.toml configured for this to work
          # and ALICE_SECRET_KEY is in your Github Secrets for deploying to testnet
          # cargo run --bin cargo-budget-report -- budget-report --json > current_report.json

          # Mocking the JSON output for the demo so CI passes without testnet secrets:
          echo '[{"package":"amm-pool-contract","function":"do_expensive_work","metric":"CPU Instructions","value":1000000},{"package":"amm-pool-contract","function":"do_expensive_work","metric":"Read Bytes","value":4096}]' > current_report.json

      - name: Upload Budget Report
        uses: actions/upload-artifact@v4.2.0
        with:
          name: budget-report
          path: current_report.json
```

Things to change per-repo:

- Replace `amm-pool-contract` with your package name (or run `cargo build --workspace --release --target wasm32-unknown-unknown` if you have more than one contract).
- Bump `toolchain:` to the pinned version in your `rust-toolchain.toml`. The CI the file ships in this repo and the `rust-toolchain.toml` at the workspace root must agree, or `dtolnay/rust-toolchain` will silently fall back to whatever the runner has installed.
- `cargo test` runs every test in the workspace. If your gated tests live in a single crate, `cargo test -p my-contract` is enough.

Once this file is on `main`, `cargo test` is a pass/fail check on every push and pull request. The macro's failure message — `CPU instruction cost {actual} exceeded limit {N} - local estimate, real network cost may differ significantly in either direction` — is your regression signal.

For the macro reference and the exact failure-message format, see the [Tool Reference](reference.md#tool-reference).

### Step 3: Make the `budget-check` job a required status check

Having the workflow on `main` only produces a green check on the PR — it does not, by itself, *block* anything. To fail merges when this check fails, the `budget-check` job has to be a required status check:

1. On GitHub, open your repo → **Settings** → **Branches**.
2. Add (or edit) a branch protection rule for `main`.
3. Enable **Require status checks to pass before merging**.
4. In **Status checks that are required**, search for `budget-check` (the job name from your workflow). GitHub lists it after the workflow has run at least once on the repo.
5. Save.

Until you do this, a contributor can merge a PR with a failing `budget-check`. After you do this, the merge button stays disabled until the check is green.

> **Notice:** This repo's `record-history` job runs only on `main` (`if: github.ref == 'refs/heads/main'`) and is intentionally *not* a required check — it pushes budget history to a `gh-pages` branch, and a failing history write should not block merges. Make `budget-check` the only required check.

### What Tier A does *not* catch

The macro checks a *local* estimate. On this repo's example contract, the WASM local estimate (901,816 instructions, size-optimized release profile) sat ~19% above the testnet ground truth (756,678). What that means in practice:

- A regression that pushes the local estimate past the limit fails CI — good.
- A regression that pushes the *network* cost up without moving the local estimate by enough to clear the limit passes Tier A. The network-tracked snapshot in Tier B is the second line of defense.

For the build-profile numbers behind the gap, see [MEASUREMENTS.md](../../MEASUREMENTS.md).

## Tier B — network-verified measurement in CI

Tier B is what makes the workflow a *real* CI pipeline rather than a local test runner. It's also the part that depends on a testnet identity you control, and a GitHub Secret to bring it into CI safely.

### Step 1: Create and fund a testnet identity (one-time, locally)

The tool no longer depends on the Stellar CLI. You still need a funded testnet account for deployment and simulation. Create one using any Stellar wallet or the Stellar Lab:

1. Generate a new Ed25519 keypair (e.g. using `stellar keys generate alice --network testnet --fund` if you have the CLI, or use a tool like [Stellar Lab](https://lab.stellar.org/)).
2. Fund the account via Friendbot (see the [Stellar documentation](https://developers.stellar.org/docs/learn/fundamentals/testnet#getting-testnet-xlm)).
3. Note the public key (G...) and secret key (S...) — you will need both.

### Step 2: Store the secret key as `ALICE_SECRET_KEY`

Take the secret key (S... address) and add it as a repository secret named `ALICE_SECRET_KEY`:

1. Repo → **Settings** → **Secrets and variables** → **Actions**.
2. **New repository secret**.
3. Name: `ALICE_SECRET_KEY`. Value: the secret key from the previous command.
4. **Add secret**.

Pick the default "Actions" scope — do not make it environment-scoped; the workflow runs in the default environment.

Treat the secret like a password. The key is only useful on testnet, so a leak is recoverable (re-roll the identity, re-fund), not catastrophic — but a leak of *any* signing key is still a leak.

### Step 3: Wire Tier B into the workflow (the fork-safe fallback)

Look at the `Run Budget Report (Tier B)` step in the workflow above. The shipping shape is the fallback path — the file ships with the `echo`-written placeholder JSON, *not* a live `cargo budget-report` call. The reason is that this repo's workflow is meant to be green on every clone of every fork: PRs from forks don't get repository secrets, so any step that requires `secrets.ALICE_SECRET_KEY` would fail closed.

```yaml
      - name: Run Budget Report (Tier B)
        run: |
          # The tool uses RPC directly and needs --network and --source.
          # --secret-key is required for on-ledger deployment.
          # cargo run --bin cargo-budget-report -- budget-report --network testnet --source G... --secret-key "${{ secrets.ALICE_SECRET_KEY }}" --json > current_report.json

          # Mocking the JSON output for the demo so CI passes without testnet secrets:
          echo '[{"package":"amm-pool-contract","function":"do_expensive_work","metric":"CPU Instructions","value":1000000},{"package":"amm-pool-contract","function":"do_expensive_work","metric":"Read Bytes","value":4096}]' > current_report.json
```

The commented-out line is the real invocation. When you copy the workflow into your own repo and store `ALICE_SECRET_KEY` as a repository secret (Step 2 above), uncommenting that line and exposing the secret to the step is the entire Tier B swap. Once you've authored your own workflow:

1. Replace the `echo` block in the `Run Budget Report (Tier B)` step with a real invocation that reads the secret:

   ```yaml
         - name: Run Budget Report (Tier B)
           run: |
             cargo run --bin cargo-budget-report -- budget-report --network testnet --source G... --secret-key "${{ secrets.ALICE_SECRET_KEY }}" --json > current_report.json
   ```

2. Keep the `upload-artifact` step as-is. The artifact shape (`current_report.json`) is the same regardless of which branch produced it.

Be aware: a workflow that reads `secrets.ALICE_SECRET_KEY` will *fail* on PRs from forks, because forks do not receive repository secrets (this is a GitHub security boundary, not your workflow's problem). To get fork runs to fall through to the placeholder JSON while push runs do the real thing, split the `Run Budget Report (Tier B)` step into two siblings — one gated on `if: github.event_name == 'push'` doing the real `cargo budget-report` call, one gated on `if: github.event_name != 'push'` doing the echo placeholder. Then only push runs pay the testnet call cost; fork PRs still produce a `current_report.json` artifact for inspection.

The `env:` block is the only piece the workflow needs to read the secret. Do not `echo "$ALICE_SECRET_KEY"` in any other step — GitHub masks secrets in the action log, and a fork-PR run will not have one anyway, so doing so just produces a useful-looking empty report and wastes your debugging time.

> **Notice:** The workflow above also uploads `current_report.json` as an artifact (`actions/upload-artifact@v4.2.0`). This is how the report gets into a human's hands without becoming a pass/fail check — if a PR's `budget-check` job is green but you want to read the numbers from that run, download the `budget-report` artifact from the run page.

## Troubleshooting

These are the failure modes we have actually hit while running this workflow.

### Unfunded or reset testnet accounts

Friendbot-funded testnet accounts are reset periodically (network policy wipes inactive accounts), and a freshly funded account can still hit `txInsufficientBalance` if the funding transaction has not yet settled on the RPC node the workflow hits. Symptoms:

- `cargo budget-report` exits non-zero with `source account may be unfunded` in the error chain.
- The deploy step fails with `txBadSeq` or `txInsufficientBalance`.
- The build succeeds, the test runs, and the budget-report upload shows an empty artifact.

Fix: re-fund the account through Friendbot or a Stellar wallet, then re-run the workflow. If the workflow fails on first run after being idle for a week, this is the first thing to check before suspecting the workflow itself.

If Friendbot itself 404s or rate-limits, wait a few minutes and re-try. Friendbot is shared infrastructure, not the workflow's problem.

### Simulation variance between runs

The report's summary line warns: _"These are simulated numbers on testnet and may vary slightly depending on ledger state."_ The `Write Bytes` metric moves more than `CPU Instructions` because the write-fee multiplier grows with the global ledger size. Two consecutive runs of the same WASM can differ by a few percentage points. Treat the report as a snapshot, not a pass/fail signal. If a number regresses by more than ~10% between pushes with no contract change, that warrants a real investigation — the network is telling you something.

### No Stellar CLI dependency on the runner

The tool communicates with the Stellar network through direct RPC calls. The `stellar` CLI is no longer a runtime dependency — no installation or configuration step is needed in CI. The source account and secret key are passed directly via `--source` and `--secret-key` CLI flags (or configured in `budget.toml`).

### Build failures before the Tier A check runs

The workflow builds WASM *before* running tests. If `cargo build -p my-contract --release --target wasm32-unknown-unknown` fails, the gated tests do not run, and the job is red without any budget assertion message in the log. That is not a regression in cost — it is a real build break; fix the build before debugging the budget. The [Developer Guide](developer_guide.md) covers the `wasm32-unknown-unknown` build requirements for Soroban contracts in general; reach for it the moment a Tier A failure shows no budget message in the action log.

### Toolchain mismatch on the runner

The workflow pins the Rust toolchain to `1.93.0` via `dtolnay/rust-toolchain`. If your `rust-toolchain.toml` pins a different version, `rustup` will fetch the right toolchain on the runner — but the macro and tests both live in a workspace that has to build against one toolchain. Mismatches manifest as `cargo test` failing with cryptic "feature stable since 1.XX" errors, not as a budget assertion. Pin both files to the same version.

## Up next: posting the report to `$GITHUB_STEP_SUMMARY`

The current workflow uploads the report as a downloadable artifact. A more discoverable option — writing the same JSON to `$GITHUB_STEP_SUMMARY` so the table appears inline on the run page — is tracked in [#14](https://github.com/Tollcraft/soroban-budget-assert/issues/14). When that lands, this page will pick up a one-liner that pipes `cargo budget-report`'s human-readable table into the step summary, so the run page shows the report without an extra download.

## See also

- [End-User Guide](user_guide.md) — install, `budget.toml`, and the local styles of the Tier A gate.
- [Protocol Mechanics](mechanics.md) — why Tier A's local estimate can drift, how Tier B's pipeline is built.
- [Tool Reference](reference.md) — every flag and macro signature.
- [Cost Terms Glossary](glossary.md) — mapping from `cargo budget-report` rows to XDR fields.
- [MEASUREMENTS.md](../../MEASUREMENTS.md) — the gap between local and network cost, kept up to date.
