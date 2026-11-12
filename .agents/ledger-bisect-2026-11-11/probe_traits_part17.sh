#!/bin/bash
set -euo pipefail
WITNESS="pass/traits/runtime_qualified_owned_requirement_calls_exit"
PIN="${1:-51e8c45c793007f211d0d9baee7de16767172444}"
BREAK=$(git -C "$REPO" rev-parse 29f25c7d)
GOOD_PARENT="${BREAK}^1"
REPO="/agent/repos/omega"
cd "$REPO"
TOOLS="/tmp/ledger-bisect-part-30-tools"
RUNTIME="/tmp/ledger-bisect-part-30"
LOG="$RUNTIME/traits_probe.log"
export OMEGA_REPO="$REPO" OMEGA_BISECT_TOOLS="$TOOLS" PATH="/home/ubuntu/.local/share/mbx/bin:$PATH"
export OMEGA_PIN_ROOT="$RUNTIME" BISECT_WITNESS="$WITNESS"
TOOLS_REF="work/ledger-bisect-part-30"
mkdir -p "$RUNTIME" "$TOOLS"
git -C "$REPO" show "$TOOLS_REF:.agents/ledger-bisect-2026-11-11/tools/bisect_test.py" >"$TOOLS/bisect_test.py"
git -C "$REPO" show "$TOOLS_REF:.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh" >"$TOOLS/bisect_witness.sh"
git -C "$REPO" show "$TOOLS_REF:.agents/ledger-bisect-2026-11-11/run_one_bisect.sh" >"$TOOLS/run_one_bisect.sh"
chmod +x "$TOOLS"/*.sh
bash "$REPO/.agents/ledger-bisect-2026-11-11/setup_pin_refs.sh" "$PIN" "$WITNESS" >/dev/null
export OMEGA_PIN_LEDGER="$RUNTIME/pin_corpus_outcomes.txt"
export OMEGA_PIN_OBS="$RUNTIME/pin_obs_${WITNESS//\//__}.txt"

restore_report() {
  git checkout -qf work/ledger-bisect-part-30 -- \
    .agents/ledger-bisect-2026-11-11/part-30.md \
    .agents/ledger-bisect-2026-11-11/part-30.tsv 2>/dev/null || true
}

probe() {
  local c="$1"
  git checkout -qf "$c"
  restore_report
  rm -f /tmp/ledger-bisect-part-30-target/.bisect_head
  set +e
  local o=0 b=0 s=0 r
  for _ in 1 2; do
    bash "$TOOLS/bisect_witness.sh" >/dev/null 2>&1
    r=$?
    case $r in 0) o=$((o + 1));; 1) b=$((b + 1));; *) s=$((s + 1));; esac
  done
  set -e
  echo "PROBE $(git log -1 --oneline "$c") ok=$o bad=$b skip=$s"
  echo "$o $b $s"
}

: >"$LOG"
GOOD_PARENT=$(git rev-parse "$GOOD_PARENT")
PIN=$(git rev-parse "$PIN")
read -r po pb ps < <(probe "$GOOD_PARENT" | tail -1)
read -r bo bb bs < <(probe "$BREAK" | tail -1)
probe "$GOOD_PARENT" | tee -a "$LOG"
probe "$BREAK" | tee -a "$LOG"

if [[ "$po" -eq 2 && "$bb" -eq 2 && "$ps" -eq 0 && "$bs" -eq 0 ]]; then
  echo "PROBE_OK" | tee -a "$LOG"
  BLOG="$RUNTIME/bisect_traits.log"
  bash "$TOOLS/run_one_bisect.sh" "$WITNESS" "$GOOD_PARENT" "$PIN" "$BLOG"
  python3 "$REPO/.agents/ledger-bisect-2026-11-11/record_cluster.py" traits "$BLOG" bisected
  git checkout -qf work/ledger-bisect-part-30
  exit 0
fi
echo "PROBE_FAIL good=$po/$pb/$ps break=$bo/$bb/$bs" | tee -a "$LOG"
exit 1
