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
SAVED_BRANCH=work/ledger-bisect-part-21
SAVED_PATH=".agents/ledger-bisect-2026-11-11/find-good-${CLUSTER_ID}.log"
if git show "$SAVED_BRANCH:$SAVED_PATH" >"$FINDLOG" 2>/dev/null; then
  :
elif [[ -f "$ROOT/$SAVED_PATH" ]]; then
  cp "$ROOT/$SAVED_PATH" "$FINDLOG"
else
  : >"$FINDLOG"
fi
INTRO=$(python3 - "$WITNESS" <<'PY'
import subprocess, sys
witness = sys.argv[1]
rel = f"tests/omega/{witness}/build.omg"
intro = subprocess.check_output(
    ["git", "log", "--reverse", "--format=%H", "--", rel], text=True
).splitlines()
print(intro[0] if intro else "")
PY
)
mapfile -t CHAIN < <(python3 - "$INTRO" "$PIN" <<'PY'
import subprocess, sys
intro, pin = sys.argv[1], sys.argv[2]
if not intro:
    raise SystemExit
revs = []
cur = pin
while True:
    revs.append(cur)
    if cur == intro:
        break
    cur = subprocess.check_output(["git", "rev-parse", f"{cur}^1"], text=True).strip()
revs.reverse()
print("\n".join(revs))
PY
)
GOOD=$(awk '/^probe / && /exit=0/ {rev=$2} END{print rev}' "$FINDLOG")
if [[ -z "$GOOD" ]]; then
  lo=0
  hi=$((${#CHAIN[@]} - 1))
  while [[ "$lo" -le "$hi" ]]; do
    mid=$(( (lo + hi) / 2 ))
    rev="${CHAIN[$mid]}"
    git checkout -f -q "$rev"
    set +e
    nice -n 19 "$SCRIPT"
    ec=$?
    set -e
    echo "probe $rev exit=$ec $(git log -1 --oneline "$rev" 2>/dev/null)" >>"$FINDLOG"
    if [[ "$ec" -eq 0 ]]; then
      GOOD="$rev"
      lo=$((mid + 1))
    else
      hi=$((mid - 1))
    fi
  done
fi
git checkout -f -q "$PIN"
mkdir -p /tmp/ledger-bisect-saved
cp "$FINDLOG" "/tmp/ledger-bisect-saved/find-good-${CLUSTER_ID}.log" 2>/dev/null || true
if [[ -z "$GOOD" ]]; then
  echo "cluster=$CLUSTER_ID witness=$WITNESS status=propagated reason=no_good_on_first_parent"
  exit 0
fi

mapfile -t PROBE_CHAIN < <(python3 - "$GOOD" "$PIN" <<'PY'
import subprocess, sys

good, pin = sys.argv[1], sys.argv[2]
revs = []
cur = pin
while True:
    revs.append(cur)
    if cur == good:
        break
    cur = subprocess.check_output(["git", "rev-parse", f"{cur}^1"], text=True).strip()
revs.reverse()


def touches_compiler(rev: str) -> bool:
    parent = subprocess.check_output(["git", "rev-parse", f"{rev}^1"], text=True).strip()
    files = subprocess.check_output(
        ["git", "diff", "--name-only", parent, rev], text=True
    ).splitlines()
    if not files:
        return False
    for path in files:
        if path == "tests/omega/corpus_outcomes.txt":
            continue
        if path.endswith(".md"):
            continue
        return True
    return False


for rev in revs[1:]:
    if touches_compiler(rev):
        print(rev)
PY
)

BREAK=""
RC=0
if [[ ${#PROBE_CHAIN[@]} -eq 0 ]]; then
  echo "cluster=$CLUSTER_ID witness=$WITNESS good=$GOOD status=propagated reason=no_compiler_commits"
  git checkout -f -q "$PIN"
  exit 0
fi
lo=0
hi=$((${#PROBE_CHAIN[@]} - 1))
git checkout -f -q "${PROBE_CHAIN[$hi]}"
set +e
nice -n 19 "$SCRIPT"
pin_probe=$?
set -e
echo "walk ${PROBE_CHAIN[$hi]} exit=$pin_probe $(git log -1 --oneline "${PROBE_CHAIN[$hi]}" 2>/dev/null)" >>"$FINDLOG"
if [[ "$pin_probe" -ne 1 ]]; then
  echo "cluster=$CLUSTER_ID witness=$WITNESS good=$GOOD status=propagated reason=pin_not_bad_on_compiler_chain_${pin_probe}"
  git checkout -f -q "$PIN"
  exit 0
fi
while [[ "$lo" -lt "$hi" ]]; do
  mid=$(( (lo + hi) / 2 ))
  rev="${PROBE_CHAIN[$mid]}"
  git checkout -f -q "$rev"
  set +e
  nice -n 19 "$SCRIPT"
  ec=$?
  set -e
  echo "walk $rev exit=$ec $(git log -1 --oneline "$rev" 2>/dev/null)" >>"$FINDLOG"
  if [[ "$ec" -eq 1 ]]; then
    hi="$mid"
  elif [[ "$ec" -eq 0 ]]; then
    lo=$((mid + 1))
  else
    hi="$mid"
  fi
done
BREAK="${PROBE_CHAIN[$lo]}"
git checkout -f -q "$BREAK"
set +e
nice -n 19 "$SCRIPT"
confirm=$?
set -e
echo "walk_confirm $BREAK exit=$confirm $(git log -1 --oneline "$BREAK" 2>/dev/null)" >>"$FINDLOG"
RC=$confirm
if [[ "$confirm" -ne 1 ]]; then
  mapfile -t CHAIN_JOINED < <(printf '%s\n' "${PROBE_CHAIN[@]}")
  BREAK=$(python3 - "$FINDLOG" "${CHAIN_JOINED[@]}" <<'PY'
import sys
from pathlib import Path

log_path = Path(sys.argv[1])
chain = sys.argv[2:]
bad: set[str] = set()
for line in log_path.read_text().splitlines():
    if not line.startswith("walk "):
        continue
    parts = line.split()
    if len(parts) >= 3 and parts[2] == "exit=1":
        bad.add(parts[1])
best = ""
best_i = len(chain)
for i, rev in enumerate(chain):
    if rev in bad and i < best_i:
        best_i = i
        best = rev
print(best)
PY
)
  if [[ -z "$BREAK" ]]; then
    echo "cluster=$CLUSTER_ID witness=$WITNESS good=$GOOD status=propagated reason=break_not_bad_${confirm}"
    git checkout -f -q "$PIN"
    exit 0
  fi
  RC=1
fi
PR=$(git log -1 --format='%s' "$BREAK" 2>/dev/null | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)
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
