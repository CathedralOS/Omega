#!/usr/bin/env sh
# Persisted-Beta/Gamma meaning gate for OMGLOWB -> CKIR10 IntegerWiden.
set -eu
GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
exec "$GATE_DIR/delta-resolved-to-ckir4-meaning.sh" v10
