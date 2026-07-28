#!/usr/bin/env bash
# Installs repository git hooks (currently: pre-commit formatting check).
#
# Run once after cloning: `bash scripts/install-hooks.sh`

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
hook_source="$repo_root/scripts/pre-commit"
hook_target="$repo_root/.git/hooks/pre-commit"

cp "$hook_source" "$hook_target"
chmod +x "$hook_target"

echo "✅ Installed pre-commit hook -> $hook_target"
