This PR addresses several documentation and code cleanup tasks assigned to this module.

- closes #636
- closes #620
- closes #616
- closes #612

## Changes
- **Issue 636**: Improved `budget-macros/tests/ui/env_file_no_env.rs` by correctly documenting the parser expectations and binding the unused `env` variable with a `_` prefix to suppress compiler warnings during UI snapshot evaluation.
- **Issue 620**: Added standardized module-level `//!` documentation to `amm-pool-contract/tests/cross_contract_test.rs` to clearly explain the objective of the integration tests concerning raw Rust measurements versus WASM environment limits.
- **Issue 616**: Enhanced inline documentation in `amm-pool-contract/tests/measure_crypto_gap.rs` with explicit recommendations on leveraging baseline outputs for realistic configuration in `#[budget_cpu_lt]`.
- **Issue 612**: Added comprehensive clarifying remarks in `amm-pool-contract/tests/measure_token_transfer_gap.rs` concerning the scalability profiling of sequential cross-contract IO calls.

The local test suites verify that none of these changes introduce regressions.
