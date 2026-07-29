<#
.SYNOPSIS
    Installs the repository's pre-commit formatting hook.

.DESCRIPTION
    PowerShell equivalent of `scripts/install-hooks.sh` for Windows
    contributors who do not have Git Bash or WSL available.

    Copies `scripts/pre-commit` to `.git/hooks/pre-commit` and ensures
    the hook is executable.

    Run once after cloning:
        pwsh scripts/install-hooks.ps1

    The hook runs `cargo fmt --all -- --check` before every commit and
    blocks the commit if formatting is off. Fix with `cargo fmt --all`
    and commit again.

.NOTES
    - Requires PowerShell 5.1+ (or PowerShell Core 6+).
    - The pre-commit hook script itself still uses `#!/usr/bin/env bash`;
      on Windows it is invoked via Git's bundled bash, which is always
      available in a `git commit` context.
    - clippy and tests are intentionally left to CI and the manual
      pre-PR checklist since they take longer to run.
#>

$ErrorActionPreference = "Stop"

$repoRoot = (git rev-parse --show-toplevel)
if (-not $repoRoot) {
    Write-Error "Failed to determine repository root. Are you inside a git repository?"
    exit 1
}

$hookSource = Join-Path $repoRoot "scripts" "pre-commit"
$hookTarget = Join-Path $repoRoot ".git" "hooks" "pre-commit"

if (-not (Test-Path $hookSource)) {
    Write-Error "Hook source not found at: $hookSource"
    exit 1
}

Copy-Item -Path $hookSource -Destination $hookTarget -Force

# Git on Windows (Git for Windows) uses its bundled bash to execute
# hooks, which honours the #! line. The hook just needs to be readable;
# chmod +x is not needed on native Windows. If you're running this
# inside WSL or Cygwin, use `bash scripts/install-hooks.sh` instead.

Write-Host "✅ Installed pre-commit hook -> $hookTarget"
Write-Host ""
Write-Host "The hook runs 'cargo fmt --all -- --check' before every commit."
Write-Host "If it blocks a commit, run 'cargo fmt --all' and commit again."
