#!/bin/bash
set -euo pipefail
WITNESS="${1:?witness}"
OUT_JSON="${2:?output json}"
PIN="${3:-51e8c45c793007f211d0d9baee7de16767172444}"
ROOT="$(git -C "$(dirname "$0")/../../.." rev-parse --show-toplevel)"
TOOLS="$ROOT/.agents/ledger-bisect-2026-11-11/tools"
cd "$ROOT"

INSTALL=/tmp/ledger-bisect-p6
REF=/tmp/ledger-bisect-p6-ref
mkdir -p "$INSTALL" "$REF"
export BISECT_PIN="$PIN"
export BISECT_PIN_REF="$REF"
export OMEGA_REPO_ROOT="$ROOT"

python3 "$TOOLS/prepare_pin_ref.py" "$WITNESS" >/tmp/prepare_pin_ref.log

cp "$TOOLS/bisect_witness.sh" "$INSTALL/bisect_witness.sh"
cp "$TOOLS/bisect_test.py" "$INSTALL/bisect_test.py"
cp "$TOOLS/classify_corpus.py" "$INSTALL/classify_corpus.py" 2>/dev/null || true
chmod +x "$INSTALL/bisect_witness.sh"
export BISECT_TEST_PY="$INSTALL/bisect_test.py"
export BISECT_WITNESS_SCRIPT="$INSTALL/bisect_witness.sh"

git checkout -q work/ledger-bisect-part-6 2>/dev/null || true

GOOD="$(BISECT_WITNESS="$WITNESS" BISECT_WITNESS_SCRIPT="$INSTALL/bisect_witness.sh" BISECT_TEST_PY="$INSTALL/bisect_test.py" BISECT_PIN_REF="$REF" OMEGA_REPO_ROOT="$ROOT" python3 "$TOOLS/find_good.py" "$WITNESS")"

sanity() {
  git checkout -q "$1"
  set +e
  BISECT_WITNESS="$WITNESS" "$INSTALL/bisect_witness.sh"
  echo $?
  set -e
}

G_RC=$(sanity "$GOOD")
B_RC=$(sanity "$PIN")
STABILITY="deterministic"
[[ "$G_RC" -eq 0 ]] || STABILITY="wrong_good_end(rc=$G_RC)"
[[ "$B_RC" -eq 1 ]] || STABILITY="wrong_bad_end(rc=$B_RC)"

export GIT_PAGER=cat
git bisect reset >/dev/null 2>&1 || true
git bisect start "$PIN" "$GOOD" --first-parent
set +e
git bisect run "$INSTALL/bisect_witness.sh"
RC=$?
set -e
BREAK=$(git rev-parse HEAD^{commit})
git bisect reset >/dev/null 2>&1 || true
git checkout -q work/ledger-bisect-part-6

# Reject ledger-only breaking commits (should be skipped, but fail closed).
mapfile -t _changed < <(git diff --name-only "${BREAK}^1" "$BREAK" 2>/dev/null || true)
if ((${#_changed[@]} > 0)); then
  _ledger_only=1
  for _f in "${_changed[@]}"; do
    case "$_f" in tests/omega/corpus_outcomes.txt|*.md) ;; *) _ledger_only=0; break ;; esac
  done
  if ((_ledger_only)); then
    STABILITY="ledger_only_break_rejected"
    BREAK=""
  fi
fi

PR=""
MECH=""
if [[ -n "$BREAK" ]]; then
  PR=$(git log -1 --format='%s' "$BREAK" | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)
  MECH=$(python3 - "$BREAK" <<'PY'
import subprocess, sys
brk = sys.argv[1]
subj = subprocess.run(["git", "log", "-1", "--format=%s", brk], capture_output=True, text=True).stdout.strip()
files = subprocess.run(["git", "show", "--name-only", "--format=", brk], capture_output=True, text=True).stdout.strip().splitlines()
code = [f for f in files if not f.endswith(".md") and "corpus_outcomes" not in f][:3]
print(subj + (" — " + ", ".join(code) if code else ""))
PY
)
fi

python3 - "$OUT_JSON" <<PY
import json
json.dump({
  "witness": "$WITNESS",
  "breaking_commit": "$BREAK",
  "pr": "$PR" or None,
  "mechanism": """${MECH//\"/\\\"}"""[:300],
  "status": "bisected" if "$BREAK" else "propagated",
  "bisect_exit": $RC,
  "good_commit": "$GOOD",
  "good_end_rc": $G_RC,
  "bad_pin_rc": $B_RC,
  "stability": "$STABILITY",
  "method": "pin_frozen_ledger_v2",
}, open("$OUT_JSON","w"), indent=2)
PY
echo "witness=$WITNESS break=${BREAK:0:12} pr=${PR:-} good=${GOOD:0:12} stability=$STABILITY"
