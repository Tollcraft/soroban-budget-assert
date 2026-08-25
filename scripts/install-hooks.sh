#!/usr/bin/env bash
# Installs repository git hooks (currently: pre-commit formatting check).
#
# Run once after cloning: `bash scripts/install-hooks.sh`
# Safe to run from any directory inside the repository, and safe to run
# more than once.

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
hook_source="$repo_root/scripts/pre-commit"
hook_target_dir="$repo_root/.git/hooks"
hook_target="$hook_target_dir/pre-commit"

if [ ! -f "$hook_source" ]; then
    echo "❌ Hook source not found at: $hook_source" >&2
    exit 1
fi

mkdir -p "$hook_target_dir"

cp "$hook_source" "$hook_target"
chmod +x "$hook_target"

echo "✅ Installed pre-commit hook -> $hook_target"
echo ""
echo "The hook runs 'cargo fmt --all -- --check' before every commit."
echo "If it blocks a commit, run 'cargo fmt --all' and commit again."
