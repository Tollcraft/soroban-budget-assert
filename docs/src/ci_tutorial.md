# End-to-End CI Tutorial

This page is the "wire it up and forget it" guide. It takes you from a contract repo with no automated cost checks to a CI pipeline that fails a pull request the moment a change pushes a function past its budget. The pipeline we're going to build is the one this repository itself runs — the job structure below mirrors `.github/workflows/budget.yml`, and every command is one we use locally.

One framing note before we start. Our own `budget.yml` is a deliberately *reduced* case: its Tier B step currently emits placeholder JSON so the job stays green without testnet secrets, and other work is reinstating live reporting behind event gates. Rather than transcribe that reduction (and go stale the day it changes), this page teaches the pattern the file implements — **Tier A everywhere, no secrets; Tier B only where secrets exist** — and calls out the reductions where they happen. If a snippet below and the shipped file ever disagree on details, copy the pattern, not the file.

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

- **Tier A — `#[budget_cpu_lt]` / `#[budget_mem_lt]`**: a local test-time check that runs on every CI push. Fast (no network), deterministic, safe to gate merges on. Wiring it up is two workflow steps: build the contract WASM, run `cargo test`. Needs no secrets, so it runs unchanged on fork PRs.
- **Tier B — `cargo budget-report`**: a workspace report of *real* testnet-simulated resource costs. Slower (network calls), sensitive to ledger state, and only reliable with a funded identity accessible from CI. Treat it as a *measurement* job, not a pass/fail gate: its JSON is uploaded as an artifact and its table is rendered into the run-page step summary. (`--check` can turn it into a hard gate against `budget.toml` limits — see [Tool Reference](reference.md) — but that requires secrets on every run, which collides with the constraint below.)

## The fork-PR constraint: plan for secrets before you write YAML

Read this before writing any workflow YAML, because it decides the shape of the whole file.

GitHub withholds repository secrets — everything behind `secrets.*`, including your `ALICE_SECRET_KEY` — from any workflow run triggered by a `pull_request` event **opened from a fork**, and hands the job a read-only `GITHUB_TOKEN`. This is a deliberate security boundary: an untrusted PR must not be able to exfiltrate the credentials your workflows hold.

It bites because every external contribution to an open-source repo arrives exactly that way. A naive workflow that configures a testnet identity in an ungated step is green on every maintainer push and red on every contributor PR — the worst possible failure mode, because your own pushes never show it.

We hit exactly this. The comment our own workflow carries explains the fix:

```yaml
      # No Stellar CLI or testnet identity is installed here on purpose.
      # The Tier B step below is mocked, so nothing in this job reaches the
      # network, and `secrets` are withheld from pull_request runs on forks —
      # where every contribution to this repo comes from. Gating the job on a
      # secret it never uses made it fail on every contributor PR.
      #
      # To restore real Tier B reporting, add `if: github.event_name == 'push'`
      # to the reinstated CLI/identity steps and un-comment the budget-report
      # invocations below, so fork PRs still run Tier A only.
```

(`.github/workflows/budget.yml`)

The rule that comment encodes, and the one this page implements:

> **Tier A runs everywhere and needs no secrets. Tier B runs only where secrets exist — gate every secret-consuming step on `if: github.event_name == 'push'`, or move it to a separate push-only job.**

With those gates in place, fork PRs skip the testnet steps entirely (or fall back to placeholder output — both variants shown below), and only pushes to your own branches pay the network cost.

> **Notice:** `pull_request` runs from branches *inside* the same repository do receive secrets. You cannot rely on that, though — first-time contributors cannot open those, so plan for the fork case or your required check will fail on precisely the PRs you most want to merge.

## Prerequisites

- A Soroban contract repo with at least one `cdylib` package.
- Rust and the WASM target installed locally (this repo builds `wasm32v1-none`; older SDK setups use `wasm32-unknown-unknown`).
- Push access to `.github/workflows/*` on the target repo.

