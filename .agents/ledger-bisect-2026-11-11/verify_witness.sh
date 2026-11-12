#!/bin/bash
# Run witness at current HEAD; prints pass/bad/skip and exit code.
set -euo pipefail
WITNESS="${1:?}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
DIR="$(cd "$(dirname "$0")" && pwd)"
export BISECT_WITNESS="$WITNESS"
cd "$ROOT"
rc=$("$DIR/bisect_witness.sh"; echo $?)
case "$rc" in
  0) echo "pass" ;;
  1) echo "bad" ;;
  *) echo "skip($rc)" ;;
esac
exit "$rc"
