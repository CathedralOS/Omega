#!/bin/bash
set -euo pipefail
WITNESS="${1:?witness}"
OUT="${2:?json out}"
ROOT="${OMEGA_REPO:-/agent/repos/omega}"
TOOLS="${BISECT_TOOLS:-/tmp/ledger-bisect-tools}"
PIN="${LEDGER_BISECT_PIN:-51e8c45c793007f211d0d9baee7de16767172444}"
PIN_FULL=$(git -C "$ROOT" rev-parse "$PIN^{commit}")

if [[ -n "${BISECT_GOOD:-}" ]]; then
  GOOD="$BISECT_GOOD"
else
  GOOD=$(python3 "$TOOLS/find_good_binary.py" "$WITNESS" "$PIN_FULL")
fi

export OMEGA_REPO="$ROOT" BISECT_TOOLS="$TOOLS" BISECT_WITNESS="$WITNESS" GIT_PAGER=cat

verify_at() {
  local rev=$1
  git -C "$ROOT" checkout -f "$rev" >/dev/null 2>&1
  set +e
  local rc
  rc=$("$TOOLS/run_witness_at_rev.sh" "$WITNESS")
  set -e
  echo "$rc"
}

g_rc=$(verify_at "$GOOD")
b_rc=$(verify_at "$PIN_FULL")
git -C "$ROOT" checkout -f work/ledger-bisect-part-11 >/dev/null 2>&1
if [[ "$g_rc" -ne 0 || "$b_rc" -ne 1 ]]; then
  echo "endpoints failed g_rc=$g_rc b_rc=$b_rc good=$GOOD" >&2
  python3 - "$OUT" "$WITNESS" "$GOOD" "$g_rc" "$b_rc" <<'PY'
import json, sys
json.dump({
    "witness": sys.argv[2], "good": sys.argv[3],
    "endpoint_good_rc": int(sys.argv[4]), "endpoint_bad_rc": int(sys.argv[5]),
    "status": "propagated", "reason": "endpoints not good=0 pin=1",
}, open(sys.argv[1], "w"), indent=2)
PY
  exit 2
fi

cd "$ROOT"
git bisect reset 2>/dev/null || true
git bisect start --first-parent --bad "$PIN_FULL" --good "$GOOD"
set +e
git bisect run "$TOOLS/bisect_witness.sh"
RC=$?
set -e
BREAK=$(git -C "$ROOT" rev-parse HEAD^{commit})
if "$TOOLS/is_ledger_only_commit.sh" "$BREAK"; then
  RC=10
fi
PR=$(git -C "$ROOT" log -1 --format='%s' "$BREAK" | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)
SUBJ=$(git -C "$ROOT" log -1 --format='%s' "$BREAK")
DIFF_FILES=$(git -C "$ROOT" diff --name-only "${BREAK}^" "$BREAK" | grep -v '^tests/omega/corpus_outcomes.txt' | grep -v '\.md$' | head -8 | tr '\n' ' ')
STAT=$(git -C "$ROOT" show --stat --format= "$BREAK" | head -8 | tr '\n' ' ')
git bisect reset 2>/dev/null || true
git -C "$ROOT" checkout -f work/ledger-bisect-part-11 >/dev/null 2>&1
python3 - "$OUT" "$WITNESS" "$BREAK" "$PR" "$SUBJ" "$STAT" "$DIFF_FILES" "$RC" "$GOOD" <<'PY'
import json, sys
out, witness, brk, pr, subj, stat, diff_files, rc, good = sys.argv[1:10]
mech = subj
if diff_files.strip():
    mech = f"{subj} — touched: {diff_files.strip()[:160]}"
elif stat.strip():
    mech = f"{subj} — {stat.strip()[:160]}"
ok = int(rc) in (0, 1) and brk != good
json.dump({
    "witness": witness,
    "breaking_commit": brk,
    "pr": pr or None,
    "mechanism": mech[:400],
    "status": "bisected" if ok else "propagated",
    "bisect_exit": int(rc),
    "good": good,
    "method": "fixed-pin-corpus",
}, open(out, "w"), indent=2)
print("bisected" if ok else "propagated", brk[:12], pr or "-")
PY
