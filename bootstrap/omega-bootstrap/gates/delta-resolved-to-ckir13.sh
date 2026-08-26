#!/usr/bin/env sh
# Stable OMGRSW5/OMGLOWE source-to-CKIR13 full-u32 subtraction producer gate.
set -eu
GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
exec python3 -B "$GATE_DIR/delta-resolved-to-ckir13-fixture.py"
