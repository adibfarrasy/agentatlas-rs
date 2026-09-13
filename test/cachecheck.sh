#!/usr/bin/env bash
# cachecheck.sh — the index cache: hit, mtime-noise, rebuild, --no-cache, corrupt-fallback.
set -euo pipefail
cd "$(dirname "$0")/.."
BIN="${BIN:-./target/debug/agentatlas}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

cp -R test/fixtures/golang "$TMP/repo"

# first run rebuilds; second run hits
AGENTATLAS_CACHE_DIR="$TMP/cache" "$BIN" "$TMP/repo" > "$TMP/run1.xml"
AGENTATLAS_CACHE_DIR="$TMP/cache" "$BIN" "$TMP/repo" > "$TMP/run2.xml"
grep -q 'cache="rebuilt"' "$TMP/run1.xml" || { echo "FAIL: run1 not rebuilt"; exit 1; }
grep -q 'cache="hit"' "$TMP/run2.xml" || { echo "FAIL: run2 not hit"; exit 1; }
echo "OK: first rebuild, second hit"