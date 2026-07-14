#!/bin/bash

echo 'Creating: Bug: cargo budget-report ignores --network during simulation'
gh issue create --title 'Bug: cargo budget-report ignores --network during simulation' --body '**Description:**
Deploy honors `--network`, but the `simulateTransaction` RPC call is hardcoded to `https://soroban-testnet.stellar.org` (cargo-budget-report/src/main.rs:172). Anyone targeting futurenet/mainnet gets silently wrong numbers.

**Requirements and Context:**
* Update the simulation logic to read the network flag/config and use the corresponding RPC URL.
* Default to testnet if not specified, but properly route if futurenet or mainnet is provided.

**Suggested Execution:**
* Fork the repo and create a branch: `git checkout -b fix/dynamic-rpc-url`

**Implement Changes:**
* Map network names (testnet, futurenet, mainnet) to their respective public RPC URLs.
* Update the curl POST request to use the dynamic URL instead of the hardcoded testnet string.

**Example Commit Message:**
`fix: use dynamic rpc url based on network argument`

**Guidelines:**
* Complexity: Medium
* PR description must include: `Closes #[issue_id]`
' --label bug 
echo 'Creating: Bug: Budget macros break tests that return Result or use early returns'
gh issue create --title 'Bug: Budget macros break tests that return Result or use early returns' --body '**Description:**
The macro appends the assertion after the body'"'"'s statements, so a test ending in `Ok(())` fails to compile, and early returns skip the assertion entirely.

**Requirements and Context:**
* Needs the body wrapped (closure or block-expression) so the assertion always runs after a captured result, returning the result at the end.

**Suggested Execution:**
* Fork the repo and create a branch: `git checkout -b fix/macro-result-handling`

**Implement Changes:**
* Modify the generated AST in `budget-macros` to evaluate the original block into a variable (e.g., `let __result = (|| { ... })();` or similar block).
* Run the budget assertion.
* Return `__result`.

**Example Commit Message:**
`fix: support Result returns and early exits in macros`

**Guidelines:**
* Complexity: Medium
* PR description must include: `Closes #[issue_id]`
' --label bug 
echo 'Creating: Bug: Malformed budget.toml is silently ignored'
gh issue create --title 'Bug: Malformed budget.toml is silently ignored' --body '**Description:**
`toml::from_str(&s).ok()` swallows parse errors and proceeds with defaults. Users get confusing "missing --network" errors instead of the real TOML syntax error.

**Requirements and Context:**
* If `budget.toml` exists but cannot be parsed, the tool should fail loudly and print the TOML parsing error.

**Suggested Execution:**
* Fork the repo and create a branch: `git checkout -b fix/toml-parse-errors`

**Implement Changes:**
* Replace `.ok()` with explicit error handling for the `toml::from_str` call.
* Use `anyhow::Context` to provide a clear error message.

**Example Commit Message:**
`fix: bubble up toml parsing errors instead of swallowing them`

**Guidelines:**
* Complexity: Trivial
* PR description must include: `Closes #[issue_id]`
' --label bug --label 'good first issue' 
echo 'Creating: Bug: Invalid value in env = "VAR" limit silently disables the assertion'
gh issue create --title 'Bug: Invalid value in env = "VAR" limit silently disables the assertion' --body '**Description:**
A typo like `BUDGET_LIMIT=1_000_000` (with underscores) parses as `None` and currently falls back to `u64::MAX`, so the test passes vacuously. It should fail loudly.

**Requirements and Context:**
* If the environment variable exists but fails to parse as a valid `u64`, the macro should panic immediately rather than falling back.

**Suggested Execution:**
* Fork the repo and create a branch: `git checkout -b fix/strict-env-parsing`

**Implement Changes:**
* Update the macro expansion to use `.expect("Failed to parse budget env var as u64")` instead of silently consuming the parse error.

**Example Commit Message:**
`fix: panic loudly on invalid env var values in macros`

