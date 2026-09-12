#!/usr/bin/env bash
# parity.sh — byte-identical vs the committed ripwire oracle outputs (test/golden/).
# Goldens are ripwire's actual stdout from a git-free fixture copy at /tmp/golden-root
# (the git at= stamp would otherwise move between runs). See CREDITS.md.
set -euo pipefail
cd "$(dirname "$0")/.."
BIN="${BIN:-./target/debug/agentatlas}"

# git-free fixture root, same layout the goldens were captured from
rm -rf /tmp/golden-root
mkdir -p /tmp/golden-root
cp -R test/fixtures/golang test/fixtures/java /tmp/golden-root/

fail=0
check() {
    local desc="$1" fixture="$2"; shift 2
    local golden="test/golden/$desc"
    [[ -f "$golden" ]] || golden="test/golden/verbs/$desc"
    "$BIN" "/tmp/golden-root/$fixture" "$@" > /tmp/parity-out.xml
    if cmp -s /tmp/parity-out.xml "$golden"; then
        echo "PARITY OK: $desc"
    else
        echo "PARITY FAIL: $desc"
        diff <(fold -w 100 /tmp/parity-out.xml) <(fold -w 100 "$golden") | head -15
        fail=1
    fi
}

check map-golang golang
check map-java java
check grep-dist golang --grep=Dist
check grep-point golang --grep=Point
check callers-dist golang --callers=Dist
check callees-main golang --callees=main
check uses-point golang --uses=Point
check for-find golang --for=find the point distance

exit "$fail"
