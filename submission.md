# Drips Wave Submission: Soroban Budget Assert

## 1. Project Description
`soroban-budget-assert` solves the problem of hidden deployment failures caused by the gap between local resource estimates and real network costs. Our measurements show the gap is large and unpredictable: raw Rust test estimates ran ~81% under real testnet cost, while WASM-mode estimates swung from ~8% under to ~19% over the network's number depending on the release profile used to build the contract. Developers who trust local numbers either exhaust their budget on the public network or over-provision against costs that aren't real. This tool fixes both: a CLI (`cargo budget-report`) that compiles, deploys, and simulates every contract function in a workspace to report real non-refundable network costs, and strict test macros (`#[budget_cpu_lt(N)]`) that pin measured costs into `cargo test` so any cost regression fails CI before it reaches the network.

## 2. Repo Relationship Description
The project is contained entirely within a single workspace repository (`Tollcraft/soroban-budget-assert`). It includes the `budget-macros` (the procedural macro library for developers to use in tests), `cargo-budget-report` (the CLI executable for workspace-wide simulation), and `example-contract` (used for integration testing and as a reference implementation). 

## 3. Planned Issues Description
We have scoped genuine ongoing work organized into three primary areas:
1. **Documentation Expansion**: Scaffolded issues for building out the GitBook/mdBook site (`docs(site)`).
2. **CI/CD Automation**: Issues to automate binary compilation and release tagging for multiple OS targets (`ci(release)`).
3. **Macro Enhancements**: Issues to support dynamic budget thresholds via environment variables (`feat(macro)`). 

## 4. Required Links
- **Live Repository URL**: https://github.com/Tollcraft/soroban-budget-assert
- **Documentation Site**: https://tollcraft.github.io/soroban-budget-assert/
- **On-chain Contract Verification**: N/A (Developer Tool, not a deployed contract protocol)
- **Demo Video**: [INSERT URL ONCE RECORDED]

---

## 🎬 Instructions for Recording the Demo Video
To complete the submission, record a 1-2 minute Loom or screen recording showing:
1. Run `cargo test` on a contract function where the `#[budget_cpu_lt]` limit is set *too low*. Show the test failing with the explicit "underestimates real network cost" error.
2. Fix the limit in the code and re-run `cargo test` to show it passing.
3. Run `cargo budget-report` from the workspace root and show the aggregated testnet simulation cost table printing to the terminal.
4. Upload the video (YouTube/Loom/Drive) and paste the link in Section 4 above.
