#!/usr/bin/env bash
# scripts/check-docs-anchors.sh
#
# Validates that every Markdown anchor link in the mdBook sources resolves to
# a heading in the destination document:
#
#   [text](reference.md#some-heading)   cross-page anchor
#   [text](#some-heading)               same-page anchor
#
# Heading slugs are generated the way GitHub and mdBook do it: lowercase the
# heading text, drop inline Markdown/HTML decoration and punctuation, then
# replace spaces with hyphens. Duplicate headings gain `-1`, `-2`, … suffixes.
#
# Usage:
#   scripts/check-docs-anchors.sh [DOCS_DIR]
#
# DOCS_DIR defaults to `docs/src` next to this script. Exit code is non-zero
# if any anchor target is missing. Bash 3.2 compatible (macOS /bin/bash).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DOCS_DIR="${1:-$SCRIPT_DIR/../docs/src}"

if [ ! -d "$DOCS_DIR" ]; then
  echo "check-docs-anchors: docs directory not found: $DOCS_DIR" >&2
  exit 2
fi

# Heading slugs of one file, one per line, with duplicate suffixes applied.
#
# The slug rule is GitHub's / mdBook 0.5's: lowercase, drop inline code and
# emphasis markers and every character that is not alphanumeric, `_`, `-` or a
# space, then hyphenate spaces. `-1`, `-2`, … disambiguate repeated headings.
# mdBook 0.4 additionally stripped anything that looked like an HTML tag
# (`` `functions.<name>` `` became `functions-per-…`), so that spelling is
# emitted as a second accepted form.
heading_slugs() {
  LC_ALL=C awk '
    function strip_tags(s) { gsub(/<[^>]*>/, "", s); return s }
    function slug(s) {
      gsub(/[`*~]/, "", s)
      s = tolower(s)
      gsub(/[^a-z0-9 _-]/, "", s)
      gsub(/ /, "-", s)
      return s
    }
    /^#+[ \t]/ {
      line = $0
      sub(/^#+[ \t]*/, "", line)
      sub(/[ \t]*#*$/, "", line)
      github = slug(line)
      if (github == "") next
      mdbook = slug(strip_tags(line))
      n = count[github]++
      if (n > 0) {
        print github "-" n
        print (mdbook == github ? github : mdbook) "-" n
      } else {
        print github
        print mdbook
      }
    }
  ' "$1"
}

slug_known() {
  grep -qxF -- "$1" "$SLUG_SCRATCH"
}

WORK="$(mktemp -d "${TMPDIR:-/tmp}/docs-anchors.XXXXXX")"
CACHE_DIR="$WORK/cache"
mkdir -p "$CACHE_DIR"
SLUG_SCRATCH="$WORK/slugs"

# Slugs per destination file, computed once. Without this every link re-walks
# the target document and the check becomes quadratic in the book's size.
slugs_for() {
  local target="$1" cache_file
  cache_file="$CACHE_DIR/$(printf '%s' "$target" | tr '/ ' '__')"
  if [ ! -f "$cache_file" ]; then
    heading_slugs "$target" > "$cache_file"
  fi
  SLUG_SCRATCH="$cache_file"
}
trap 'rm -rf "$WORK"' EXIT

ERRORS=0
CHECKED=0
FILES=0

for file in "$DOCS_DIR"/*.md; do
  [ -f "$file" ] || continue
  FILES=$((FILES + 1))
  dir="$(dirname "$file")"

  # One scan per file: `line:ref` pairs for every fragment-bearing link.
  refs="$(grep -noE '\]\([^()]*#[^()]*\)' "$file" || true)"
  [ -n "$refs" ] || continue

  while IFS= read -r entry; do
    [ -n "$entry" ] || continue
    lineno="${entry%%:*}"
    ref="${entry#*:}"
    ref="${ref#](}"
    ref="${ref%)}"
    case "$ref" in
      *://*|mailto:*|javascript:*) continue ;;
    esac

    fragment="${ref#*#}"
    target="${ref%%#*}"
    target="${target%%\?*}"
    [ -n "$fragment" ] || continue

    if [ -z "$target" ]; then
      target_path="$file"
    else
      target_path="$dir/$target"
    fi

    # Only Markdown destinations carry heading anchors. A missing file is
    # check-compatibility-docs / link-check territory, not ours.
    case "$target_path" in
      *.md) ;;
      *) continue ;;
    esac
    [ -f "$target_path" ] || continue

    CHECKED=$((CHECKED + 1))
    slugs_for "$target_path"

    if ! slug_known "$fragment"; then
      echo "  ${file#"$SCRIPT_DIR/../"}:$lineno: missing anchor '#$fragment' in ${target:-$(basename "$file")}" >&2
      ERRORS=$((ERRORS + 1))
    fi
  done <<EOF
$refs
EOF
done

if [ "$ERRORS" -gt 0 ]; then
  echo "check-docs-anchors: $ERRORS broken anchor(s) across $FILES file(s)" >&2
  exit 1
fi

echo "check-docs-anchors: $CHECKED anchor(s) resolved across $FILES file(s)"
