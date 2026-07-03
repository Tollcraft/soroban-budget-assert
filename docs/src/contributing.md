# Contributing Guide

We welcome contributions to `soroban-budget-assert`. 

### Workflow
1. Fork the repository.
2. Create a feature branch.
3. Commit your changes using conventional commits (e.g., `feat(macro): add new limit`).
4. Push to your fork and submit a Pull Request.

### Requirements
- All new CLI features must support the `--json` flag.
- Macro changes must include corresponding `#[test]` cases in the `example-contract`.
- Do not introduce panics in the CLI; use `anyhow::Result` for graceful error handling.
