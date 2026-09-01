#!/usr/bin/env sh
# Gamma reference ownership gate: executable meaning is independent of both the
# retired Python compiler backend and compiler-admission reconstruction.
set -eu
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
command -v python3 >/dev/null 2>&1 || {
  echo "gamma reference ownership: skipped (python3 absent)"
  exit 0
}
cd "$OMEGA_GATE_DIR"
python3 -m unittest -q test_gamma_parser.py
echo "gamma reference ownership OK"
