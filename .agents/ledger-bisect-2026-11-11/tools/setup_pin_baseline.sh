#!/bin/bash
set -euo pipefail
ROOT="${OMEGA_REPO:-/agent/repos/omega}"
TOOLS="${BISECT_TOOLS:-$ROOT/.agents/ledger-bisect-2026-11-11/tools}"
exec python3 "$TOOLS/setup_pin_baseline.py" "$@"
