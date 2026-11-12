#!/bin/bash
set -euo pipefail
WITNESS=$1 GOOD=$2 BAD=$3 OUT=$4
ROOT=/agent/repos/omega
TOOLS="$ROOT/.agents/ledger-bisect-2026-11-11/tools"
INSTALL=/tmp/ledger-bisect-p6
REF=/tmp/ledger-bisect-p6-ref
cd "$ROOT"
git stash push -q -m ledger-bisect-part-6 -- .agents/ledger-bisect-2026-11-11 2>/dev/null || true
trap 'git stash list | grep -q ledger-bisect-part-6 && git stash pop -q 2>/dev/null || true' EXIT
export BISECT_PIN_REF="$REF" OMEGA_REPO_ROOT="$ROOT" CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/ledger-bisect-p6-target}"
mkdir -p "$CARGO_TARGET_DIR"
python3 "$TOOLS/prepare_pin_ref.py" "$WITNESS" >/dev/null
cp "$TOOLS/bisect_witness.sh" "$TOOLS/bisect_test.py" "$INSTALL/"
chmod +x "$INSTALL/bisect_witness.sh"
export BISECT_TEST_PY="$INSTALL/bisect_test.py"

run_witness() {
  BISECT_WITNESS="$WITNESS" "$INSTALL/bisect_witness.sh"
}

G_RC=0
B_RC=0
git checkout -q "$GOOD"
set +e
run_witness
G_RC=$?
set -e
git checkout -q "$BAD"
set +e
run_witness
B_RC=$?
set -e
STABILITY=$([[ "$G_RC" -eq 0 && "$B_RC" -eq 1 ]] && echo deterministic || echo "sanity(g=$G_RC,b=$B_RC)")

git checkout -q work/ledger-bisect-part-6
git bisect reset >/dev/null 2>&1 || true
export GIT_PAGER=cat
git bisect start "$BAD" "$GOOD" --first-parent
set +e
git bisect run bash -c 'BISECT_WITNESS="'"$WITNESS"'" BISECT_TEST_PY="'"$BISECT_TEST_PY"'" BISECT_PIN_REF="'"$REF"'" OMEGA_REPO_ROOT="'"$ROOT"'" CARGO_TARGET_DIR="'"$CARGO_TARGET_DIR"'" '"$INSTALL/bisect_witness.sh"
RC=$?
set -e
BREAK=$(git rev-parse HEAD^{commit})
git bisect reset >/dev/null 2>&1 || true
git checkout -q work/ledger-bisect-part-6

mapfile -t _ch < <(git diff --name-only "${BREAK}^1" "$BREAK" 2>/dev/null || true)
_ledger_only=0
if ((${#_ch[@]} > 0)); then
  _ledger_only=1
  for f in "${_ch[@]}"; do case $f in tests/omega/corpus_outcomes.txt|*.md) ;; *) _ledger_only=0; break ;; esac; done
fi
if ((_ledger_only)); then BREAK=""; STABILITY="ledger_only_break"; fi

PR=$([[ -n "$BREAK" ]] && git log -1 --format='%s' "$BREAK" | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)
MECH=$([[ -n "$BREAK" ]] && python3 - "$BREAK" <<'PY'
import subprocess, sys
brk = sys.argv[1]
subj = subprocess.run(["git", "log", "-1", "--format=%s", brk], capture_output=True, text=True).stdout.strip()
files = subprocess.run(["git", "show", "--name-only", "--format=", brk], capture_output=True, text=True).stdout.strip().splitlines()
code = [f for f in files if not f.endswith(".md") and "corpus_outcomes" not in f][:3]
print(subj + (" — " + ", ".join(code) if code else ""))
PY
)

python3 - "$OUT" "$WITNESS" "${BREAK:-}" "${PR:-}" "$MECH" "$RC" "$GOOD" "$G_RC" "$B_RC" "$STABILITY" <<'PY'
import json, sys
out, w, brk, pr, mech, rc, good, g, b, stab = sys.argv[1:11]
json.dump(
    {
        "witness": w,
        "breaking_commit": brk or None,
        "pr": pr or None,
        "mechanism": mech[:300],
        "status": "bisected" if brk else "propagated",
        "bisect_exit": int(rc),
        "good_commit": good,
        "bad_range_end": "51e8c45c793",
        "good_end_rc": int(g),
        "bad_pin_rc": int(b),
        "stability": stab,
        "method": "pin_frozen_narrow_v2",
    },
    open(out, "w"),
    indent=2,
)
PY
echo "witness=$WITNESS break=${BREAK:0:12} pr=$PR stability=$STABILITY"
