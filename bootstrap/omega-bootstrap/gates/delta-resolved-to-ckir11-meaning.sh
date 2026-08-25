#!/usr/bin/env sh
# Persisted-Beta/Gamma meaning gate for OMGLOWC -> CKIR11 Trapping Add.
set -eu
GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
exec sh "$GATE_DIR/delta-resolved-to-ckir4-meaning.sh" v11
