#!/usr/bin/env sh
# Beta refinement ownership gate: reconstruction consumes the reference parser
# but never imports or hosts a compiler backend.
set -eu
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
command -v python3 >/dev/null 2>&1 || {
  echo "beta refinement ownership: skipped (python3 absent)"
  exit 0
}
cd "$OMEGA_GATE_DIR"
python3 -m unittest -q test_refinement_ownership.py
echo "beta refinement ownership OK"
