#!/usr/bin/env bash
# Check that every `.rs` file under a crate's `src/` is reachable from the
# crate root module tree.
#
# A source file that is never declared via `mod name;` — directly or
# transitively — is never read by the compiler: clippy never lints it, tests
# never run it, and it silently rots against a crate root that moved on.
# That is exactly what happened to the eight orphaned files in
# `cargo-budget-report/src/` (see issue #451). This script walks each crate
# root, follows every `mod name;` declaration, and fails if any `.rs` file
# under `src/` is left outside the module tree.
#
# Usage: scripts/check-module-reachability.sh
#   exit 0 — every `.rs` file under every `src/` is reachable
#   exit 1 — at least one orphaned file (listed on stdout)
#
# Targets bash 3.2, the version macOS still ships as `/bin/bash`. No
# `mapfile`, no associative arrays, and no expansion of a possibly-empty
# array under `set -u`. `cargo-budget-report/tests/script_portability.rs`
# holds the scripts to that.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

# Strip `//` line comments, `/* ... */` block comments (including across
# lines) and `"..."` string literals so `mod` declarations inside them are
# not mistaken for real ones. Line-oriented; string literals are handled
# well enough for the module-level code we scan (module declarations cannot
# appear inside a function body, so a real `mod x;` is never preceded by an
# in-string `//`).
strip_comments() {
    awk '
        BEGIN { in_block = 0 }
        {
            line = $0
            out = ""
            n = length(line)
            i = 1
            while (i <= n) {
                c = substr(line, i, 1)
                nxt = substr(line, i + 1, 1)
                if (in_block) {
                    if (c == "*" && nxt == "/") { in_block = 0; i += 2; continue }
                    i += 1
                    continue
                }
                if (c == "/" && nxt == "/") break
                if (c == "/" && nxt == "*") { in_block = 1; i += 2; continue }
                if (c == "\"") {
                    out = out c
                    i += 1
                    while (i <= n) {
                        c = substr(line, i, 1)
                        out = out c
                        i += 1
                        if (c == "\\" && i <= n) { out = out substr(line, i, 1); i += 1 }
                        else if (c == "\"") break
                    }
                    continue
                }
                out = out c
                i += 1
            }
            if (!in_block) print out
        }
    '
}

# Extract the names declared by `mod name;` / `pub mod name;` /
# `pub(crate) mod name;` (with optional `#[...]` attributes on a preceding
# line) from a module file. Prints one name per line, deduplicated.
declared_modules() {
    local file="$1"
    strip_comments <"$file" \
        | grep -oE '(pub(\([^)]*\))?[[:space:]]+)?mod[[:space:]]+[A-Za-z_][A-Za-z0-9_]*[[:space:]]*;' \
        | awk '{ name = $NF; gsub(/;/, "", name); print name }' \
        | sort -u
}

# All crate roots across the workspace: `src/main.rs`, `src/lib.rs`, or each
# top-level `src/bin/*.rs` binary.
# Collected with a `read` loop rather than `mapfile`, which is a bash 4
# builtin: macOS still ships bash 3.2 as `/bin/bash`, and CONTRIBUTING.md
# expects this script to run there too.
crate_roots=()
while IFS= read -r crate_root; do
    [ -z "$crate_root" ] && continue
    crate_roots+=("$crate_root")
done < <(
    find . -type f \( \
        -name 'main.rs' -o -name 'lib.rs' -o \
        \( -name '*.rs' -path '*/src/bin/*' -not -path '*/src/bin/*/*' \) \
    \) -path '*/src/*' \
        -not -path './target/*' -not -path './node_modules/*' | sort
)

# Files that exist because they are separate crate roots rather than modules
# of another root crate.
is_crate_root_file() {
    case "$1" in
        */src/main.rs | */src/lib.rs) return 0 ;;
        */src/bin/*)
            case "$1" in
                */src/bin/*/*) return 1 ;;
                *.rs) return 0 ;;
            esac
            return 1
            ;;
        *) return 1 ;;
    esac
}

# The set of files already visited. Associative arrays are also bash 4, so
# the set is a newline-delimited string and membership is an exact-line
# match. Paths come from `find .`, so each one starts with `./` and cannot
# contain a newline that would split an entry.
reachable=$'\n'

is_reachable() {
    case "$reachable" in
        *$'\n'"$1"$'\n'*) return 0 ;;
        *) return 1 ;;
    esac
}

mark_reachable() {
    reachable="$reachable$1"$'\n'
}

# Walk the module tree from each crate root, resolving `mod name;` to
# `name.rs` or `name/mod.rs` relative to the declaring file's directory.
# `${a[@]+"${a[@]}"}` rather than `"${a[@]}"`: expanding an empty array
# trips `set -u` on bash 3.2.
queue=(${crate_roots[@]+"${crate_roots[@]}"})

# Consumed with a moving index, since slicing the array down to empty trips
# the same `set -u` behaviour.
queue_index=0
while [ "$queue_index" -lt ${#queue[@]} ]; do
    file="${queue[$queue_index]}"
    queue_index=$((queue_index + 1))

    if is_reachable "$file"; then
        continue
    fi
    mark_reachable "$file"

    dir="$(dirname "$file")"
    while IFS= read -r name; do
        [ -z "$name" ] && continue
        for candidate in "$dir/$name.rs" "$dir/$name/mod.rs"; do
            if [ -f "$candidate" ]; then
                queue+=("$candidate")
                break
            fi
        done
    done < <(declared_modules "$file")
done

# Compare against everything on disk under `src/`.
orphans=0
while IFS= read -r file; do
    [ -z "$file" ] && continue
    if is_crate_root_file "$file"; then
        continue
    fi
    if ! is_reachable "$file"; then
        echo "orphaned: $file is not reachable from any crate root (missing \`mod\` declaration)"
        orphans=1
    fi
done < <(
    find . -type f -name '*.rs' -path '*/src/*' \
        -not -path './target/*' -not -path './node_modules/*' | sort
)

if [ "$orphans" -ne 0 ]; then
    echo ""
    echo "Every .rs file under src/ must be reachable from its crate root." >&2
    exit 1
fi

echo "module reachability: all .rs files under src/ are reachable"
