#!/bin/bash
# Bisect one witness: fixed PIN ledger + first-parent good walk (methodology #2).
set -euo pipefail
WITNESS="${1:?witness}"
OUT_JSON="${2:?output json}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
PIN="51e8c45c793007f211d0d9baee7de16767172444"
TOOL_SRC="$ROOT/.agents/ledger-bisect-2026-11-11/tools"
TMP_TOOLS="/tmp/ledger-bisect-tools-7"
rm -rf "$TMP_TOOLS"
mkdir -p "$TMP_TOOLS"
cp -a "$TOOL_SRC/bisect_witness.sh" "$TOOL_SRC/bisect_test.py" "$TOOL_SRC/prepare_pin_snapshot.py" "$TMP_TOOLS/"
chmod +x "$TMP_TOOLS/bisect_witness.sh"
SCRIPT="$TMP_TOOLS/bisect_witness.sh"
export BISECT_WITNESS="$WITNESS"
export GIT_PAGER=cat
cd "$ROOT"

is_ledger_only_commit() {
  local c="$1"
  local -a files=()
  mapfile -t files < <(git diff-tree --no-commit-id --name-only -r "$c" 2>/dev/null || true)
  [[ ${#files[@]} -eq 0 ]] && return 1
  local f
  for f in "${files[@]}"; do
    [[ -z "$f" ]] && continue
    [[ "$f" == tests/omega/corpus_outcomes.txt ]] && continue
    [[ "$f" == *.md ]] && continue
    return 1
  done
  return 0
}

git checkout -q work/ledger-bisect-part-7
python3 "$TMP_TOOLS/prepare_pin_snapshot.py" "$WITNESS"
export BISECT_PIN_LEDGER=/tmp/ledger-bisect-pin/corpus_outcomes.txt
export BISECT_PIN_STATE=/tmp/ledger-bisect-pin/witness_state.json

run_witness_test() {
  set +e
  nice -n 19 "$SCRIPT"
  local rc=$?
  set -e
  return "$rc"
}

find_good_commit() {
  local low=759c80404428e6d9922a8b67cce32daf7e41a02e
  git checkout -q "$PIN"
  if run_witness_test; then
    echo "PIN tests good but expected bad" >&2
    echo ""
    return 1
  fi
  git checkout -q "$low"
  if ! run_witness_test; then
    local cur="$low"
    local parent rc
    while parent=$(git rev-parse "$cur^" 2>/dev/null); do
      if is_ledger_only_commit "$parent"; then
        cur="$parent"
        continue
      fi
      git checkout -q "$parent"
      run_witness_test
      rc=$?
      if [[ "$rc" -eq 0 ]]; then
        echo "$parent"
        return 0
      fi
      cur="$parent"
    done
    echo ""
    return 1
  fi
  local -a revs=()
  mapfile -t revs < <(git rev-list --first-parent "$PIN^" --not "$low")
  local mid lo=0 hi=$((${#revs[@]} - 1)) found=""
  # rev-list is newest-first; seek newest ancestor that still tests good (0).
  while (( lo <= hi )); do
    mid=$(( (lo + hi) / 2 ))
    local c="${revs[$mid]}"
    if is_ledger_only_commit "$c"; then
      if (( mid + 1 <= hi )); then
        lo=$((mid + 1))
      else
        hi=$((mid - 1))
      fi
      continue
    fi
    git checkout -q "$c"
    run_witness_test
    rc=$?
    if [[ "$rc" -eq 0 ]]; then
      found="$c"
      lo=$((mid + 1))
    else
      hi=$((mid - 1))
    fi
  done
  if [[ -n "$found" ]]; then
    echo "$found"
    return 0
  fi
  echo "$low"
  return 0
}

if [[ -z "${BISECT_GOOD:-}" ]]; then
  GOOD=$(find_good_commit) || true
else
  GOOD="$BISECT_GOOD"
fi
git checkout -q work/ledger-bisect-part-7
[[ -n "$GOOD" ]] || { echo "no good commit for $WITNESS (walk exhausted)" >&2; exit 2; }
echo "GOOD=$GOOD for $WITNESS" >&2

git bisect reset 2>/dev/null || true
if ! git bisect start "$PIN" "$GOOD" --first-parent 2>/dev/null; then
  git bisect start
  git bisect bad "$PIN"
  git bisect good "$GOOD"
fi
set +e
nice -n 19 git bisect run "$SCRIPT"
RC=$?
set -e
BREAK=$(git rev-parse HEAD^{commit} 2>/dev/null || true)
git bisect log >"${OUT_JSON%.json}.log" 2>/dev/null || true
git bisect reset 2>/dev/null || true
git checkout -q work/ledger-bisect-part-7 2>/dev/null || true

ledger_only_break=0
if [[ -n "$BREAK" ]] && is_ledger_only_commit "$BREAK"; then
  ledger_only_break=1
fi

PR=""
if [[ -n "$BREAK" ]]; then
  PR=$(git log -1 --format='%s' "$BREAK" 2>/dev/null | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)
fi
python3 - "$OUT_JSON" "$WITNESS" "$BREAK" "$PR" "$RC" "$GOOD" "$ledger_only_break" <<'PY'
import json, re, subprocess, sys

out, witness, brk, pr, rc, good, ledger_only = sys.argv[1:8]
mech = ""
status = "propagated"
if brk and ledger_only == "1":
    mech = "invalid bisect: breaking commit touches only ledger/md"
    status = "propagated"
elif brk and int(rc) == 0:
    subj = subprocess.run(
        ["git", "log", "-1", "--format=%s", brk], capture_output=True, text=True
    ).stdout.strip()
    names = subprocess.run(
        ["git", "show", "--name-only", "--format=", brk],
        capture_output=True,
        text=True,
    ).stdout.strip().splitlines()
    top = [n for n in names if n and not n.startswith(".")][:4]
    path_hint = ", ".join(top) if top else ""
    mech = subj
    if path_hint and path_hint not in subj:
        mech = f"{subj} ({path_hint})"
    mech = re.sub(r"\s+", " ", mech)[:500]
    status = "bisected"
payload = {
    "witness": witness,
    "good": good,
    "breaking_commit": brk,
    "pr": pr or None,
    "mechanism": mech[:500],
    "status": status,
    "bisect_exit": int(rc),
    "ledger_only_break": ledger_only == "1",
}
open(out, "w").write(json.dumps(payload, indent=2))
open(out + ".tmp", "w").write(json.dumps(payload, indent=2))
print(json.dumps(payload, indent=2))
PY
