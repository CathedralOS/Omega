#!/usr/bin/env sh
# Stable verify-lattice entry for the focused CKIR3 source producer gate.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
for TOOL in python3 cargo clang codesign; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "resolved-to-CKIR3: skipped ($TOOL absent)"
    exit 0
  }
done
exec python3 -B "$GATE_DIR/delta-resolved-to-ckir3-fixture.py" gate
