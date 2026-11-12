#!/bin/bash
set -euo pipefail
export OMEGA_REPO=/agent/repos/omega
export OMEGA_BISECT_TOOLS=/tmp/ledger-bisect-part-30-tools
export PATH="/home/ubuntu/.local/share/mbx/bin:$PATH"
PIN=51e8c45c793007f211d0d9baee7de16767172444
cd "$OMEGA_REPO"
cp -a "$OMEGA_REPO/.agents/ledger-bisect-2026-11-11/tools/"* "$OMEGA_BISECT_TOOLS/"
cp "$OMEGA_REPO/.agents/ledger-bisect-2026-11-11/run_one_bisect.sh" "$OMEGA_BISECT_TOOLS/"
chmod +x "$OMEGA_BISECT_TOOLS"/*.sh

run_cluster() {
  local witness="$1" good="$2" tag="$3"
  echo "=== BISECT $tag witness=$witness good=$good ==="
  bash "$OMEGA_BISECT_TOOLS/run_one_bisect.sh" "$witness" "$good" "$PIN" \
    "$OMEGA_REPO/.agents/ledger-bisect-2026-11-11/bisect_${tag}.log"
}

REC_GOOD=$(bash "$OMEGA_REPO/.agents/ledger-bisect-2026-11-11/walk_good.sh" \
  pass/facts/recoverable_replacement_window_through_const_name_exit "$PIN" | tail -1)
echo "recoverable good=$REC_GOOD"
run_cluster pass/facts/recoverable_replacement_window_through_const_name_exit "$REC_GOOD" recoverable

# MID verified good for opaque at 741270df; PIN bad
run_cluster pass/facts/opaque_independent_inherited_result_through_local_exit 741270df60f7 opaque

TRA_GOOD=$(bash "$OMEGA_REPO/.agents/ledger-bisect-2026-11-11/walk_good.sh" \
  pass/traits/runtime_qualified_owned_requirement_calls_exit "$PIN" | tail -1)
run_cluster pass/traits/runtime_qualified_owned_requirement_calls_exit "$TRA_GOOD" traits

GUI_GOOD=$(bash "$OMEGA_REPO/.agents/ledger-bisect-2026-11-11/walk_good.sh" \
  pass/effects/gui_semantic_services "$PIN" | tail -1)
run_cluster pass/effects/gui_semantic_services "$GUI_GOOD" gui

echo "ALL BISECTS DONE"
