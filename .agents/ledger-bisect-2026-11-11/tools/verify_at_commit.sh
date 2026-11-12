#!/bin/bash
# Run witness at one commit (commit-scoped target); print exit code + first diagnostic line.
set -euo pipefail
COMMIT="${1:?commit}"
WITNESS="${2:?witness}"
ROOT="${LEDGER_BISECT_REPO:?set LEDGER_BISECT_REPO to omega checkout}"
cd "$ROOT"
git checkout -q "$COMMIT"
export BISECT_WITNESS="$WITNESS"
TOOLS_DIR="${LEDGER_BISECT_TOOLS:-$(dirname "$0")}"
SCRIPT="$TOOLS_DIR/bisect_witness.sh"
r=0
"$SCRIPT" || r=$?
if [[ -f /tmp/bisect_one_record.txt ]]; then
  diag=$(python3 - "$WITNESS" <<'PY'
import sys
w = sys.argv[1]
text = open("/tmp/bisect_one_record.txt").read().splitlines()
active = False
for line in text:
    if line.startswith(w + " "):
        status = line.split()[1]
        print("status=" + status, end=" ")
        active = True
        continue
    if active and line.startswith("\t"):
        print("diag=" + line[1:].strip()[:200])
        break
PY
)
else
  diag="status=? no record"
fi
echo "exit=$r $diag"
