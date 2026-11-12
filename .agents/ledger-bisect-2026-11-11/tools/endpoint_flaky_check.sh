#!/bin/bash
# Run witness 3x at good and bad commits; record exit codes.
set -euo pipefail
WITNESS="${1:?}"
GOOD="${2:?}"
BAD="${3:?}"
OUT="${4:?}"
ROOT="${OMEGA_REPO:-/agent/repos/omega}"
TOOLS="${BISECT_TOOLS:-/tmp/ledger-bisect-tools}"
export OMEGA_REPO="$ROOT" BISECT_TOOLS="$TOOLS"
cd "$ROOT"
{
  echo "witness=$WITNESS good=$GOOD bad=$BAD"
  for rev in "$GOOD" "$BAD"; do
    git checkout -f "$rev" >/dev/null 2>&1
    for n in 1 2 3; do
      set +e
      rc=$("$TOOLS/run_witness_at_rev.sh" "$WITNESS")
      set -e
      echo "rev=$rev run=$n rc=$rc"
    done
  done
} | tee "$OUT"
git checkout -f work/ledger-bisect-part-11 >/dev/null 2>&1 || true
