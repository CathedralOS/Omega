#!/bin/bash
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
ROOT="${OMEGA_REPO:-/agent/repos/omega}"
TOOLS="${BISECT_TOOLS:-/tmp/ledger-bisect-tools}"
REV=$(git -C "$ROOT" rev-parse HEAD^{commit})
if "$TOOLS/is_ledger_only_commit.sh" "$REV"; then
  exit 125
fi
exec "$TOOLS/run_witness_at_rev.sh" "$WITNESS"
