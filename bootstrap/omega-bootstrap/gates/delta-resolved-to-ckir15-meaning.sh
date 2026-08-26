#!/usr/bin/env sh
# Persisted-Beta/Gamma meaning gate for OMGLOWG -> CKIR15 guarded views.
set -eu
GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
exec sh "$GATE_DIR/delta-resolved-to-ckir4-meaning.sh" v15
