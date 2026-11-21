#!/usr/bin/env bash
# Check that README.md documents the SDK/XDR versions pinned by the manifests.
# This complements check-cli-docs.sh: that script checks CLI flag coverage,
# while this direction checks compatibility documentation against Cargo.toml.
#
# Issue #978 adds a second direction to the same section: the README has to keep
# the *pinned* protocol (derived from the manifests) and the *observed live*
# protocol (read off a public network on a date) as two separate records, so a
# reader cannot take "mainnet runs 29" as a claim that this workspace supports 29.
# The check stays offline — it inspects the document, never the ledger. A CI job
# that called an RPC endpoint would be a flaky gate.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

readme="README.md"
if [ ! -f "$readme" ]; then
    echo "check-compatibility-docs: expected file not found: $readme" >&2
    exit 1
fi

manifest_versions=()
while IFS= read -r version; do
    manifest_versions+=("$version")
done < <(
    grep -R -hE --include='Cargo.toml' \
        '^[[:space:]]*(soroban-sdk|stellar-xdr)[[:space:]]*=' . \
        | grep -oE '"[0-9]+\.[0-9]+\.[0-9]+"' \
        | tr -d '"' \
        | sort -u
)

missing=0
for version in "${manifest_versions[@]}"; do
    if ! grep -qF -- "$version" "$readme"; then
        echo "missing: manifest version $version is not documented in $readme"
        missing=1
    fi
done

compatibility="$(sed -n '/^## ⚙️ Supported Versions & Compatibility/,/^### What "Supported" means here/p' "$readme")"
while IFS= read -r version; do
    if [ -n "$version" ] && ! grep -R -qF --include='Cargo.toml' -- "$version" .; then
        echo "missing: documented version $version is absent from Cargo.toml"
        missing=1
    fi
done < <(printf '%s\n' "$compatibility" | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | sort -u)

# The two records the compatibility section must keep apart. Each is a single
# top-level bullet, anchored by its label, so prose that rewords or reorders the
# section fails here instead of quietly dropping one of the two claims.
pinned_record="$(printf '%s\n' "$compatibility" \
    | grep -E '^- \*\*Pinned protocol\*\*:' | head -n1 || true)"
observed_record="$(printf '%s\n' "$compatibility" \
    | grep -E '^- \*\*Observed live protocol\*\*:' | head -n1 || true)"

xdr_major="$(
    grep -R -hE --include='Cargo.toml' '^[[:space:]]*stellar-xdr[[:space:]]*=' . \
        | grep -oE '"[0-9]+' | head -n1 | tr -d '"'
)"
if [ -z "$xdr_major" ]; then
    echo "missing: no stellar-xdr major version is pinned in any Cargo.toml"
    missing=1
fi

if [ -z "$pinned_record" ]; then
    echo "missing: README has no \"- **Pinned protocol**:\" record in the compatibility section"
    missing=1
elif ! printf '%s\n' "$pinned_record" | grep -qE 'Protocol [0-9]+'; then
    echo "missing: the \"Pinned protocol\" record states no \`Protocol <n>\`"
    missing=1
else
    protocol="$(printf '%s\n' "$pinned_record" | grep -oE 'Protocol [0-9]+' \
        | head -n1 | awk '{print $2}')"
    if [ -n "$xdr_major" ] && [ "$protocol" -ne "$xdr_major" ]; then
        echo "mismatch: README protocol $protocol does not match stellar-xdr major $xdr_major"
        missing=1
    fi
fi

if [ -z "$observed_record" ]; then
    echo "missing: README has no \"- **Observed live protocol**:\" record in the compatibility section"
    missing=1
else
    for network in mainnet testnet; do
        if ! printf '%s\n' "$observed_record" | grep -qE "$network[[:space:]]+[0-9]+"; then
            echo "missing: the observed record gives no protocol number for $network"
            missing=1
        fi
    done
    if ! printf '%s\n' "$observed_record" | grep -qE '[0-9]{4}-[0-9]{2}-[0-9]{2}'; then
        echo "missing: the observed record is undated (an ISO YYYY-MM-DD observation date is required)"
        missing=1
    fi
fi

# Merging the two claims back into one is the failure mode this gate exists for:
# either the pinned record starts naming networks and dates, or the observation
# starts carrying the pin. Both are checked on the anchored lines only.
if printf '%s\n' "$pinned_record" | grep -qiE 'mainnet|testnet|observed'; then
    echo "merged: the \"Pinned protocol\" record names a network or an observation date"
    missing=1
fi
if printf '%s\n' "$observed_record" | grep -qE 'Protocol [0-9]+'; then
    echo "merged: the \"Observed live protocol\" record restates a \`Protocol <n>\` pin"
    missing=1
fi

# The observation is only re-checkable if the command that produced it is in the
# document, so the gate looks for the invocation itself rather than the word.
for fragment in '"method":"getNetwork"' 'horizon.stellar.org'; do
    if ! printf '%s\n' "$compatibility" | grep -qF -- "$fragment"; then
        echo "missing: the compatibility section no longer records the command that re-derives the observation ($fragment)"
        missing=1
    fi
done

if [ "$missing" -ne 0 ]; then
    exit 1
fi

echo "compatibility docs: README pins match Cargo.toml manifests, and the pinned and observed protocols are recorded separately"
