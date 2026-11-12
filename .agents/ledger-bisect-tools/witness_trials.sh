#!/bin/bash
# Run witness test n times; prints pass (0) / fail (1) / skip (125) counts.
set -euo pipefail
WITNESS="${1:?witness}"
N="${2:-3}"
TOOL_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(git -C "$TOOL_DIR" rev-parse --show-toplevel)"
cd "$ROOT"
export BISECT_WITNESS="$WITNESS"
pass=0 fail=0 skip=0
for _ in $(seq 1 "$N"); do
  rc=0
  bash "$TOOL_DIR/bisect_witness.sh" || rc=$?
  case $rc in
    0) pass=$((pass+1)) ;;
    1) fail=$((fail+1)) ;;
    *) skip=$((skip+1)) ;;
  esac
done
echo "witness=$WITNESS trials=$N pass=$pass fail=$fail skip=$skip"
