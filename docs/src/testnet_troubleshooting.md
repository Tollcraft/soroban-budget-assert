# Testnet Troubleshooting Guide

Tier B (`cargo budget-report` without `--replay`) deploys a contract, funds an account through friendbot as part of that deploy, and simulates every exported function. Every one of those three steps can fail for reasons that have nothing to do with your contract's code. This page catalogs the failures you're likely to hit, what they mean, and what to do about each one — so a testnet blip doesn't send you searching your own contract for a bug that isn't there.

{% hint style="info" %}
Every entry below is labelled either **reproduced** — the exact wording quoted was produced by actually triggering the failure — or **from source** — the wording comes directly from the string literal in `cargo-budget-report/src/main.rs` or `live.rs`, quoted rather than paraphrased, but not independently triggered against a live testnet from this environment. Both are accurate; the label just tells you which kind of confidence to expect.
{% endhint %}

## First: is it hung, or is it retrying?

Deploy, invoke-build, and the `simulateTransaction` RPC call are each wrapped in the same retry loop (`run_with_retry` in `main.rs`). By default that's **up to 4 attempts, with the delay before each retry doubling**: 2s → 4s → 8s. Unless `--quiet` is passed, every retry prints a line to stderr before it sleeps:

```
Deploy attempt 1/4 failed. Retrying in 2 s...
Deploy attempt 2/4 failed. Retrying in 4 s...
Deploy attempt 3/4 failed. Retrying in 8 s...
```

*(from source — the exact format string in `run_with_retry`.)*

With the default settings, the worst case for a single call site is `2 + 4 + 8 = 14` seconds of sleeping before it gives up — bounded and predictable, not a hang. If your terminal has sat silent for longer than that with no new output and no exit, something other than the documented retry loop is going on (network still connecting, DNS still resolving) — that's a genuine hang, not this mechanism.

To change this behavior:

- `--max-retry-attempts N` (or `[retry].max_attempts` in `budget.toml`) — `1` disables retry entirely, useful when you want a fast, clear failure instead of a slow one.
- `--retry-backoff-secs N` (or `[retry].initial_backoff_secs`) — changes the initial delay.

