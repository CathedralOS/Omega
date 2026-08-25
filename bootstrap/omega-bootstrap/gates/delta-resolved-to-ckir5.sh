#!/usr/bin/env sh
# Stable entry for the focused OMGRSW3/OMGLOW6 source-to-CKIR5 producer gate.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
for TOOL in python3 cargo clang codesign; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "resolved-to-CKIR5: skipped ($TOOL absent)"
    exit 0
  }
done
exec python3 -B "$GATE_DIR/delta-resolved-to-ckir5-fixture.py" gate
