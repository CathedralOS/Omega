#!/bin/bash
# exit 0 if witness first diagnostic matches PIN backing-relation class; 1 if rejected other; 125 otherwise
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
ROOT="${LEDGER_BISECT_REPO:?}"
cd "$ROOT"
export LEDGER_BISECT_REPO LEDGER_BISECT_TOOLS
/tmp/part32-bisect-tools/tools/verify_at_commit.sh "$(git rev-parse HEAD)" "$WITNESS" > /tmp/pin_probe_out.txt 2>&1 || true
grep -q 'normal-exit backing relation' /tmp/pin_probe_out.txt && exit 0
grep -q 'exit=1' /tmp/pin_probe_out.txt && exit 1
exit 125