**Guidelines:**
* Complexity: Trivial
* PR description must include: `Closes #[issue_id]`
' --label bug --label 'good first issue' 
echo 'Creating: Feature: Assertion mode for cargo budget-report'
gh issue create --title 'Feature: Assertion mode for cargo budget-report' --body '**Description:**
Add CI gating on real network costs. Turns Tier B from reporting-only into a CI-blocking step.

**Requirements and Context:**
* Add `[functions.X]` `cpu_limit` / `read_limit` / `write_limit` to `budget.toml`.
* Add a `--check` flag that exits non-zero if any measured metric exceeds the limits.

**Suggested Execution:**
* Fork the repo and create a branch: `git checkout -b feat/assertion-mode`

**Implement Changes:**
* Parse new fields in `FunctionConfig`.
* Compare simulated values against parsed limits.
* Exit `1` if limits are breached.

**Example Commit Message:**
`feat: add --check flag to gate CI on budget limits`

**Guidelines:**
* Complexity: High
* PR description must include: `Closes #[issue_id]`
' --label enhancement 
echo 'Creating: Feature: Baseline/snapshot mode with % tolerance'
gh issue create --title 'Feature: Baseline/snapshot mode with % tolerance' --body '**Description:**
Record measured costs to a committed baseline file, then fail when a run regresses beyond a configurable tolerance (similar to `insta` for snapshots, but for budgets).

**Requirements and Context:**
* Introduce a `--update-baseline` flag to record current costs to `budget.baseline.json`.
* During normal runs, compare current simulation costs against the baseline.
* Fail if the regression exceeds a configurable tolerance (e.g., 5%).

**Suggested Execution:**
* Fork the repo and create a branch: `git checkout -b feat/baseline-snapshots`

**Implement Changes:**
* Implement baseline file I/O.
* Add percentage comparison logic.
* Integrate into the reporting loop.

**Example Commit Message:**
`feat: implement budget snapshots and regression tolerance`

**Guidelines:**
* Complexity: High
* PR description must include: `Closes #[issue_id]`
' --label enhancement 
echo 'Creating: Feature: Combined #[budget_lt(cpu = N, mem = M)] macro + configurable env ident'
gh issue create --title 'Feature: Combined #[budget_lt(cpu = N, mem = M)] macro + configurable env ident' --body '**Description:**
One attribute for both metrics, and an option for tests whose `Env` variable isn'"'"'t literally named `env`.

**Requirements and Context:**
* Support syntax: `#[budget_lt(cpu = 100, mem = 50, env_ident = "my_env")]`.

**Suggested Execution:**
* Fork the repo and create a branch: `git checkout -b feat/combined-macro`

**Implement Changes:**
* Create a new `budget_lt` macro.
* Parse multiple key-value attributes.
* Inject both assertions in the generated block.

**Example Commit Message:**
`feat: introduce combined budget_lt macro with custom env ident`

**Guidelines:**
* Complexity: Medium
* PR description must include: `Closes #[issue_id]`
' --label enhancement 
echo 'Creating: Feature: Replace curl subprocess with a native Rust HTTP client'
gh issue create --title 'Feature: Replace curl subprocess with a native Rust HTTP client' --body '**Description:**
Currently, `cargo-budget-report` shells out to `curl` to hit the Soroban RPC. Replacing this with a native Rust HTTP client (like `reqwest`) improves Windows portability and proper error handling.

**Requirements and Context:**
* Remove `curl` subprocess dependency.
* Use a native HTTP client.

**Suggested Execution:**
* Fork the repo and create a branch: `git checkout -b feat/native-http-client`

**Implement Changes:**
* Add `reqwest` (blocking or async) to `Cargo.toml`.
* Replace the `curl` `Command::new` block with a direct HTTP POST request.

**Example Commit Message:**
`feat: replace curl subprocess with reqwest`

**Guidelines:**
* Complexity: Medium
* PR description must include: `Closes #[issue_id]`
' --label enhancement 
echo 'Creating: Feature: --package/--function filters and contract-id reuse'
gh issue create --title 'Feature: --package/--function filters and contract-id reuse' --body '**Description:**
Today every run rebuilds, redeploys, and simulates the whole workspace. Filters plus a cached contract-id make iteration fast.

