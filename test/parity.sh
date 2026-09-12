#!/usr/bin/env bash
# parity.sh — byte-identical vs the committed ripwire oracle outputs (test/golden/).
# Each golden file is ripwire's actual stdout, recorded from the reference build (see CREDITS.md).
set -euo pipefail

cd "$(dirname "$0")/.."
BIN="${BIN:-./target/debug/agentatlas}"
fail=0

for spec in "golang:go" "java:java"; do
    fixture="${spec%%:*}"
    refname="${spec##*:}"
    "$BIN" "test/fixtures/$fixture" > "/tmp/agentatlas-$fixture.xml"
    if ! cmp -s "/tmp/agentatlas-$fixture.xml" "test/golden/ref-$refname.xml"; then
        echo "PARITY FAIL: $fixture differs from the oracle golden"
        diff <(fold -w 100 "/tmp/agentatlas-$fixture.xml") <(fold -w 100 "test/golden/ref-$refname.xml") | head -20
        fail=1
    else
        echo "PARITY OK: $fixture byte-identical to ripwire"
    fi
done

exit "$fail"