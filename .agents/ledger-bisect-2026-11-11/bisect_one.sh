#!/bin/bash
set -euo pipefail
WITNESS="${1:?witness}"
OUT_JSON="${2:?out.json}"
PIN="${3:-51e8c45c793007f211d0d9baee7de16767172444}"
MAIN_REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
ROOT="${BISECT_REPO_ROOT:-$MAIN_REPO}"
SCRIPT="$MAIN_REPO/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
FIND_GOOD="$MAIN_REPO/.agents/ledger-bisect-2026-11-11/find_good_commit.sh"
PREPARE="$MAIN_REPO/.agents/ledger-bisect-2026-11-11/tools/prepare_bisect_expect.py"
ENRICH="$MAIN_REPO/.agents/ledger-bisect-2026-11-11/tools/enrich_bisect_evidence.py"
cd "$ROOT"
START_SHA="$(git rev-parse HEAD)"

export BISECT_REPO_ROOT="$ROOT"
export BISECT_PIN_LEDGER="${BISECT_PIN_LEDGER:-/tmp/bisect_pin_corpus_outcomes.txt}"
export BISECT_EXPECT_FILE="$(BISECT_PIN_LEDGER="$BISECT_PIN_LEDGER" python3 "$PREPARE" "$WITNESS")"
commit_is_ledger_only() {
  local brk="$1"
  mapfile -t files < <(git diff-tree --no-commit-id --name-only -r "$brk" 2>/dev/null)
  [[ ${#files[@]} -eq 0 ]] && return 1
  local compiler=0
  for f in "${files[@]}"; do
    case "$f" in tests/omega/corpus_outcomes.txt|*.md) ;; *) compiler=1; break ;; esac
  done
  [[ "$compiler" -eq 0 ]]
}
mechanism_from_diff() {
  local brk="$1"
  local subj
  subj="$(git log -1 --format='%s' "$brk" 2>/dev/null)"
  mapfile -t files < <(git diff-tree --no-commit-id --name-only -r "$brk" 2>/dev/null | grep -v '^tests/omega/corpus_outcomes.txt$' | grep -v '\.md$' | head -6)
  if [[ ${#files[@]} -eq 0 ]]; then
    echo "$subj"
    return
  fi
  echo "$subj — ${files[0]}"
}
linear_first_compiler_bad() {
  local good="$1" pin="$2"
  local c t found=""
  while IFS= read -r c; do
    git checkout -q "$c" 2>/dev/null || continue
    commit_is_ledger_only "$c" && continue
    set +e
    "$SCRIPT" >/dev/null 2>&1
    t=$?
    set -e
    if [[ "$t" -eq 1 ]]; then
      found="$c"
      break
    fi
  done < <(git rev-list --first-parent --reverse "${good}..${pin}")
  echo "$found"
}
USED_LINEAR=0
FG_ERR="$(mktemp)"
set +e
GOOD="$(nice -n 19 "$FIND_GOOD" "$WITNESS" 2>"$FG_ERR")"
FG_RC=$?
set -e
if [[ "$FG_RC" -eq 2 ]] || grep -q '^NEVER_GOOD:' "$FG_ERR" 2>/dev/null; then
  FIX_ADD="$(grep '^NEVER_GOOD:' "$FG_ERR" | head -1 | cut -d: -f2-)"
  MECH_NG="never-good since ${FIX_ADD}"
  python3 - "$OUT_JSON" "$WITNESS" "$MECH_NG" "$FIX_ADD" <<'PY'
import json, sys
open(sys.argv[1], "w").write(json.dumps({
    "witness": sys.argv[2], "breaking_commit": "", "fixture_added": sys.argv[4],
    "pr": None, "mechanism": sys.argv[3], "status": "propagated",
    "bisect_exit": 2, "stability": "unknown", "find_good_method": "exponential",
}, indent=2))
print("propagated never-good", sys.argv[2])
PY
  rm -f "$FG_ERR"
  git checkout -f "$START_SHA" >/dev/null 2>&1
  exit 0
fi
rm -f "$FG_ERR"
if [[ "$FG_RC" -ne 0 || -z "$GOOD" ]]; then
  python3 - "$OUT_JSON" "$WITNESS" <<'PY'
import json, sys
open(sys.argv[1], "w").write(json.dumps({
    "witness": sys.argv[2], "breaking_commit": "", "pr": None,
    "mechanism": "no good commit on exponential first-parent walk from PIN",
    "status": "propagated", "bisect_exit": 2, "stability": "unknown",
    "find_good_method": "exponential",
}, indent=2))
print("propagated no-good", sys.argv[2])
PY
  git checkout -f "$START_SHA" >/dev/null 2>&1
  exit 0
fi

echo "witness=$WITNESS good=$GOOD bad=$PIN" >&2
export BISECT_WITNESS="$WITNESS"
export GIT_PAGER=cat
git checkout -f "$START_SHA" >/dev/null 2>&1
git bisect reset >/dev/null 2>&1 || true
BSTART=$(git bisect start --first-parent "$PIN" "$GOOD" 2>&1)
echo "$BSTART" >&2
if ! echo "$BSTART" | grep -qiE 'Bisecting|revisions left'; then
  python3 - "$OUT_JSON" "$WITNESS" "$GOOD" <<'PY'
import json, sys
open(sys.argv[1], "w").write(json.dumps({
    "witness": sys.argv[2], "good_commit": sys.argv[3], "breaking_commit": "", "pr": None,
    "mechanism": f"git bisect start failed between PIN and good {sys.argv[3]}",
    "status": "propagated", "bisect_exit": 2, "find_good_method": "exponential",
}, indent=2))
PY
  exit 0
fi

export BISECT_WITNESS="$WITNESS"
git checkout -q "$GOOD"
g1=$("$SCRIPT"; echo $?)
git checkout -q "$PIN"
b1=$("$SCRIPT"; echo $?)
git checkout -f "$START_SHA" >/dev/null 2>&1
echo "sanity good=$g1 bad=$b1" >&2
STABILITY="deterministic"
if [[ "$g1" != "0" || "$b1" != "1" ]]; then
  git checkout -q "$GOOD"; g2=$("$SCRIPT"; echo $?); g3=$("$SCRIPT"; echo $?)
  git checkout -q "$PIN"; b2=$("$SCRIPT"; echo $?); b3=$("$SCRIPT"; echo $?)
  git checkout -f "$START_SHA" >/dev/null 2>&1
  if [[ "$g1" != "$g2" || "$g2" != "$g3" || "$b1" != "$b2" || "$b2" != "$b3" ]]; then
    STABILITY="flaky"
  fi
  if [[ "$g1" != "0" ]]; then
    python3 - "$OUT_JSON" "$WITNESS" "$GOOD" "$STABILITY" "$g1" "$b1" <<'PY'
import json, sys
open(sys.argv[1], "w").write(json.dumps({
    "witness": sys.argv[2], "good_commit": sys.argv[3], "breaking_commit": "",
    "mechanism": f"sanity failed: good exit {sys.argv[5]} bad exit {sys.argv[6]} (expected 0/1); stability={sys.argv[4]}",
    "status": "propagated", "bisect_exit": 2, "stability": sys.argv[4], "find_good_method": "exponential",
}, indent=2))
PY
    exit 0
  fi
fi

export BISECT_STEP_LOG="${BISECT_STEP_LOG:-$MAIN_REPO/.agents/ledger-bisect-2026-11-11/bisect_steps.tsv}"
: >"$BISECT_STEP_LOG"
git bisect reset >/dev/null 2>&1 || true
git bisect start --first-parent "$PIN" "$GOOD" >/dev/null
set +e
git bisect run "$SCRIPT"
RC=$?
set -e
BREAK=$(git rev-parse HEAD^{commit} 2>/dev/null || true)
git bisect reset >/dev/null 2>&1 || true
git checkout -f "$START_SHA" >/dev/null 2>&1
PR=""
MECH=""
STATUS=propagated
SKIP_NOTE=""
if [[ -n "$BREAK" && "$RC" -le 1 && "$BREAK" != "$PIN" ]] && ! commit_is_ledger_only "$BREAK"; then
  STATUS=bisected
  PR=$(git log -1 --format='%s' "$BREAK" 2>/dev/null | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)
  MECH="$(mechanism_from_diff "$BREAK")"
elif [[ -n "$BREAK" ]] && commit_is_ledger_only "$BREAK"; then
  MECH="git bisect landed on ledger-only commit; linear scan follows"
elif [[ "$RC" -gt 1 ]]; then
  SKIP_NOTE="git bisect ended skip-only between $GOOD and $PIN (exit $RC)"
fi
if [[ "$STATUS" != "bisected" ]]; then
  FB="$(linear_first_compiler_bad "$GOOD" "$PIN")"
  if [[ -n "$FB" && "$FB" != "$PIN" ]] && ! commit_is_ledger_only "$FB"; then
    BREAK="$FB"
    STATUS=bisected
    RC=0
    USED_LINEAR=1
    PR=$(git log -1 --format='%s' "$BREAK" 2>/dev/null | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)
    MECH="$(mechanism_from_diff "$BREAK")"
    [[ -n "$SKIP_NOTE" ]] && MECH="${MECH}; ${SKIP_NOTE}"
  fi
fi
python3 - "$OUT_JSON" "$WITNESS" "$BREAK" "$PR" "$MECH" "$STATUS" "$RC" "$GOOD" "$STABILITY" "$USED_LINEAR" "$SKIP_NOTE" <<'PY'
import json, sys
out, witness, brk, pr, mech, status, rc, good, stab, linear, skip = sys.argv[1:12]
obj = {
    "witness": witness, "good_commit": good, "breaking_commit": brk, "pr": pr or None,
    "mechanism": mech[:500], "status": status, "bisect_exit": int(rc), "stability": stab,
    "find_good_method": "exponential", "used_linear_fallback": linear == "1",
}
if skip:
    obj["bisect_skip_note"] = skip
open(out, "w").write(json.dumps(obj, indent=2))
print(status, (brk or "?")[:12], "pr", pr, "good", good[:12])
PY
BISECT_REPO_ROOT="$ROOT" python3 "$ENRICH" "$OUT_JSON"
git checkout -f "$START_SHA" >/dev/null 2>&1