**Requirements and Context:**
* Allow users to only measure specific packages or functions.
* Optionally cache the deployed `contract_id` locally so it doesn'"'"'t deploy every single time.

**Suggested Execution:**
* Fork the repo and create a branch: `git checkout -b feat/filters-and-caching`

**Implement Changes:**
* Add `--package` and `--function` CLI arguments.
* Implement a local cache (e.g., `contract_id.txt` or similar JSON) that stores the ID per package/network combination.

**Example Commit Message:**
`feat: add target filtering and contract id caching`

**Guidelines:**
* Complexity: Medium
* PR description must include: `Closes #[issue_id]`
' --label enhancement 
echo 'Creating: Feature: Markdown output + GitHub Actions job summary integration'
gh issue create --title 'Feature: Markdown output + GitHub Actions job summary integration' --body '**Description:**
Add `--format md` and a workflow snippet that posts the budget table to `$GITHUB_STEP_SUMMARY` or a PR comment.

**Requirements and Context:**
* Support outputting the report in Markdown table format.
* Append to the environment variable path `$GITHUB_STEP_SUMMARY` if present and requested.

**Suggested Execution:**
* Fork the repo and create a branch: `git checkout -b feat/markdown-summary`

**Implement Changes:**
* Use the `tabled` crate'"'"'s markdown formatting features.
* Write to `$GITHUB_STEP_SUMMARY` file path if it'"'"'s set in the environment.

**Example Commit Message:**
`feat: support markdown format and GITHUB_STEP_SUMMARY`

**Guidelines:**
* Complexity: Medium
* PR description must include: `Closes #[issue_id]`
' --label enhancement 
echo 'Creating: Bug: cargo-budget-report exits with 0 even if function simulations fail'
gh issue create --title 'Bug: cargo-budget-report exits with 0 even if function simulations fail' --body '**Description:**
Currently, if the `simulateTransaction` RPC call returns an error (e.g., due to a contract panic, missing arguments, or invalid state), the `cargo-budget-report` tool simply logs a warning to stdout and skips to the next function. Because the `main` function still returns `Ok(())` at the end, the process exits with a `0` code. In a CI/CD environment, this means a pipeline will pass green even if every single contract simulation failed.

**Requirements and Context:**
* Track whether any simulation failed during the workspace loop.
* If failures occurred, the CLI should still print the report for the successful functions, but it must exit with a non-zero exit code (e.g., `std::process::exit(1)`) at the very end.
* This ensures CI pipelines correctly fail when contracts regress and panic during simulation.

**Suggested Execution:**
* Fork the repo and create a branch: `git checkout -b fix/report-exit-code`

**Implement Changes:**
* Update `cargo-budget-report/src/main.rs`.
* Introduce a boolean flag (e.g., `let mut has_errors = false;`) before the loop.
* Set the flag to `true` inside the error blocks.
* At the end of `main`, check the flag and return an error or exit non-zero.

**Example Commit Message:**
`fix: exit with non-zero code when contract simulations fail`

**Guidelines:**
* Complexity: Medium
* PR description must include: `Closes #[issue_id]`
' --label bug 
echo 'Creating: Feature: Configurable cargo build profile override'
gh issue create --title 'Feature: Configurable cargo build profile override' --body '**Description:**
`cargo-budget-report` hardcodes the `--release` profile when building the WASM. Users might have a custom profile like `release-opt` that strips more symbols, which drastically affects deployment and execution costs.

**Requirements and Context:**
* Add a `--profile` flag to `BudgetReportArgs`.
* Default this flag to `release` if not provided.
* Pass this profile to the `cargo build` command instead of the hardcoded `--release` flag.

**Suggested Execution:**
* Fork the repo and create a branch: `git checkout -b feat/build-profile-override`

**Implement Changes:**
* Add `profile: Option<String>` to arguments.
* Substitute `--release` in `Command::new("cargo")` with `--profile` and the actual profile string.

**Example Commit Message:**
`feat: allow overriding the cargo build profile`

**Guidelines:**
* Complexity: Trivial
* PR description must include: `Closes #[issue_id]`
' --label enhancement --label 'good first issue' 
