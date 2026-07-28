#!/bin/bash
set -euo pipefail
cd /Users/caner/.local/drips-agent/workspace/soroban-budget-assert
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"
LOG=.ci-output.log
{
  echo "=== verify macros $(date) ==="
  # Use installed 1.85 if 1.91 isn't ready yet
  if rustup run 1.85.0 rustc -V >/dev/null 2>&1; then
    export RUSTUP_TOOLCHAIN=1.85.0
    echo "using RUSTUP_TOOLCHAIN=1.85.0"
  fi
  rustc -V
  cargo fmt --all
  echo FMT_OK
  cargo fmt --all -- --check
  echo FMT_CHECK_OK
  cargo clippy -p budget-macros --all-targets -- -D warnings
  echo CLIPPY_OK
  cargo test -p budget-macros --lib
  echo MACROS_OK
  # quick syntax check on budget_test via rustfmt
  rustfmt --check --edition 2021 amm-pool-contract/tests/budget_test.rs && echo BUDGET_TEST_FMT_OK
  echo DONE
} >"$LOG" 2>&1
cat "$LOG"
