#!/bin/bash
set -euo pipefail
WITNESS="${1:?witness}"
OUT_JSON="${2:?output json}"
PIN="${3:-51e8c45c793007f211d0d9baee7de16767172444}"
REF_741270="${4:-741270df60f7eab218e879eb6771d2bc75e64c00}"
ROOT="$(git -C "$(dirname "$0")/../../.." rev-parse --show-toplevel)"
TOOLS="$ROOT/.agents/ledger-bisect-2026-11-11/tools"
cd "$ROOT"
export BISECT_WITNESS="$WITNESS"
GOOD="$(python3 "$TOOLS/resolve_good.py" "$WITNESS")"

# Installed paths must survive `git checkout` during bisect (`.agents/` is not on old commits).
INSTALL=/tmp/ledger-bisect-p6
mkdir -p "$INSTALL"
cp "$TOOLS/bisect_witness.sh" "$INSTALL/bisect_witness.sh"
cp "$TOOLS/bisect_test.py" "$INSTALL/bisect_test.py"
chmod +x "$INSTALL/bisect_witness.sh"
SCRIPT="$INSTALL/bisect_witness.sh"
export OMEGA_REPO_ROOT="$ROOT"
export BISECT_TEST_PY="$INSTALL/bisect_test.py"

sanity() {
  local ref=$1
  git checkout -q "$ref"
  set +e
  "$SCRIPT"
  local rc=$?
  set -e
  echo "$rc"
}

git bisect reset >/dev/null 2>&1 || true
git checkout -q work/ledger-bisect-part-6

G_RC=$(sanity "$GOOD")
B_PIN=$(sanity "$PIN")
B_741=$(sanity "$REF_741270")

STABILITY="deterministic"
if [[ "$G_RC" -ne 0 ]]; then
  STABILITY="flaky_or_wrong_good(rc=$G_RC at ${GOOD:0:12})"
elif [[ "$B_PIN" -ne 1 ]]; then
  STABILITY="flaky_or_wrong_bad_pin(rc=$B_PIN)"
elif [[ "$B_741" -ne 1 ]]; then
  STABILITY="note_bad_at_741270_rc=$B_741"
fi

export GIT_PAGER=cat
git bisect start "$PIN" "$GOOD" --first-parent
set +e
git bisect run "$SCRIPT"
RC=$?
set -e
BREAK=$(git rev-parse HEAD^{commit})
git bisect reset >/dev/null 2>&1 || true
git checkout -q work/ledger-bisect-part-6

if [[ "$B_741" -eq 1 ]] && [[ "$BREAK" == "$PIN" ]]; then
  STABILITY="${STABILITY};pin_not_first_bad_already_bad_at_741270"
fi

PR=$(git log -1 --format='%s' "$BREAK" | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)
MECH=$(python3 - "$BREAK" <<'PY'
import subprocess, sys
brk = sys.argv[1]
subj = subprocess.run(["git", "log", "-1", "--format=%s", brk], capture_output=True, text=True).stdout.strip()
stat = subprocess.run(["git", "show", "--stat", "--format=", brk], capture_output=True, text=True).stdout.strip().split("\n")
top = stat[0].strip() if stat and stat[0].strip() else ""
print(subj if not top else f"{subj} — {top.split('|')[0].strip()}")
PY
)

python3 - "$OUT_JSON" <<PY
import json
json.dump({
  "witness": "$WITNESS",
  "breaking_commit": "$BREAK",
  "pr": "$PR" or None,
  "mechanism": """${MECH//\"/\\\"}"""[:300],
  "status": "bisected",
  "bisect_exit": $RC,
  "good_commit": "$GOOD",
  "good_end_rc": $G_RC,
  "bad_pin_rc": $B_PIN,
  "bad_741270_rc": $B_741,
  "stability": "$STABILITY",
}, open("$OUT_JSON","w"), indent=2)
PY
echo "witness=$WITNESS break=${BREAK:0:12} pr=${PR:-none} good=${GOOD:0:12} stability=$STABILITY"
