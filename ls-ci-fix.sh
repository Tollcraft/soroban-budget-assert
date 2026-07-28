#!/bin/bash
set -euo pipefail
cd /Users/caner/.local/drips-agent/workspace/soroban-budget-assert
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:$PATH"
cargo fmt --all
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p budget-macros --lib
cargo test --workspace
echo CI_OK
