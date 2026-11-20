#!/usr/bin/env bash
# site/scripts/validate-anchors.sh
#
# Resolves link fragments in the static site against element `id` (and `<a
# name>`) attributes in the destination document:
#
#   <a href="#quickstart">          same-page anchor
#   <a href="dashboard.html#controls">   cross-page anchor
#
# `validate.sh` only proves the file exists — its link regex stops at `#` — so
# a renamed or deleted id deployed broken in-page navigation.
#
# Usage:
#   site/scripts/validate-anchors.sh [SITE_DIR]
#
# SITE_DIR defaults to the directory above this script. Exit code is non-zero
# if any fragment has no matching target.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SITE_DIR="${1:-$(cd "$SCRIPT_DIR/.." && pwd)}"

if [ ! -d "$SITE_DIR" ]; then
  echo "validate-anchors: site directory not found: $SITE_DIR" >&2
  exit 2
fi

node -e '
const fs = require("fs");
const path = require("path");

const root = process.argv[1];
const files = fs.readdirSync(root)
  .filter((name) => /\.html?$/.test(name))
  .map((name) => path.join(root, name))
  .sort();

const idCache = new Map();
function anchorsIn(file) {
  if (!idCache.has(file)) {
    const html = fs.readFileSync(file, "utf8");
    const ids = new Set();
    let match;
    const idRe = /\bid\s*=\s*["\x27]([^"\x27]+)["\x27]/g;
    while ((match = idRe.exec(html)) !== null) ids.add(match[1]);
    const nameRe = /<a\b[^>]*\bname\s*=\s*["\x27]([^"\x27]+)["\x27]/gi;
    while ((match = nameRe.exec(html)) !== null) ids.add(match[1]);
    idCache.set(file, ids);
  }
  return idCache.get(file);
}

let errors = 0;
let checked = 0;

for (const file of files) {
  const html = fs.readFileSync(file, "utf8");
  const linkRe = /(?:href|src)\s*=\s*["\x27]([^"\x27]*)["\x27]/g;
  let match;
  while ((match = linkRe.exec(html)) !== null) {
    const ref = match[1];
    if (/^(?:[a-z][a-z0-9+.-]*:|\/\/)/i.test(ref)) continue;
    const hash = ref.indexOf("#");
    if (hash === -1) continue;
    const rawFragment = ref.slice(hash + 1);
    if (rawFragment === "") continue;

    let fragment;
    try {
      fragment = decodeURIComponent(rawFragment);
    } catch {
      fragment = rawFragment;
    }

    const target = ref.slice(0, hash).split("?")[0];
    const targetPath = target === ""
      ? file
      : path.resolve(path.dirname(file), target);

    // A missing file is validate.sh step 3 with a better message.
    if (!fs.existsSync(targetPath) || !/\.html?$/.test(targetPath)) continue;

    checked += 1;
    if (!anchorsIn(targetPath).has(fragment)) {
      const line = html.slice(0, match.index).split("\n").length;
      const source = path.relative(root, file) || path.basename(file);
      const destination = path.relative(root, targetPath) || path.basename(targetPath);
      console.error(`  ${source}:${line}: missing anchor #${rawFragment} in ${destination}`);
      errors += 1;
    }
  }
}

if (errors > 0) {
  console.error(`validate-anchors: ${errors} broken anchor(s) in ${files.length} file(s)`);
  process.exit(1);
}
console.log(`  ${checked} anchor target(s) resolved in ${files.length} file(s): OK`);
' "$SITE_DIR"
