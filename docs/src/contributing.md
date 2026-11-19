# Contributing Guide

We welcome contributions to `soroban-budget-assert`. 

### Workflow
1. Fork the repository.
2. Create a feature branch.
3. Commit your changes using conventional commits (e.g., `feat(macro): add new limit`).
4. Push to your fork and submit a Pull Request.

### Requirements
- All new CLI features must support the `--json` flag.
- Macro changes must cover the compile-fail and pass fixtures exercised by
  [`budget-macros/tests/ui.rs`](../../budget-macros/tests/ui.rs), as well as
  relevant runtime tests in `amm-pool-contract`. The UI fixtures live in
  `budget-macros/tests/ui/`; update their expected compiler diagnostics with
  `TRYBUILD=overwrite cargo test -p budget-macros`.
- Do not introduce panics in the CLI; use `anyhow::Result` for graceful error handling.
- When adding a new cost metric, follow the [Adding a New Cost Metric](adding_a_metric.md) guide to ensure all coordinated edits are made.

## ⚙️ Supported Versions & Compatibility

See the [Supported Versions & Compatibility reference](reference.md#-supported-versions--compatibility)
for the canonical SDK/XDR pins and compatibility matrix.
