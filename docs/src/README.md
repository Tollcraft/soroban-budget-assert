# Introduction

`soroban-budget-assert` solves a specific problem in the Stellar ecosystem: the gap between local resource estimates and real network costs.

During Milestone 1 research, we found that local WASM-mode estimates underestimate real testnet costs by ~8%, and raw Rust local estimates underestimate by ~83% (measured at 143,887 CPU instructions locally vs 832,006 on testnet). 

If developers deploy contracts based solely on local estimates, transactions will fail on the public network when budgets are exhausted. This tool measures and asserts that gap during CI, preventing failed deployments.
