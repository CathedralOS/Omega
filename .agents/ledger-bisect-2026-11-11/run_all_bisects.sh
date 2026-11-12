#!/bin/bash
set -euo pipefail
export OMEGA_REPO=/agent/repos/omega
export OMEGA_BISECT_TOOLS=/tmp/ledger-bisect-part-30-tools
export PATH="/home/ubuntu/.local/share/mbx/bin:$PATH"
PIN=51e8c45c793007f211d0d9baee7de16767172444
ART=/opt/cursor/artifacts/ledger-bisect-part-30
REPORT_DIR="$OMEGA_REPO/.agents/ledger-bisect-2026-11-11"
mkdir -p "$ART" "$OMEGA_BISECT_TOOLS"
cd "$OMEGA_REPO"
git checkout -qf work/ledger-bisect-part-30
cp -a "$REPORT_DIR/tools/"* "$OMEGA_BISECT_TOOLS/"
cp "$REPORT_DIR/run_one_bisect.sh" "$OMEGA_BISECT_TOOLS/"
chmod +x "$OMEGA_BISECT_TOOLS"/*.sh

run_cluster() {
  local witness="$1" good="$2" tag="$3"
  local log="$ART/bisect_${tag}.log"
  echo "=== BISECT $tag witness=$witness good=$good ===" | tee -a "$ART/run_all.log"
  bash "$OMEGA_BISECT_TOOLS/run_one_bisect.sh" "$witness" "$good" "$PIN" "$log" | tee -a "$ART/run_all.log"
  git checkout -qf work/ledger-bisect-part-30
}

REC_GOOD=$(bash "$REPORT_DIR/walk_good.sh" pass/facts/recoverable_replacement_window_through_const_name_exit "$PIN" 2>"$ART/walk_recoverable.err" | tail -1)
echo "recoverable good=$REC_GOOD" | tee -a "$ART/run_all.log"
run_cluster pass/facts/recoverable_replacement_window_through_const_name_exit "$REC_GOOD" recoverable

run_cluster pass/facts/opaque_independent_inherited_result_through_local_exit 741270df60f7 opaque

TRA_GOOD=$(bash "$REPORT_DIR/walk_good.sh" pass/traits/runtime_qualified_owned_requirement_calls_exit "$PIN" 2>"$ART/walk_traits.err" | tail -1)
run_cluster pass/traits/runtime_qualified_owned_requirement_calls_exit "$TRA_GOOD" traits

GUI_GOOD=$(bash "$REPORT_DIR/walk_good.sh" pass/effects/gui_semantic_services "$PIN" 2>"$ART/walk_gui.err" | tail -1)
run_cluster pass/effects/gui_semantic_services "$GUI_GOOD" gui

echo "ALL BISECTS DONE" | tee -a "$ART/run_all.log"
