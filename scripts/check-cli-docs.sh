#!/usr/bin/env bash
# Check that every `#[arg(...)]`-declared flag of `cargo budget-report`
# (cargo-budget-report/src/cli.rs) is documented in the CLI flag reference
# (docs/src/reference.md).
#
# The reference page used to cover only a handful of the ~20+ flags defined
# in `cli.rs`, and there was nothing to stop that gap from growing again as
# new flags were added (see issue #434). This script is the drift check: it
# derives each flag's `--kebab-case` name from its `cli.rs` field name and
# fails if that literal string is missing from reference.md.
#
# This only catches *drift* (a flag with no mention at all) — it says
# nothing about whether the prose documenting an existing flag is accurate
# or complete. It cannot replace review of the actual English, only make an
# omission impossible to merge unnoticed.
#
# Usage: scripts/check-cli-docs.sh
#   exit 0 — every flag in cli.rs is mentioned in reference.md
#   exit 1 — at least one flag is missing (listed on stdout)

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

cli_file="cargo-budget-report/src/cli.rs"
ref_file="docs/src/reference.md"

for f in "$cli_file" "$ref_file"; do
    if [ ! -f "$f" ]; then
        echo "check-cli-docs: expected file not found: $f" >&2
        exit 1
    fi
done

# Extract the field name declared on the first non-blank, non-comment line
# after each `#[arg(...)]` attribute. `cli.rs` writes each attribute on its
# own single line immediately above the field it decorates (`#[arg(long)]`
# / `#[arg(long, value_name = "PATH")]` / etc.), so this is a plain
# line-oriented scan rather than a real Rust parser. Written as a plain bash
# loop (not awk) so it runs the same under gawk-less environments (macOS
# ships the one-true-awk, Ubuntu's `awk` is mawk — neither supports the
# 3-arg `match()` capture-group extension gawk provides).
fields=()
want=0
while IFS= read -r raw_line; do
    if [[ "$raw_line" == *'#[arg('* ]]; then
        want=1
        continue
    fi
    if [ "$want" -eq 1 ]; then
        line="${raw_line#"${raw_line%%[![:space:]]*}"}" # trim leading whitespace
        if [ -z "$line" ] || [[ "$line" == //* ]]; then
            continue
        fi
        if [[ "$line" =~ ^pub[[:space:]]+([A-Za-z_][A-Za-z0-9_]*)[[:space:]]*: ]]; then
            fields+=("${BASH_REMATCH[1]}")
        fi
        want=0
    fi
done < "$cli_file"

if [ "${#fields[@]}" -eq 0 ]; then
    echo "check-cli-docs: found zero #[arg(...)] fields in $cli_file — parser likely broken" >&2
    exit 1
fi

missing=0
for field in "${fields[@]}"; do
    flag="--${field//_/-}"
    if ! grep -qF -- "$flag" "$ref_file"; then
        echo "missing: $flag (cli.rs field \`$field\`) is not mentioned in $ref_file"
        missing=1
    fi
done

if [ "$missing" -ne 0 ]; then
    echo ""
    echo "Every #[arg(...)] flag in $cli_file must appear in $ref_file." >&2
    exit 1
fi

echo "cli docs: all ${#fields[@]} flags in $cli_file are mentioned in $ref_file"
