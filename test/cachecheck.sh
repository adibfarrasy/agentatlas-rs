#!/usr/bin/env bash
# cachecheck.sh — the index cache: hit, mtime-noise, rebuild, --no-cache, corrupt-fallback, delete.
set -euo pipefail
cd "$(dirname "$0")/.."
BIN="${BIN:-./target/debug/agentatlas}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

cp -R test/fixtures/golang "$TMP/repo"

# 1. first run rebuilds; second and third run hit, byte-identical to each other
AGENTATLAS_CACHE_DIR="$TMP/cache" "$BIN" "$TMP/repo" > "$TMP/run1.xml"
AGENTATLAS_CACHE_DIR="$TMP/cache" "$BIN" "$TMP/repo" > "$TMP/run2.xml"
AGENTATLAS_CACHE_DIR="$TMP/cache" "$BIN" "$TMP/repo" > "$TMP/run3.xml"
grep -q 'cache="rebuilt"' "$TMP/run1.xml" || { echo "FAIL: run1 not rebuilt"; exit 1; }
grep -q 'cache="hit"' "$TMP/run2.xml" || { echo "FAIL: run2 not hit"; exit 1; }
cmp -s "$TMP/run2.xml" "$TMP/run3.xml" || { echo "FAIL: hits not byte-identical"; exit 1; }
echo "OK: first rebuild, second+third hit, hits identical"

# 2. mtime noise (touch, no content change) stays a hit
sleep 1
touch "$TMP/repo/main.go"
AGENTATLAS_CACHE_DIR="$TMP/cache" "$BIN" "$TMP/repo" > "$TMP/touch.xml"
grep -q 'cache="hit"' "$TMP/touch.xml" || { echo "FAIL: touch caused rebuild"; exit 1; }
echo "OK: touch = hit"

# 3. content change rebuilds
printf 'package main\nfunc Changed() {}\n' >> "$TMP/repo/main.go"
AGENTATLAS_CACHE_DIR="$TMP/cache" "$BIN" "$TMP/repo" > "$TMP/edit.xml"
grep -q 'cache="rebuilt"' "$TMP/edit.xml" || { echo "FAIL: edit not rebuilt"; exit 1; }
echo "OK: content change = rebuild"

# 4. --no-cache always rebuilds
AGENTATLAS_CACHE_DIR="$TMP/cache" "$BIN" "$TMP/repo" --no-cache > "$TMP/nocache.xml"
grep -q 'cache="rebuilt"' "$TMP/nocache.xml" || { echo "FAIL: --no-cache not rebuilt"; exit 1; }
echo "OK: --no-cache = rebuild"

# 5. corrupt cache falls back to a correct rebuild
CF="$(find "$TMP/cache" -name '*.dat' | head -1)"
head -c 20 "$CF" > "$CF.trunc"
mv "$CF.trunc" "$CF"
AGENTATLAS_CACHE_DIR="$TMP/cache" "$BIN" "$TMP/repo" > "$TMP/corrupt.xml"
grep -q 'cache="rebuilt"' "$TMP/corrupt.xml" || { echo "FAIL: corrupt cache not rebuilt"; exit 1; }
cmp -s "$TMP/edit.xml" "$TMP/corrupt.xml" || { echo "FAIL: corrupt-fallback output differs"; exit 1; }
echo "OK: corrupt cache = rebuild, output matches"

# 6. deleting a file rebuilds
rm "$TMP/repo/main.go"
AGENTATLAS_CACHE_DIR="$TMP/cache" "$BIN" "$TMP/repo" > "$TMP/del.xml"
grep -q 'cache="rebuilt"' "$TMP/del.xml" || { echo "FAIL: delete not rebuilt"; exit 1; }
echo "OK: delete = rebuild"