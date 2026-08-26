#!/usr/bin/env sh
# Focused OMGRFN15 candidate-frame reference gate (not persisted-Beta R1--R5).
set -eu
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
exec python3 -B "$HERE/omgrfn15-reference-gate.py"
