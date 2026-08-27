#!/usr/bin/env sh
# Rust-free persisted-Beta/Gamma observation of OMGLOWA -> CKIR9.
set -eu
GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
exec sh "$GATE_DIR/delta-resolved-to-ckir4-meaning.sh" v9
