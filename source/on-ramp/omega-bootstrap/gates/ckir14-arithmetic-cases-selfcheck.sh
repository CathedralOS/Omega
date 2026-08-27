#!/usr/bin/env sh
# Source/expectation gate only. Future producer wiring belongs in its versioned gate.
set -eu
GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
python3 -B "$GATE_DIR/ckir14-arithmetic-cases.py" check \
    "$GATE_DIR/fixtures/ckir14-arithmetic-cases"
