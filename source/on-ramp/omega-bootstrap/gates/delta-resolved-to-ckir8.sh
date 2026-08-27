#!/usr/bin/env sh
# Stable OMGLOW9 source-to-CKIR8 primitive scalar-equality producer gate.
set -eu
GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
exec python3 -B "$GATE_DIR/delta-resolved-to-ckir5-fixture.py" gate-v8
