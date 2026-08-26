#!/usr/bin/env sh
# Stable OMGLOWF/OMGRSW7 source-to-CKIR14 recursive full-u32 arithmetic gate.
set -eu
GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
exec python3 -B "$GATE_DIR/delta-resolved-to-ckir14-fixture.py"
