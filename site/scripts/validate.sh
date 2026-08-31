#!/usr/bin/env bash
# site/scripts/validate.sh
#
# Validates site/ contents before deployment:
#   1. HTML files parse without errors
#   2. JavaScript files have valid syntax
#   3. Internal links and asset references resolve to existing files
#
# Exit code is non-zero if any check fails.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SITE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

ERRORS=0

# ── 1. HTML validation ──────────────────────────────────────────────────
echo "==> Checking HTML well-formedness..."

for f in "$SITE_DIR"/*.html; do
  [ -f "$f" ] || continue
  if node -e "
    const fs = require('fs');
    const file = '$f';
    const html = fs.readFileSync(file, 'utf8');

    if (!/<!DOCTYPE\\s+html>/i.test(html)) {
      console.error('  ' + file + ': missing <!DOCTYPE html>');
      process.exit(1);
    }

    const voidElements = new Set([
      'area','base','br','col','embed','hr','img','input',
      'link','meta','param','source','track','wbr'
    ]);
    const stack = [];
    let i = 0;
    while (i < html.length) {
      if (html[i] !== '<') { i++; continue; }
      if (html.startsWith('<!--', i)) {
        const end = html.indexOf('-->', i + 4);
        i = end === -1 ? html.length : end + 3;
        continue;
      }
      let j = i + 1;
      let inQuote = false;
      let quoteChar = '';
      while (j < html.length) {
        const ch = html[j];
        if (inQuote) {
          if (ch === quoteChar) inQuote = false;
        } else if (ch === '\"' || ch === \"'\") {
          inQuote = true;
          quoteChar = ch;
        } else if (ch === '>') {
          break;
        }
        j++;
      }
      if (j >= html.length) break;
      const raw = html.slice(i, j + 1);
      i = j + 1;

      const tagMatch = raw.match(/^<\\/?([a-zA-Z][a-zA-Z0-9]*)/);
      if (!tagMatch) continue;
      const tag = tagMatch[1].toLowerCase();
      const isClosing = raw[1] === '/';
      const isSelfClosing = raw.endsWith('/>');

      if (voidElements.has(tag)) continue;

      if (isClosing) {
        if (stack.length === 0 || stack[stack.length - 1] !== tag) {
          console.error('  ' + file + ': unexpected </' + tag + '> (expected </' + (stack[stack.length - 1] || '?') + '>)');
          process.exit(1);
        }
        stack.pop();
      } else if (!isSelfClosing) {
        stack.push(tag);
      }
    }
    if (stack.length > 0) {
      console.error('  ' + file + ': unclosed tags: ' + JSON.stringify(stack.slice(0, 10)) + (stack.length > 10 ? ' ...' : ''));
      process.exit(1);
    }
    console.log('  ' + file + ': OK');
  " 2>&1; then
    :
  else
    ERRORS=$((ERRORS + 1))
  fi
done

# ── 2. JavaScript syntax validation ────────────────────────────────────
echo ""
echo "==> Checking JavaScript syntax..."

for f in "$SITE_DIR"/assets/*.js "$SITE_DIR"/*.js; do
  if [ -f "$f" ]; then
    if node --check "$f" 2>&1; then
      echo "  $f: OK"
    else
      echo "  $f: SYNTAX ERROR"
      ERRORS=$((ERRORS + 1))
    fi
  fi
done

# ── 3. Internal link / asset reference validation ──────────────────────
echo ""
echo "==> Checking internal links and asset references..."

for f in "$SITE_DIR"/*.html; do
  [ -f "$f" ] || continue
  if node -e "
    const fs = require('fs');
    const path = require('path');
    const file = '$f';
    const dir = path.dirname(file);
    const html = fs.readFileSync(file, 'utf8');
    let ok = true;
    const attrRe = /(?:href|src)\\s*=\\s*[\\\"']([^\\\"'#]+)[\\\"']/g;
    let m;
    while ((m = attrRe.exec(html)) !== null) {
      const ref = m[1];
      if (/^https?:\\/\\//.test(ref)) continue;
      const target = path.resolve(dir, ref);
      if (!fs.existsSync(target)) {
        console.error('  ' + file + ': missing reference ' + ref);
        ok = false;
      }
    }
    if (ok) console.log('  ' + file + ': OK');
    else process.exit(1);
  " 2>&1; then
    :
  else
    ERRORS=$((ERRORS + 1))
  fi
done

for f in "$SITE_DIR"/*.css "$SITE_DIR"/assets/*.css; do
  [ -f "$f" ] || continue
  if node -e "
    const fs = require('fs');
    const path = require('path');
    const file = '$f';
    const dir = path.dirname(file);
    const css = fs.readFileSync(file, 'utf8');
    let ok = true;
    const urlRe = /url\\((?:['\\\"]?)([^'\\\"\\\\)]+)(?:['\\\"]?)\\)/g;
    let m;
    while ((m = urlRe.exec(css)) !== null) {
      const ref = m[1];
      if (/^https?:\\/\\//.test(ref)) continue;
      const target = path.resolve(dir, ref);
      if (!fs.existsSync(target)) {
        console.error('  ' + file + ': missing url() reference ' + ref);
        ok = false;
      }
    }
    if (ok) console.log('  ' + file + ': OK');
    else process.exit(1);
  " 2>&1; then
    :
  else
    ERRORS=$((ERRORS + 1))
  fi
done

# ── Summary ─────────────────────────────────────────────────────────────
echo ""
if [ "$ERRORS" -gt 0 ]; then
  echo "❌ Validation failed ($ERRORS check(s) failed)"
  exit 1
else
  echo "✅ All site/ checks passed"
fi
