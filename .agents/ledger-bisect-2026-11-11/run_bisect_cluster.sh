#!/bin/bash
set -euo pipefail
WITNESS="${1:?}"
CLUSTER_ID="${2:?}"
PIN=51e8c45c793007f211d0d9baee7de16767172444
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
TOOLHOME=/tmp/ledger-bisect-part-21-tools
mkdir -p "$TOOLHOME"
mkdir -p "$TOOLHOME/tools"
cp -a "$ROOT/.agents/ledger-bisect-2026-11-11/tools/." "$TOOLHOME/tools/"
cp -a "$ROOT/.agents/ledger-bisect-2026-11-11/prepare_witness.sh" "$ROOT/.agents/ledger-bisect-2026-11-11/part21_observed.txt" "$TOOLHOME/"
cd "$ROOT"
SCRIPT="$TOOLHOME/tools/bisect_witness.sh"
export BISECT_WITNESS="$WITNESS"
export BISECT_REPO_ROOT="$ROOT"
chmod -R u+w "$ROOT/target" 2>/dev/null || true

SPEC=$(bash "$TOOLHOME/prepare_witness.sh" "$WITNESS")
export BISECT_SPEC="$SPEC"
export BISECT_LEDGER=/tmp/ledger-bisect-pin-corpus_outcomes.txt
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4

git checkout -f -q "$PIN"
set +e
nice -n 19 "$SCRIPT"
pin_ec=$?
set -e
if [[ "$pin_ec" -ne 1 ]]; then
  echo "cluster=$CLUSTER_ID witness=$WITNESS status=propagated reason=pin_exit_${pin_ec}"
  exit 0
fi

FINDLOG="/tmp/find-good-${CLUSTER_ID}.log"
: >"$FINDLOG"
GOOD=""
rev="$PIN"
for _ in $(seq 1 500); do
  parent=$(git rev-parse -q "${rev}^1" 2>/dev/null) || break
  rev="$parent"
  git checkout -f -q "$rev"
  set +e
  nice -n 19 "$SCRIPT"
  ec=$?
  set -e
  echo "walk $rev exit=$ec $(git log -1 --oneline "$rev" 2>/dev/null)" >>"$FINDLOG"
  if [[ "$ec" -eq 0 ]]; then
    GOOD="$rev"
    break
  fi
done
git checkout -f -q "$PIN"
if [[ -z "$GOOD" ]]; then
  echo "cluster=$CLUSTER_ID witness=$WITNESS status=propagated reason=no_good_on_first_parent"
  exit 0
fi

git bisect reset 2>/dev/null || true
git bisect start --first-parent "$PIN" "$GOOD"
set +e
git bisect run "$SCRIPT"
RC=$?
set -e
BREAK=$(git rev-parse HEAD^{commit} 2>/dev/null || echo "")
PR=$(git log -1 --format='%s' "$BREAK" 2>/dev/null | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)
git bisect reset
git checkout -f -q "$PIN"

if [[ -n "$BREAK" ]]; then
  p=$(git rev-parse -q "$BREAK^1" 2>/dev/null || true)
  if [[ -n "$p" ]]; then
    mapfile -t touched < <(git diff --name-only "$p" "$BREAK" 2>/dev/null || true)
    ledger_only=1
    for f in "${touched[@]}"; do
      [[ "$f" == tests/omega/corpus_outcomes.txt ]] && continue
      case "$f" in *.md) continue ;; esac
      ledger_only=0
    done
    if [[ "$ledger_only" -eq 1 && ${#touched[@]} -gt 0 ]]; then
      echo "cluster=$CLUSTER_ID witness=$WITNESS good=$GOOD status=propagated reason=ledger_only_break_${BREAK:0:12}"
      exit 0
    fi
  fi
fi

MECH=$(python3 - "$BREAK" <<'PY'
import subprocess, sys
brk = sys.argv[1]
if not brk:
    print(""); raise SystemExit
subj = subprocess.check_output(["git", "log", "-1", "--format=%s", brk], text=True).strip()
files = subprocess.check_output(["git", "show", "--no-renames", "--format=", "--name-only", brk], text=True).splitlines()
code = [f for f in files if f.startswith(("omega-rust/", "source/"))][:4]
print(f"{subj} ({', '.join(code)})"[:200])
PY
)
echo "cluster=$CLUSTER_ID witness=$WITNESS good=$GOOD break=$BREAK pr=$PR rc=$RC mech=$MECH status=bisected"
