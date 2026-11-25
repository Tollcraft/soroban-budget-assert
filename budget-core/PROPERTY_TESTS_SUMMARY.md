## Repository Context & Implementation Constraints

Before proposing changes, modifying code, or extending this test suite, the repository's governing documents must be reviewed:

- [README.md](https://github.com/Tollcraft/soroban-budget-assert/blob/main/README.md) — Understand the project's architecture, responsibilities of `budget-macros`, and intended behavior.
- [CONTRIBUTING.md](https://github.com/Tollcraft/soroban-budget-assert/blob/main/CONTRIBUTING.md) — Follow the repository's contribution, testing, documentation, and development requirements.
- [SECURITY.md](https://github.com/Tollcraft/soroban-budget-assert/blob/main/SECURITY.md) — Respect the project's security policy, particularly dependency auditing and responsible vulnerability disclosure.

### Scope Restrictions

The property-testing work is specifically concerned with the hand-written attribute parsers in `budget-macros/src/parser_props.rs`.

The following constraints apply:

- Keep changes limited to parser property tests, their generators, and directly necessary parser-related fixes.
- Do not introduce unrelated changes to the CLI, Soroban contracts, reporting infrastructure, or other workspace components.
- Preserve the existing parser interfaces and intended attribute syntax.
- Do not weaken existing validation or parser invariants simply to make generated cases pass.
- Invalid input must produce a `syn::Error`, never a panic.
- Any parser or diagnostic fixes discovered during property testing should be tracked under issue #394 (macro diagnostics + UI tests), rather than expanding this suite beyond its intended responsibility.
- Any dependency changes must comply with the repository's security policy and dependency auditing requirements.

The property tests are intended to improve parser robustness, not redefine the macro specification.

## Mandatory CI Verification

Passing the focused property tests is necessary but does not, by itself, establish that the task is complete.

Before marking the work as complete, all repository CI gates must be executed from the workspace root:

```bash
cargo fmt --all -- --check

cargo clippy --workspace --all-targets -- -D warnings

cargo test --workspace

bash scripts/check-module-reachability.sh

bash .github/scripts/check-cli-docs.sh

bash .github/scripts/check-compatibility-docs.sh
```

### Completion Criteria

The task can only be marked complete when:

1. All five property tests pass.
2. All three deterministic regression tests pass.
3. The complete workspace test suite passes.
4. Formatting verification passes.
5. Clippy completes without warnings.
6. Module reachability verification passes.
7. CLI documentation verification passes.
8. Compatibility documentation verification passes.
9. No unrelated changes or unintended behavioral regressions have been introduced.

**Verification reporting:** Report each CI command's actual result. Do not describe a command as passing unless it has been executed successfully. If any command fails, report the failure and its relevant output rather than marking the task complete.

If a required check cannot be executed, explicitly identify it as unverified.

