#!/bin/bash
set -euo pipefail
WITNESS="${1:?witness}"
OUT_JSON="${2:?output json}"
TOOLS="$(cd "$(dirname "$0")" && pwd)"
export LEDGER_BISECT_TOOLS="$TOOLS"
ROOT="${LEDGER_BISECT_ROOT:-$(git -C "$TOOLS" rev-parse --show-toplevel)}"
PIN="51e8c45c793007f211d0d9baee7de16767172444"
cd "$ROOT"
git checkout -q "$PIN"
chmod +x "$TOOLS"/*.sh "$TOOLS"/*.py 2>/dev/null || true
# Fixed PIN context
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target-pin}"
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$CARGO_TARGET_DIR" mbx build -p omega < /dev/null >/dev/null 2>&1 || true
RUNNER=$(bash "$TOOLS/get_corpus_runner.sh")
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/pin_witness_record.txt
stable_record() {
  local r1=/tmp/pin_witness_record_1.txt r2=/tmp/pin_witness_record_2.txt
  for r in "$r1" "$r2"; do
    export OMEGA_CORPUS_RECORDS="$r"
    rm -f "$r"
    nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
  done
  s1=$(head -1 "$r1" | awk '{print $2}' 2>/dev/null || true)
  s2=$(head -1 "$r2" | awk '{print $2}' 2>/dev/null || true)
  if [[ -z "$s1" || -z "$s2" || "$s1" != "$s2" ]]; then
    echo "flaky witness at PIN status: $s1 vs $s2" >&2
    exit 2
  fi
  cp "$r1" /tmp/pin_witness_record.txt
}
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS"
stable_record
python3 "$TOOLS/pin_context.py" "$WITNESS" /tmp/pin_witness_record.txt
if [[ -z "${BISECT_GOOD:-}" ]]; then
  BISECT_GOOD=$(bash "$TOOLS/find_good_walk.sh" "$WITNESS" "$PIN" 1200) || {
    echo "no good commit for $WITNESS" >&2
    exit 2
  }
fi
GOOD="$(git rev-parse "$BISECT_GOOD")"
BAD="$(git rev-parse "$PIN")"
echo "bisect $WITNESS good=$GOOD bad=$BAD"
set +e
python3 "$TOOLS/bisect_test_pin.py" "$WITNESS" /tmp/pin_witness_record.txt
pin_rc=$?
set -e
if [[ "$pin_rc" != "1" ]]; then
  echo "PIN observation not bad (rc=$pin_rc)" >&2
  exit 2
fi
export BISECT_WITNESS="$WITNESS"
export GIT_PAGER=cat
git bisect reset 2>/dev/null || true
git bisect start "$BAD" "$GOOD" --first-parent
set +e
git bisect run "$TOOLS/bisect_witness.sh"
RC=$?
set -e
BREAK="$(git rev-parse HEAD^{commit} 2>/dev/null || true)"
git bisect log > "${OUT_JSON}.log" 2>/dev/null || true
git bisect reset 2>/dev/null || true
git checkout -q "$PIN"
PR=""
MECH=""
if [[ -n "$BREAK" ]]; then
  PR=$(git log -1 --format='%s' "$BREAK" | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)
  subj=$(git log -1 --format='%s' "$BREAK")
  files=$(git diff --name-only "${BREAK}^1" "$BREAK" | grep -E '^(omega-rust|source)/' | head -3 | tr '\n' ', ')
  MECH="${subj} (${files%,})"
fi
touched=$(git diff --name-only "${BREAK}^1" "$BREAK" 2>/dev/null | tr '\n' ' ')
STATUS=propagated
if [[ "$RC" -le 1 && -n "$BREAK" ]]; then
  if [[ "$touched" == *"tests/omega/corpus_outcomes.txt"* ]] && [[ "$touched" != *"omega-rust"* ]] && [[ "$touched" != *"source/"* ]]; then
    STATUS=propagated
    MECH="ledger-only commit; re-run required ($MECH)"
  else
    STATUS=bisected
  fi
fi
python3 - "$OUT_JSON" "$WITNESS" "$GOOD" "$BREAK" "$PR" "$STATUS" "$RC" "$MECH" <<'PY'
import json, sys
out, witness, good, brk, pr, status, rc, mech = sys.argv[1:9]
open(out, "w").write(json.dumps({
    "witness": witness,
    "good_commit": good,
    "breaking_commit": brk,
    "pr": pr or None,
    "mechanism": (mech or "")[:400],
    "status": status,
    "bisect_exit": int(rc),
}, indent=2))
PY
echo "$OUT_JSON" "$STATUS" "${BREAK:0:12}" "pr=$PR"
