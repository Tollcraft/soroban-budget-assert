#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
#
# regenerate-measurements.sh
# --------------------------
# Discover and run all measurement harnesses that produce local estimates.
# Harnesses are identified by a special marker comment "// @measure <mode>"
# where <mode> is "local" (run automatically) or "testnet" (requires a funded
# testnet identity and the `stellar` CLI). The script runs the "local" harnesses
# and prints a summary of skipped "testnet" harnesses.
#
# Usage:
#   ./scripts/regenerate-measurements.sh [--out DIR]
#
#   --out DIR   Directory to write captured output (default: ./measurements-out)
#
# The script is intentionally simple and does not attempt to parse the output –
# it merely captures the raw `cargo test` output for each harness. Users can
# diff the generated files against the existing MEASUREMENTS.md.

set -euo pipefail

OUT_DIR="measurements-out"

# ── Parse arguments ────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --out)
            if [[ -z "${2:-}" ]]; then
                echo "Error: --out requires a directory argument" >&2
                exit 1
            fi
            OUT_DIR="$2"
            shift 2
            ;;
        *)
            echo "Error: unknown argument '$1'" >&2
            echo "Usage: $0 [--out DIR]" >&2
            exit 1
            ;;
    esac
done

mkdir -p "$OUT_DIR"

# Helper to run a cargo test harness and capture its output.
run_harness() {
    local crate="$1"
    local test_target="$2"
    local mode="$3"
    local feature="${4:-}"
    local test_name="${5:-}"
    local out_file="$OUT_DIR/${crate}__${test_target}.log"

    echo "Running $crate $test_target ($mode)..."
    if [[ "$mode" == "local" ]]; then
        # Build the WASM for the contract (required by many harnesses).
        # We build once per crate; ignore errors if already built.
        local build_args=(--target wasm32v1-none --release -p "$crate")
        if [[ -n "$feature" ]]; then
            build_args+=(--features "$feature")
        fi
        cargo build "${build_args[@]}" >/dev/null 2>&1 || true

        # Run the harness with --nocapture to see its eprintln output.
        local test_args=(-p "$crate" --test "$test_target")
        if [[ -n "$feature" ]]; then
            test_args+=(--features "$feature")
        fi
        test_args+=(-- --nocapture)
        if [[ -n "$test_name" ]]; then
            test_args+=("$test_name")
        fi
        cargo test "${test_args[@]}" 2>&1 | tee "$out_file"
    else
        echo "SKIPPED (testnet required) – $crate $test_target" | tee "$out_file"
    fi
}

# ── Discover harnesses ─────────────────────────────────────────────────
# Look for files under */tests/*.rs containing the marker.
# The marker format: // @measure <mode>[:<feature>[:<test_name>]]
#   <mode>     = "local" or "testnet"
#   <feature>  = optional Cargo feature to enable (e.g. "sdk20", "sdk22")
#   <test_name>= optional specific test function to run

mapfile -t test_files < <(git ls-files "*/tests/*.rs")

for file in "${test_files[@]}"; do
    # Extract crate name from the path (first component before '/').
    crate="$(echo "$file" | cut -d'/' -f1)"
    # Determine the test target name (file stem without .rs).
    test_target="$(basename "$file" .rs)"
    # Read the marker line.
    marker="$(grep -m1 '// @measure' "$file" || true)"
    if [[ -z "$marker" ]]; then
        # No marker – skip this file (new harnesses must add a marker).
        continue
    fi
    # Parse mode, optional feature, and optional test name.
    # Expected: "// @measure local" or "// @measure local:sdk22" or
    #           "// @measure local:sdk22:my_test"
    mode_part="$(echo "$marker" | awk -F'@measure' '{print $2}' | xargs)"
    mode="$(echo "$mode_part" | cut -d':' -f1 | xargs)"
    feature="$(echo "$mode_part" | cut -d':' -f2 | xargs)"
    test_name="$(echo "$mode_part" | cut -d':' -f3 | xargs)"
    if [[ -z "$mode" ]]; then
        mode="local"
    fi
    run_harness "$crate" "$test_target" "$mode" "$feature" "$test_name"
done

# Summary
echo ""
echo "=== Regeneration complete ==="
echo "Outputs written to $OUT_DIR/"