See the [full retry policy reference](reference.md#retry-transient-failure-retry-policy) for the precedence between these and defaults.

### Not every failure is retried

The retry loop only re-attempts failures it classifies as **plausibly transient**. Everything else fails on the first try, on purpose — retrying a deterministic failure (a typo'd contract ID, a malformed argument) four times with growing delays would only make the eventual, unavoidable failure slower. The exact classifier (`is_transient_error` in `main.rs`) retries a message if it contains any of these substrings, case-insensitively:

`rate limit`, `rate-limited`, `ratelimit`, `429`, `too many requests`, `connection`, `timed out`, `timeout`, `reset by peer`, `broken pipe`, `503`, `502`, `unavailable`, `temporarily`, `try again`

Anything else — including a missing contract, a bad XDR, or an RPC-reported simulation error — is treated as permanent and fails immediately. This is the practical way to tell the two failure classes apart: **if you saw retry lines in the output, the tool already decided this looked transient; if it failed on attempt 1 with no retry lines, it decided the failure was deterministic** — which almost always means something in your configuration, not the network, needs fixing.

## Your configuration vs. a bad network day

The single fastest way to tell which kind of problem you have:

| You see... | It usually means |
|---|---|
| Several `attempt N/4 failed. Retrying in ... s` lines, then final failure | The network (or friendbot) is having a bad day. Retrying later, or re-running, often just works. |
| Failure on the very first attempt, no retry lines at all | Something in your setup is wrong in a way retrying cannot fix: a missing binary, an unfunded/misconfigured identity, a bad path, a bad argument. |

Use the table below to go from the specific symptom to the specific cause.

## Failure catalog

### Friendbot rate limiting

**Symptom:** Deploy fails, retries a few times with growing delays, then either succeeds or exhausts all 4 attempts with a final message like:

```
Failed to deploy amm-pool-contract after 4 attempts. Ensure your source account is funded.
Last error: Error: friendbot rate-limited (try again later)
```

*(from source — `deploy_contract_with_retry`'s error format, combined with the friendbot wording the `stellar` CLI itself returns and that this project's own test fixture at `cargo-budget-report/tests/fixtures/fake_bin/stellar` deliberately reproduces for `MOCK_STELLAR_FAIL_COUNT` tests.)*

**Cause:** Testnet's friendbot service rate-limits funding requests. Deploying a fresh (unfunded) source identity triggers a friendbot call as part of `stellar contract deploy`; under load, or when many CI jobs hit it in a short window, that call is throttled.

**What to do:** This is the classic "bad network day" case — it's why the retry loop exists. Let the retries run; if all 4 still fail, wait a minute and re-run the command. If this happens constantly in CI, fund your source identity once ahead of time (`stellar keys generate alice --network testnet --fund`, or top it up manually) so deploy doesn't need friendbot on every run, and consider raising `--max-retry-attempts` for that job.

### Friendbot / account not yet confirmed on-ledger

**Symptom:** Same shape as rate limiting — deploy fails and retries — but the underlying cause is different: friendbot's funding transaction succeeded, but hasn't yet been confirmed by the ledger the deploy step queries. `main.rs`'s own comment on the retry constants names this explicitly as one of the two reasons deploy retry exists:

> "when friendbot funding is suspected to have failed transiently (rate-limiting, network hiccups, or the account not being fully confirmed on-ledger yet)"

*(from source, quoted directly.)*

**Cause:** Ledger propagation delay between "friendbot accepted the funding request" and "the account is visible to the network for the deploy transaction."

**What to do:** Identical to rate limiting — this is exactly what the exponential backoff is for. No action needed beyond letting the retries run; a source identity that was already funded before this run won't hit this path at all.

### Friendbot / testnet unavailable

**Symptom:** Deploy fails immediately or after retries with a connection-level error rather than an explicit rate-limit message — e.g. a `curl`/CLI-level connection failure surfacing through the same retry path.

**Cause:** Testnet infrastructure (friendbot, RPC, or both) is down or unreachable, as opposed to just being slow or rate-limiting you.

**What to do:** Check [Stellar's status page](https://status.stellar.org/) before assuming your setup is broken. If testnet itself is down, no flag or configuration change here will help — wait it out. This is the one case where even a `--max-retry-attempts` increase won't save you; the 14-second default window (or whatever you've configured) is meant to ride out a blip, not an outage.

### RPC unreachable (`simulateTransaction`)

**Symptom:** Simulation fails — either after retries, or immediately depending on the specific network condition — with a message shaped like:

```
simulateTransaction RPC failed: curl exited with status <code>: <stderr>
```

*(from source — the exact wrapper text in `LiveTransport::simulate_transaction`, `live.rs`.)*

**Cause and reproduction:** This tool shells out to `curl -s -X POST ... https://soroban-testnet.stellar.org:443` directly (see the [`--network` discrepancy note](reference.md#--network-does-not-actually-route-the-simulate-step) — this URL is hardcoded and does not follow `--network`). **Reproduced** directly in this environment: `curl`'s `-s`/`--silent` flag suppresses its progress meter *and* its own error text, so a DNS or connection failure surfaces here as an exit status with an **empty** `<stderr>` — not silence from the tool hiding something, but `curl` genuinely not printing anything under `-s`:

```
$ curl -X POST -H "Content-Type: application/json" -d '{}' https://this-host-does-not-exist.invalid:443
curl: (6) Could not resolve host: this-host-does-not-exist.invalid
$ echo $?
6
```

With `-s` (as this tool invokes it), that run produces exit code `6` and empty stderr — so the message you'll actually see is closer to `curl exited with status: 6` with nothing after the colon. The exit code is still the fastest way to identify the cause; the common ones from `curl(1)`:

| Exit code | Meaning |
|---|---|
| `6` | Could not resolve host — DNS failure, no route to the RPC host |
| `7` | Failed to connect — host resolved but refused the connection, or a firewall/proxy is blocking it |
| `28` | Operation timeout — the host is reachable but not responding |
| `35` | SSL/TLS connect error |

**What to do:** This class of failure is in the retry classifier's transient list (`connection`, `timed out`, `timeout`, `unavailable` all match curl's own wording for these cases), so it retries automatically. If it still fails after all attempts: confirm outbound network access to `soroban-testnet.stellar.org:443` from wherever the tool is running (a CI runner's egress firewall is a common culprit — this is exactly the situation this project's own `budget.yml` sidesteps by mocking the Tier B step for fork pull requests, since forks don't have the network access or secrets Tier B needs), and confirm `curl` itself is on `PATH` and working (`curl --version`).

### Simulation failure (`transaction simulation failed` or similar)

**Symptom:** The run completes for other functions, but one specific function is skipped with a warning and reported as failed rather than crashing the whole run. The exact warning line depends on which of three sub-cases occurred (**from source**, the `match` on `SimulationFailure` around line 1684 of `main.rs`):

```
Warning: Simulation failed for <function>: <stellar CLI stderr>
Warning: RPC error for <function>: <RPC "error" field contents>
Warning: Failed to extract metrics for <function>: <parse error>
```

The report still prints for every function that succeeded; a fully failed run instead says `No successful simulations to report.` and exits `0` (**from source** — `main.rs`'s handling around `SimulationOutcome::Failed` and the "no successful simulations" message).

**Cause:** `simulate_function` classifies this into the same three sub-cases as the warnings above, all reported as `SimulationFailure` rather than aborting the process (**from source**, `error.rs`):

- **`Invoke`** (`Warning: Simulation failed for ...`) — `stellar contract invoke --build-only` itself failed (bad arguments in `[functions.<name>].args`, a function name that doesn't match the deployed contract's actual signature, etc.).
- **`Rpc`** (`Warning: RPC error for ...`) — the `simulateTransaction` response body parsed fine, but its JSON carried an `"error"` field — the network answered, and the answer was "no." This is the case the [`--network` discrepancy](reference.md#--network-does-not-actually-route-the-simulate-step) produces when you deploy to a non-testnet network: the simulate step queries testnet for a contract ID that only exists on the network you actually deployed to, and testnet correctly reports it as unknown.
- **`MetricsExtraction`** (`Warning: Failed to extract metrics for ...`) — the RPC responded successfully, but the expected `SorobanTransactionData` fields weren't where the tool expected them (a Protocol/SDK mismatch is the likely cause — see the [supported-versions table](reference.md#%EF%B8%8F-supported-versions--compatibility)).

**What to do:** Check which sub-case you're in from the surrounding warning text. `Invoke` failures are almost always a `budget.toml` `[functions.<name>].args` problem — cross-check against the function's real signature. `Rpc` failures where you're confident the contract and function are right are the `--network` trap above — confirm you're on `testnet`. `MetricsExtraction` is worth filing as an issue; it usually means an SDK/protocol version this tool hasn't been updated for.

### A contract that exports nothing simulatable

Two distinct, silent-by-default cases, both worth knowing apart:

**Case 1 — the package isn't a `cdylib` at all.** The tool discovers packages via `cargo metadata` and skips (with a plain Rust `continue`, no message of any kind) any package whose targets don't include a `cdylib` crate type (**from source**, the `is_cdylib` check in `main.rs`). If you expected a package to be built and simulated and it just never shows up anywhere in the output — not even a warning — check its `Cargo.toml` for:

```toml
[lib]
crate-type = ["cdylib", "rlib"]
```

A package missing `cdylib` here produces **zero output about itself**, which is easy to mistake for "it ran and had nothing to report" rather than "it was never considered."

**Case 2 — it's a `cdylib`, but the compiled WASM exports no simulatable functions.** Once a package does build as a `cdylib`, the tool parses the compiled WASM's export section and simulates every function export except names starting with `_` and the special `memory` export. If that leaves nothing, you get an explicit message this time:

```
No exported functions found in <package>
```

*(from source — the exact string in `main.rs`.)* This usually means every `pub fn` in the contract is either unintentionally private to the crate, or genuinely has no public entry points — check that your contract functions are on a `#[contractimpl]`-annotated `impl` block, which is what actually produces WASM exports for Soroban.

### Missing or misconfigured identity

**Symptom:** Deploy fails on the very first attempt — no retry lines — with a `stellar contract deploy failed: ...` message (**from source**, `LiveTransport::deploy_contract`'s error wrapper) whose actual text comes from the `stellar` CLI itself, not this tool. The tool cannot predict that text since it depends on the installed Stellar CLI version and your local identity configuration — this environment does not have the `stellar` CLI installed to reproduce it directly, so treat the CLI's own wording as authoritative over any paraphrase here.

**Cause:** The identity named by `--source` (or `source` in `budget.toml`) either doesn't exist in your local Stellar CLI's keystore, or exists but isn't funded on the target network.

**What to do:**
- Confirm the identity exists: `stellar keys ls`.
- Confirm it's funded on the network you're using: `stellar keys fund <name> --network testnet` (or generate + fund in one step: `stellar keys generate <name> --network testnet --fund`, as the [End-User Guide](user_guide.md#prerequisites) shows).
- Because this fails on attempt 1 with no retry, it will **not** self-resolve by waiting — unlike the friendbot cases above, this needs a configuration fix before re-running.

### Stellar CLI or the `wasm32` target isn't installed

Not a network failure at all, but the tool checks for both before doing any network work (`run_preflight_checks` in `main.rs`, skipped entirely under `--replay`), specifically so a missing local tool doesn't masquerade as a network problem:

```
Stellar CLI is not installed or not on PATH.
Install it with:  cargo install --locked stellar-cli
See: https://github.com/stellar/stellar-cli
```

```
wasm32-unknown-unknown target is not installed.
Install it with:  rustup target add wasm32-unknown-unknown
```

*(from source, both exact strings.)* These fail immediately with no retry, since no amount of waiting installs a binary.

## Summary: which page to reach for

- **Wrong output, right network** (a check failed, a limit needs updating): see the [main Tool Reference](reference.md), not this page.
- **Nothing works and you don't know why**: work top-to-bottom through [First: is it hung, or is it retrying?](#first-is-it-hung-or-is-it-retrying) to classify the failure, then jump to the matching entry in the [Failure catalog](#failure-catalog) above.
- **Every flag's exact behavior**: the [CLI flag reference](reference.md#full-flag-table) documents all of them, including two behaviors (`--color`, `--network`) that this page and that one both link to because they directly affect how testnet failures show up.
