#!/usr/bin/env bash
# gaincheck.sh — the `gain` ledger verb: round-trip, determinism, disclosure, absent-ledger.
set -euo pipefail
cd "$(dirname "$0")/.."
BIN="${BIN:-./target/debug/agentatlas}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
LED="$TMP/gain.jsonl"

# absent ledger → honest message, not a fabricated zero
out="$("$BIN" gain --gain-log="$LED" 2>/dev/null || true)"
case "$out" in
    *"nothing recorded yet"*) echo "OK: absent-ledger message" ;;
    *) echo "FAIL: absent ledger"; exit 1 ;;
esac

# round-trip: two retrieval runs log two rows
RIPWIRE_GAIN_LOG="$LED" "$BIN" /tmp/golden-root/golang --grep=Dist >/dev/null 2>&1 || true
RIPWIRE_GAIN_LOG="$LED" "$BIN" /tmp/golden-root/golang --for="find the point distance" >/dev/null 2>&1 || true
rows=$(wc -l < "$LED" | tr -d " ")
[ "$rows" = "2" ] && echo "OK: 2 rows logged" || { echo "FAIL: expected 2 rows, got $rows"; exit 1; }

# spent-only rows marked model="none" (analyze/batch/quality_delta/edit_check) — simulate by logging a
# none-model row and checking the disclosure names the unmodeled count
printf '%s\n' '{"ts":1,"repo":"r","verb":"analyze","spent_tokens":5,"spent_ms":1,"naive_tokens":null,"naive_ms":null,"model":"none"}' >> "$LED"
rep="$("$BIN" gain --gain-log="$LED")"
case "$rep" in
    *"unmodeled-runs 1"*) echo "OK: unmodeled disclosed" ;;
    *) echo "FAIL: unmodeled not disclosed"; exit 1 ;;
esac
# naive rows carry a token figure
case "$rep" in
    *"naive_tokens"*) echo "OK: naive reported" ;;
    *) echo "FAIL: naive missing"; exit 1 ;;
esac

# determinism: two reads of the same ledger → identical reports
a="$("$BIN" gain --gain-log="$LED")"
b="$("$BIN" gain --gain-log="$LED")"
[ "$a" = "$b" ] && echo "OK: deterministic report" || { echo "FAIL: nondeterministic"; exit 1; }

echo "gaincheck: PASS"