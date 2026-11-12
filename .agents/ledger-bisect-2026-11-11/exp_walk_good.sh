#!/bin/bash
set -euo pipefail
WITNESS="${1:?}"
PIN="${2:-51e8c45c793007f211d0d9baee7de16767172444}"
BAD_ANCHOR="${3:-$PIN}"
export OMEGA_REPO="/agent/repos/omega"
export OMEGA_BISECT_TOOLS="/tmp/ledger-bisect-part-30-tools"
export OMEGA_PIN_ROOT="/tmp/ledger-bisect-part-30"
export OMEGA_PIN_LEDGER="$OMEGA_PIN_ROOT/pin_corpus_outcomes.txt"
export OMEGA_PIN_OBS="$OMEGA_PIN_ROOT/pin_obs_${WITNESS//\//__}.txt"
export PATH="/home/ubuntu/.local/share/mbx/bin:$PATH"
export BISECT_WITNESS="$WITNESS"
SCRIPT="$OMEGA_BISECT_TOOLS/bisect_witness.sh"
LOG="${4:-/tmp/ledger-bisect-part-30/exp_walk.log}"
cd "$OMEGA_REPO"

restore_report() {
  git checkout -qf work/ledger-bisect-part-30 -- \
    .agents/ledger-bisect-2026-11-11/part-30.md \
    .agents/ledger-bisect-2026-11-11/part-30.tsv 2>/dev/null || true
}

fp_parent() {
  git rev-parse "${1}^1" 2>/dev/null || echo ""
}

fp_back() {
  local c="$1" n="$2"
  while ((n > 0)) && [[ -n "$c" ]]; do
    c=$(fp_parent "$c") || true
    ((n--)) || true
  done
  echo "$c"
}

test_commit() {
  local c="$1" r
  [[ -z "$c" ]] && return 125
  git checkout -qf "$c"
  restore_report
  "$SCRIPT"
  r=$?
  return "$r"
}

log() { echo "$*" >>"$LOG"; }

: >"$LOG"
PIN=$(git rev-parse "$PIN")
BAD_ANCHOR=$(git rev-parse "$BAD_ANCHOR")

set +e
test_commit "$PIN"
prc=$?
set -e
log "PIN $(git log -1 --oneline "$PIN") -> $prc"
[[ "$prc" -eq 1 ]] || { log "ERROR: PIN must exit 1"; exit 2; }

cur="$BAD_ANCHOR"
set +e
test_commit "$cur"
arc=$?
set -e
log "ANCHOR $(git log -1 --oneline "$cur") -> $arc"

older_good=""
gap=1
while ((gap <= 65536)); do
  target=$(fp_back "$cur" "$gap")
  [[ -z "$target" ]] && break
  set +e
  test_commit "$target"
  rc=$?
  set -e
  log "exp gap=$gap $(git log -1 --oneline "$target") -> $rc"
  if [[ "$rc" -eq 0 ]]; then
    older_good="$target"
    break
  fi
  if [[ "$rc" -eq 1 ]]; then
    cur="$target"
    gap=$((gap * 2))
    continue
  fi
  if [[ "$rc" -eq 125 ]]; then
    cur=$(fp_parent "$target")
    [[ -z "$cur" ]] && break
    continue
  fi
  cur=$(fp_parent "$cur")
  [[ -z "$cur" ]] && break
done

if [[ -z "$older_good" ]]; then
  log "NEVER_FOUND_GOOD exponential"
  git checkout -qf work/ledger-bisect-part-30
  exit 3
fi

# Newest good on first-parent path from older_good toward PIN.
good="$older_good"
for c in $(git rev-list --reverse --first-parent "$older_good..$PIN"); do
  set +e
  test_commit "$c"
  rc=$?
  set -e
  log "refine $(git log -1 --oneline "$c") -> $rc"
  [[ "$rc" -eq 0 ]] && good="$c"
done

set +e
test_commit "$good"
grc=$?
set -e
log "final good $(git log -1 --oneline "$good") -> $grc"
[[ "$grc" -eq 0 ]] || { log "ERROR: refined good not 0"; exit 2; }

git checkout -qf work/ledger-bisect-part-30
log "GOOD_SHA=$good"
echo "$good"