For Tier B only:

- The `stellar` CLI installed locally (used once for the identity setup).
- An account on testnet funded by Friendbot (the Stellar CLI's `--fund` flag drops you there automatically), plus permissions to add a GitHub repository secret.

## Tier A — gate PRs with budget assertions

The Tier A side of the pipeline is deliberately small: one step to build the WASM, one step to run the gated tests. If `cargo test` passes, the asserted limits held; if it fails, the failure message names the function, the actual cost, and the limit. Nothing in this half reads a secret, which is what makes it fork-proof.

### Step 1: Pick a test function and pin a limit

If you've followed the [End-User Guide](user_guide.md), you already have a gated test. The shape you're aiming for is in this repo's `amm-pool-contract/tests/budget_test.rs`, verbatim:

```rust
#[test]
#[budget_cpu_lt(env_file = TIER_A_LIMITS_FILE, env = "TIER_A__AMM_POOL_CONTRACT__SCENARIO__FULL_WORKFLOW__CPU", baseline = baseline_cpu())]
fn test_budget_wasm() {
    let env = soroban_sdk::Env::default();
    let (client, user) = setup_wasm(&env);

    client.deposit(&user, &10_000_i128, &10_000_i128);
    client.swap(&user, &true, &100_i128, &90_i128);
    client.withdraw(&user, &1_000_i128, &900_i128, &900_i128);
}
```

Two refinements matter for CI:

1. **Run the WASM, not raw Rust.** The macro checks the local estimate, and only the WASM estimate is in the right ballpark of network cost. `setup_wasm` above goes through `env.register_contract_wasm(...)` loading the file built by `cargo build --target wasm32v1-none`; if you `env.register(MyContract, ())` instead, the assertion passes against numbers that can be off by double-digit percentages — see [The measured gap](mechanics.md#the-measured-gap).
2. **Pin the limit from a measurement, not a guess.** The simplest form is a literal — `#[budget_cpu_lt(2_500_000)]` — pinned ~5% above a local run, with the network number kept in a comment next to it. This repo has moved past literals: limits are *derived* from a fresh Tier B network report into `tier-a-limits.env` via `cargo budget-report --derive-limits`, with explicit per-metric margin multipliers recorded in `budget.toml`. The annotation reads the key out of that file, and the derivation is auditable line by line in [`tier-a-limits.provenance.md`](../../tier-a-limits.provenance.md). Both styles work in CI identically — `cargo test` neither knows nor cares where the number came from. See [Deriving Limits](deriving_limits.md) for the full flow.

### Step 2: Add a Tier A-only `budget-check` workflow (works without secrets)

This is the minimal workflow worth starting with — the `budget-check` job from this repo's `.github/workflows/budget.yml` with its Tier B steps removed. Because nothing in it reads `secrets`, it runs green on fork PRs exactly as it does on your own pushes:

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
      - uses: actions/checkout@v7
        with:
          fetch-depth: 0

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: 1.91.0
          targets: wasm32v1-none wasm32-unknown-unknown

      - name: Install System Dependencies
        run: sudo apt-get update && sudo apt-get install -y libdbus-1-dev pkg-config libudev-dev

      - name: Build Contracts
        run: cargo build -p amm-pool-contract --release --target wasm32v1-none

      - name: Run Budget Macros Test (Tier A)
        run: cargo test --workspace
```

Things to change per-repo:

- Replace `amm-pool-contract` with your package name (or run `cargo build --workspace --release --target wasm32v1-none` if you have more than one contract). Match the target to the one your tests load — this repo's tests read `../target/wasm32v1-none/release/<contract>.wasm`.
- Bump `toolchain:` to the version you measure with locally. The pin in the workflow is what the runner installs; keep it aligned with the `channel` in your `rust-toolchain.toml` or local and CI numbers will drift apart.
- `cargo test --workspace` runs every test in the workspace. If your gated tests live in a single crate, `cargo test -p my-contract` is enough.
- `fetch-depth: 0` is only strictly needed if a second job will diff or archive reports per commit (see [Recording cost history](#recording-cost-history-on-gh-pages)); it is harmless otherwise.

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

> **Notice:** Make `budget-check` (Tier A) the *only* required check from this pipeline. This repo's `record-history` job runs only on `main` (`if: github.ref == 'refs/heads/main'`) and is intentionally *not* required — it pushes budget history to a `gh-pages` branch, and a failing history write should not block merges.

### What Tier A does *not* catch

The macro checks a *local* estimate. On this repo's example contract, the WASM local estimate (901,816 instructions, size-optimized release profile) sat ~19% above the testnet ground truth (756,678). What that means in practice:

- A regression that pushes the local estimate past the limit fails CI — good.
- A regression that pushes the *network* cost up without moving the local estimate by enough to clear the limit passes Tier A. The network-tracked snapshot in Tier B is the second line of defense.

For the build-profile numbers behind the gap, see [Measurements](measurements.md).

## Tier B — network-verified measurement in CI

Tier B is what makes the workflow a *real* CI pipeline rather than a local test runner. It depends on a testnet identity you control and a GitHub Secret to bring it into CI safely — which means every piece of it lives behind the fork-PR gates from [the constraint above](#the-fork-pr-constraint-plan-for-secrets-before-you-write-yaml).

### Step 1: Create and fund a testnet identity (one-time, locally)

You're going to need an account on testnet that holds enough native XLM to deploy contracts and pay a handful of simulation fees. Use the Stellar CLI's built-in Friendbot funding so you do not have to touch a faucet page:

```bash
stellar keys generate alice --network testnet --fund
```

`--network testnet --fund` creates a local keypair and tells Friendbot to airdrop ~10,000 XLM into it. Confirm with:

```bash
stellar keys show alice
```

You should see a public key starting with `G...` — that is the on-chain address. Friendbot funding is instant on testnet.

> **Notice:** Friendbot-funded testnet accounts are periodically reset by network policy (inactive accounts get wiped). If a workflow run fails on first attempt after the repo has been idle for a week, re-fund with `stellar keys fund alice --network testnet` before debugging further — see [Troubleshooting](#troubleshooting).

### Step 2: Store the secret key as `ALICE_SECRET_KEY`

The same local identity has a secret key beginning with `S...`. The Stellar CLI prints it on demand:

```bash
stellar keys show alice --secret
```

Take the output and add it as a repository secret named `ALICE_SECRET_KEY`:

1. Repo → **Settings** → **Secrets and variables** → **Actions**.
2. **New repository secret**.
3. Name: `ALICE_SECRET_KEY`. Value: the secret key from the previous command.
4. **Add secret**.

Pick the default "Actions" scope — do not make it environment-scoped; the workflow runs in the default environment.

Treat the secret like a password. The key is only useful on testnet, so a leak is recoverable (re-roll the identity, re-fund), not catastrophic — but a leak of *any* signing key is still a leak.

### Step 3: Run Tier B only where secrets exist

Add these steps to the `budget-check` job from Tier A above. Each one that touches the network or the secret carries the same `if:` guard, so fork PRs skip the whole group while push runs get the full report:

```yaml
      - name: Install Stellar CLI
        if: github.event_name == 'push'
        run: |
          curl -sL https://github.com/stellar/stellar-cli/releases/download/v21.5.3/stellar-cli-21.5.3-x86_64-unknown-linux-gnu.tar.gz | tar -xz
          mv stellar ~/.cargo/bin/

      - name: Configure Stellar Identity
        if: github.event_name == 'push'
        env:
          ALICE_SECRET_KEY: ${{ secrets.ALICE_SECRET_KEY }}
        run: stellar keys add alice --secret-key "$ALICE_SECRET_KEY"

      - name: Run Budget Report (Tier B)
        if: github.event_name == 'push'
        run: |
          cargo run --bin cargo-budget-report -- budget-report --json --validate > current_report.json
```

Details worth getting right:

- The identity name (`alice`) must match the `source` field in your `budget.toml` — the report reads it from there, not from the CLI args.
- The secret reaches the step through an `env:` mapping and is referenced as `"$ALICE_SECRET_KEY"` inside the script. Do not interpolate `${{ secrets.ALICE_SECRET_KEY }}` directly into `run:` lines, and do not `echo` the variable anywhere — GitHub masks secrets in logs, but direct interpolation also invites shell-quoting surprises.
- The tool has a built-in retry policy for transient RPC failures (`--max-retry-attempts`, `--retry-backoff-secs`, or the `[retry]` block in `budget.toml`). You should not need to hand-roll retry loops around the invocation anymore — see [Tool Reference](reference.md).
- `--validate` cross-checks reported metrics against the Stellar CLI's own XDR decoder. Optional, but it turns silent metric drift into a loud failure.

Because these three steps are skipped on fork PRs, anything downstream that consumes `current_report.json` has to tolerate its absence. Guard those steps with:

```yaml
        if: hashFiles('current_report.json') != ''
```

#### Variant: placeholder fallback instead of skipping

If you would rather have every run produce a report artifact — some teams like the artifact timeline to be unbroken — split Tier B into two siblings instead of guarding the consumers:

```yaml
      - name: Run Budget Report (Tier B)
        if: github.event_name == 'push'
        run: |
          cargo run --bin cargo-budget-report -- budget-report --json --validate > current_report.json

      - name: Write Placeholder Report (fork PR)
        if: github.event_name != 'push'
        run: |
          # Mocking the JSON output so downstream steps still find a file:
          echo '[{"package":"amm-pool-contract","function":"do_expensive_work","metric":"CPU Instructions","value":1000000},{"package":"amm-pool-contract","function":"do_expensive_work","metric":"Read Bytes","value":4096}]' > current_report.json
```

Then only push runs pay the testnet call cost; fork PRs still produce a `current_report.json` artifact for inspection. Two rules if you take this variant:

- **Never feed placeholder rows into history or enforcement.** The values are synthetic. Gate anything durable on `github.event_name == 'push'` regardless.
- **Label the placeholder loudly** (step name, comment, marker field in the JSON) so nobody mistakes a mocked 1,000,000 for a measurement.

### Why this repo's own file mocks Tier B instead

Our shipped `budget.yml` takes the placeholder variant one step further: its Tier B step *always* writes the mock, even on pushes, and no Stellar CLI is installed at all (see the comment block quoted [above](#the-fork-pr-constraint-plan-for-secrets-before-you-write-yaml)). That is a deliberately reduced case chosen so the job is green on every clone of every fork with zero configuration — the summary-table and history steps expect `current_report.json` to exist on every run, and the mock satisfies them cheapest. Reinstating live reporting behind the `if: github.event_name == 'push'` gates shown here is exactly the migration path the file's own comment describes. Other issues own that change; this page documents the destination pattern so it stays correct before, during, and after.

> **Notice:** Whichever variant you ship, keep uploading `current_report.json` as an artifact (`actions/upload-artifact`). This is how the report gets into a human's hands without becoming a pass/fail check — if a PR's `budget-check` job is green but you want to read the numbers from that run, download the `budget-report` artifact from the run page.

## Publishing the report to `$GITHUB_STEP_SUMMARY`

An artifact you download once is easy to ignore. The run-page step summary is where people actually look — `$GITHUB_STEP_SUMMARY` is a file-backed Markdown surface GitHub renders on the workflow run's Summary tab, and on pull requests it is surfaced with the check run. Appending to it from a step is one redirect.

Our workflow dedicates a step to it right after the report is produced:

```yaml
      - name: Publish Step Summary
        run: |
          {
            echo "# Workspace Budget Report"
            echo ""
            echo "| Function | CPU Instructions | Read Bytes | Write Bytes |"
            echo "|----------|-----------------|------------|-------------|"
            echo "| do_expensive_work | 1,000,000 inst. | 4,096 B | - |"
            echo ""
            echo "---"
            echo "_Simulated resource amounts, not fees._"
          } >> "$GITHUB_STEP_SUMMARY"
```

(That is the shipped file's step verbatim — it renders the mocked row set directly. When your Tier B produces real rows, generate the table from the JSON instead of hand-writing it:)

```yaml
      - name: Publish Step Summary
        if: hashFiles('current_report.json') != ''
        run: |
          {
            echo "# Workspace Budget Report (${{ github.sha }})"
            echo ""
            echo "| Package | Function | Metric | Value |"
            echo "|---------|----------|--------|-------|"
            jq -r '.[] | "| \(.package) | \(.function) | \(.metric) | \(.value) |"' current_report.json
            echo ""
            echo "---"
            echo "_Simulated resource amounts, not fees._"
          } >> "$GITHUB_STEP_SUMMARY"
```

The JSON rows are `{package, function, metric, value}` objects — one per metric per simulated function (`CPU Instructions`, `Read Bytes`, `Write Bytes`, `WASM Bytes`) — so the `jq` filter yields one table row each. See [Output](reference.md#output) for the exact shape.

Two gotchas:

- GitHub renders the summary as **Markdown**. The tool's plain-text table (box-drawing characters) looks wrong there; emit GitHub-flavored pipe tables as above. (A native Markdown output mode for the report itself is planned but not shipped yet — the `jq` route is the working pattern today.)
- The summary accumulates across steps in one run. Append, don't truncate, and give your section a heading so it reads cleanly alongside other jobs' contributions.

## Recording cost history on gh-pages

Artifacts expire and step summaries scroll away. The third leg of the pipeline is a permanent time series: one JSON dataset on the `gh-pages` branch with one entry per push to `main`, ready for dashboards and long-horizon regressions.

This is the `record-history` job from this repo's `.github/workflows/budget.yml`, condensed but structurally identical:

```yaml
  record-history:
    needs: budget-check
    if: github.ref == 'refs/heads/main'
    runs-on: ubuntu-latest
    concurrency:
      group: gh-pages-deploy
      cancel-in-progress: false
    permissions:
      contents: write

    steps:
      - uses: actions/checkout@v7
        with:
          fetch-depth: 0

      - name: Download Budget Report
        uses: actions/download-artifact@v8
        with:
          name: budget-report

      - name: Record History Dataset
        run: |
          # Configure Git for the bot
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"

          # Save report to temp location so it survives the checkout
          cp current_report.json /tmp/current_report.json

          # Clean up any modified files from testing before switching branches
          git reset --hard
          git clean -fd

          # Fetch and checkout the gh-pages branch, or create it if missing
          git fetch origin gh-pages || true
          git checkout gh-pages || git checkout -b gh-pages
          git pull origin gh-pages || true

          # Initialize history.json if it doesn't exist
          if [ ! -f history.json ]; then
            echo "[]" > history.json
          fi

          # Append the new report to history.json using jq
          jq --arg commit "${{ github.sha }}" \
             --arg time "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
             --slurpfile report /tmp/current_report.json \
             '. + [{commit: $commit, timestamp: $time, data: $report[0]}]' history.json > history_new.json

          mv history_new.json history.json

          # Commit and push to gh-pages branch
          git add history.json
          git commit -m "chore: record budget history for ${{ github.sha }}" || echo "No changes to commit"
          git push origin gh-pages
```

What each piece is doing and why:

- **`needs: budget-check` + artifact download.** The report was produced in the first job; the artifact is the handoff. Keeping the jobs separate means a broken history write can never fail your required Tier A check.
- **`if: github.ref == 'refs/heads/main'`.** Only pushes to `main` append history — PR runs measure but do not record. This is also quietly the fork story resolving itself: a `push` to `main` only ever happens inside the base repo (maintainers merge), where secrets and a writable token exist. Fork PR runs never reach this job.
- **`permissions: contents: write`.** The workflow-wide grant stays `contents: read`; only this job escalates, and only as far as it needs, to push to `gh-pages` with the built-in `GITHUB_TOKEN`.
- **`concurrency: group: gh-pages-deploy, cancel-in-progress: false`.** Two pushes landing minutes apart would otherwise race on the same branch and one would lose its datapoint to a non-fast-forward rejection. Serializing writes queues them instead; `cancel-in-progress: false` matters because cancelling the *later* run would drop a measurement forever. (In this repo the same group also serializes against the docs site deploy, which writes to the same branch.)
- **`cp` to `/tmp` before `git reset --hard` / `git clean -fd`.** The checkout workspace is dirty from the test run; switching branches with untracked files in tow fails or drags junk along. The report survives the cleanup outside the tree.
- **The `jq --slurpfile` append.** Reads the existing `history.json`, appends `{commit, timestamp, data}` where `data` is the parsed report array, writes atomically to a new file, moves it back. Idempotent-ish by construction; the trailing `|| echo "No changes to commit"` keeps identical reruns green.
- **Not a required status check.** A failing history append is an observability problem, not a regression. Blocking merges on it converts a dashboard outage into a team outage.

If you want the same durability without a branch dance, the modern alternative is uploading `history.json` as an artifact with a retention of 90 days or committing it back to `main` — the `jq` append logic transfers unchanged. We picked `gh-pages` because the branch already existed for the docs site.

## Customization

### Running on multiple Rust versions

If your project supports multiple Rust toolchains, use a build matrix:

```yaml
strategy:
  matrix:
    toolchain: ["1.93.0", "stable"]

steps:
  - name: Install Rust
    uses: dtolnay/rust-toolchain@stable
    with:
      toolchain: ${{ matrix.toolchain }}
      targets: wasm32v1-none wasm32-unknown-unknown
```

Note that changing the Rust version may produce different WASM and therefore different budget numbers. The pinned version in `rust-toolchain.toml` is what this project's measurements are based on.

### Limiting execution to pull requests

To run budget checks only on pull requests (not on every push to `main`), remove the `push` trigger:

```yaml
on:
  pull_request:
    branches: ["main"]
```

### Running only on selected branches

To limit execution to specific branches:

```yaml
on:
  push:
    branches: ["main", "develop"]
  pull_request:
    branches: ["main", "develop"]
```

### Integrating with existing CI pipelines

You can merge the budget check into an existing workflow file by copying the relevant steps. The minimum required steps for Tier A (local, CI-blocking) are:

```yaml
- name: Build Contracts
  run: cargo build -p your-contract --release --target wasm32v1-none
- name: Run Budget Macros Test
  run: cargo test --workspace
```

Add the Tier B steps when you need network-verified cost measurements and have configured a testnet identity with the `ALICE_SECRET_KEY` secret.

### Fork-safe fallback for Tier B

Pull requests from forks do not have access to repository secrets — GitHub withholds them by design, because the PR author is untrusted. This is why every secret-consuming step in the example above (CLI install, identity import, Tier B report) is gated on `github.event_name == 'push'`, and why a placeholder-writing sibling covers fork PRs so downstream steps still find `current_report.json`.

The minimal shape of the split:

```yaml
- name: Run Budget Report (push)
  if: github.event_name == 'push'
  run: cargo run --bin cargo-budget-report -- budget-report --json --validate > current_report.json

- name: Run Budget Report (fork / pull request)
  if: github.event_name != 'push'
  run: echo '[{"package":"your-contract","function":"your_function","metric":"CPU Instructions","value":0}]' > current_report.json
```

Rules for the placeholder path:

- Label it loudly (step name and comment) so nobody mistakes synthetic rows for measurements.
- Never feed placeholder output into cost history (`record-history`-style jobs) or `--check` enforcement; gate anything durable on the push event regardless.
- The simpler alternative is to skip Tier B on fork PRs entirely (no sibling step) and guard consumers with `if: hashFiles('current_report.json') != ''`. Use that when you do not need an artifact from every run.

This repo's own workflow hit the failure mode before it was documented: gating the job on a secret the job did not need made every contributor PR red, and the fix (dropping the testnet steps entirely until they are reinstated behind event gates) is recorded in a comment in `.github/workflows/budget.yml`. The [End-to-End CI Tutorial](ci_tutorial.md#the-fork-pr-constraint-plan-for-secrets-before-you-write-yaml) covers the constraint in depth.

---

## Best Practices

### Fail builds on budget regressions

Make the `budget-check` job (or its Tier A equivalent) a required status check in your branch protection rules. This prevents merging any pull request that would push a function past its budget.

### Keep budget baselines current

Re-run `cargo budget-report --json` and re-derive Tier A limits whenever:
- The contract source changes.
- The release profile in `Cargo.toml` changes.
- The Soroban SDK version changes.
- The `[margin]` block in `budget.toml` changes.

### Avoid unnecessary workflow duplication

If you already have a CI workflow that builds and tests your contracts, add the budget steps to that existing workflow rather than creating a separate one. The Tier A steps (`cargo build` + `cargo test`) integrate naturally into any Rust CI pipeline.

### Validate changes before merging

Require the `budget-check` job to pass before merging. The branch protection rule for `main` in this repository already requires the `Quality Checks` status check. Add `budget-check` to that list so a budget regression blocks the merge alongside formatting, Clippy, and test failures.

### Use the same release profile

Always build WASM with the same `[profile.release]` settings locally and in CI. The published measurements in this repository use the size-optimized profile (`opt-level = "z"`, `lto = true`, etc.). Numbers from a different profile are not comparable. Copy the profile from `Cargo.toml` into your workspace before recording or comparing budget figures.

---

## Troubleshooting

These are the failure modes we have actually hit while running this workflow.

### A step that reads a secret fails only on contributor PRs

**Symptom:** The job is green on your pushes and red on every fork PR, failing in (or right after) the step that consumes `secrets.*` — usually with an empty-string argument error.

**Fix:** This is the fork-PR constraint, not a misconfigured secret. Gate every secret-consuming step on `if: github.event_name == 'push'` (or split it into a push-only job) per [the fork-PR section](#the-fork-pr-constraint-plan-for-secrets-before-you-write-yaml), and decide whether fork PRs skip Tier B or fall back to a placeholder.

### Unfunded or reset testnet accounts

Friendbot-funded testnet accounts are reset periodically (network policy wipes inactive accounts), and a freshly funded account can still hit `txInsufficientBalance` if the funding transaction has not yet settled on the RPC node the workflow hits. Symptoms:

- `cargo budget-report` exits non-zero with `source account may be unfunded` in the error chain.
- The deploy step fails with `txBadSeq` or `txInsufficientBalance`.
- The build succeeds, the test runs, and the budget-report upload shows an empty artifact.

Fix:

```bash
stellar keys fund alice --network testnet
```

For deeper rot (the account exists but has zero balance after a long gap), re-run the funding command and re-try. If the workflow fails on first run after being idle for a week, this is the first thing to check before suspecting the workflow itself.

If the funding command itself 404s or rate-limits, Friendbot is having a bad day — wait a few minutes and re-try. Friendbot is shared infrastructure, not the workflow's problem.

### Simulation variance between runs

The report's summary line warns: _"These are simulated numbers on testnet and may vary slightly depending on ledger state."_ The `Write Bytes` metric moves more than `CPU Instructions` because the write-fee multiplier grows with the global ledger size. Two consecutive runs of the same WASM can differ by a few percentage points. Treat the report as a snapshot, not a pass/fail signal. If a number regresses by more than ~10% between pushes with no contract change, that warrants a real investigation — the network is telling you something.

### stellar CLI missing on the runner

The `stellar` CLI is not on GitHub-hosted `ubuntu-latest` runners out of the box. If you follow the Tier B steps above, install it directly rather than `cargo install` — the prebuilt tarball is ~30 MB and avoids a multi-minute Rust build inside CI:

```yaml
      - name: Install Stellar CLI
        if: github.event_name == 'push'
        run: |
          curl -sL https://github.com/stellar/stellar-cli/releases/download/v21.5.3/stellar-cli-21.5.3-x86_64-unknown-linux-gnu.tar.gz | tar -xz
          mv stellar ~/.cargo/bin/
```

Bump the version number in the URL (`stellar-cli-X.Y.Z-x86_64-unknown-linux-gnu.tar.gz`) when you upgrade — there is no auto-update. The release page publishes these tarballs under each GitHub release (`https://github.com/stellar/stellar-cli/releases`). Tier A-only pipelines never need the CLI on the runner at all.

### Build failures before the Tier A check runs

The workflow builds WASM *before* running tests. If `cargo build -p my-contract --release --target wasm32v1-none` fails, the gated tests do not run, and the job is red without any budget assertion message in the log. That is not a regression in cost — it is a real build break; fix the build before debugging the budget. The [Developer Guide](developer_guide.md) covers the WASM build requirements for Soroban contracts in general; reach for it the moment a Tier A failure shows no budget message in the action log.

### Toolchain mismatch between local and CI numbers

The workflow pins the Rust toolchain explicitly (`toolchain: 1.91.0` via `dtolnay/rust-toolchain` in the snippets above). If that pin disagrees with the `channel` in your `rust-toolchain.toml` — the thing your local measurements were taken under — the compiler versions differ and the WASM differs with them. Mismatches rarely announce themselves as such: they show up as budget numbers that creep for no reviewable reason, or as `cargo test` failing with cryptic "feature stable since 1.XX" errors. Pick one version, pin it in both places, and bump them together.


### Dependency caching problems
**Symptom**: The `Swatinem/rust-cache` step takes a long time or produces a cache miss on every run.

**Fix**: Ensure `Cargo.lock` is checked into the repository. The cache key is derived from `Cargo.lock` contents. Without it, the cache cannot detect dependency changes efficiently. If cache entries grow stale, clear the cache from the GitHub Actions UI (Settings → Actions → Caches).

### Failing budget assertions
**Symptom**: The `cargo test` step fails with a message like:

```
CPU instruction cost 5,400,123 exceeded limit 5,000,000 - local estimate,
real network cost may differ significantly in either direction
```

**Fix**: Re-measure the function's cost with `cargo budget-report` and update the limit in your `budget.toml` or macro annotation. If the increase is expected (e.g., you added a feature), raise the limit consciously. If it is a regression, optimize the function.

### Test failures
**Symptom**: `cargo test --workspace` fails with test errors unrelated to budget assertions.

**Fix**: Check whether the WASM was built before running tests (`cargo build -p <contract> --release --target wasm32v1-none`). Tests that load contract WASM will fail if the WASM artifact is missing or stale. Rebuild and re-run. If the failure is in a non-budget test, it is a real test break — fix the test or the code it exercises.

## See also

- [End-User Guide](user_guide.md) — install, `budget.toml`, and the local styles of the Tier A gate.
- [Deriving Limits](deriving_limits.md) — turning a Tier B report into auditable Tier A limits.
- [Protocol Mechanics](mechanics.md) — why Tier A's local estimate can drift, how Tier B's pipeline is built.
- [Tool Reference](reference.md) — every flag and macro signature.
- [Cost Terms Glossary](glossary.md) — mapping from `cargo budget-report` rows to XDR fields.
- [Measurements](measurements.md) — the gap between local and network cost, kept up to date.
