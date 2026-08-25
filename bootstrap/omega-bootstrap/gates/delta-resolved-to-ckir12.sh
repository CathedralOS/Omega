#!/usr/bin/env sh
# Stable OMGLOWD/OMGRSW4 source-to-CKIR12 shared-byte-view producer gate.
set -eu
GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
exec python3 -B "$GATE_DIR/delta-resolved-to-ckir12-fixture.py"
