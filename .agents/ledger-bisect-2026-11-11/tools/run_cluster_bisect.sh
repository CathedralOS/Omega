#!/bin/bash
set -euo pipefail
PIN="${PIN_COMMIT:?51e8c45c793007f211d0d9baee7de16767172444}"
WITNESS="${BISECT_WITNESS:?}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
export PIN_COMMIT="$PIN"
bash "$(dirname "$0")/setup_pin_bisect.sh" "$WITNESS"
GOOD="$(bash "$(dirname "$0")/find_good_commit.sh")"
echo "good=$GOOD witness=$WITNESS" >&2
git bisect reset "$PIN" >/dev/null 2>&1 || git checkout -q "$PIN"
git bisect start --first-parent "$PIN" "$GOOD"
export BISECT_WITNESS="$WITNESS"
git bisect run bash "$(dirname "$0")/bisect_witness.sh"
