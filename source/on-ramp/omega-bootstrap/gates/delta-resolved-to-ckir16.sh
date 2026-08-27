#!/usr/bin/env sh
set -eu
GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
exec python3 -B "$GATE_DIR/delta-resolved-to-ckir16-fixture.py"
