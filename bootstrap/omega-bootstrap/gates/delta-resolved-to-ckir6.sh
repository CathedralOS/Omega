#!/usr/bin/env sh
# Stable OMGLOW7 source-to-CKIR6 LogicalNot producer gate.
set -eu
GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
exec python3 -B "$GATE_DIR/delta-resolved-to-ckir5-fixture.py" gate-v6
