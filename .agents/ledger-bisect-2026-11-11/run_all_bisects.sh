#!/bin/bash
set -euo pipefail
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
PIN=51e8c45c793007f211d0d9baee7de16767172444
python3 "$ROOT/.agents/ledger-bisect-2026-11-11/setup_pin_specs.py"
declare -A WITNESS=( [0]=pass/borrows/runtime_selected_reference_mutation_complement_exit [1]=pass/facts/computed_record_window_through_other_machine_exit [2]=pass/facts/match_computed_root_through_other_state_exit [3]=pass/generics/generic_result_parameter_receipt )
for id in 0 1 2 3; do
  w="${WITNESS[$id]}"
  export BISECT_WITNESS="$w"
  git checkout -q work/ledger-bisect-part-31
  bash "$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh" >/dev/null; test $? -eq 1 || { echo "PIN not bad for $w"; exit 1; }
  if [[ ! -f "$ROOT/.agents/ledger-bisect-2026-11-11/cluster${id}-good.sha" ]]; then
    python3 "$ROOT/.agents/ledger-bisect-2026-11-11/find_good_commit.py" "$w" "$PIN" | head -1 >"$ROOT/.agents/ledger-bisect-2026-11-11/cluster${id}-good.sha"
  fi
  GOOD="$(tr -d ' \n' <"$ROOT/.agents/ledger-bisect-2026-11-11/cluster${id}-good.sha")"
  git checkout -q "$GOOD"
  bash "$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh" >/dev/null; test $? -eq 0 || { echo "good fails for $w at $GOOD"; exit 1; }
  git checkout -q work/ledger-bisect-part-31
  bash "$ROOT/.agents/ledger-bisect-2026-11-11/run_one_bisect.sh" "$w" "$id"
  sha=$(tr -d ' \n' <"$ROOT/.agents/ledger-bisect-2026-11-11/bisect-c${id}-first-bad.txt")
  git show "$sha" --stat | head -20 >"$ROOT/.agents/ledger-bisect-2026-11-11/mechanism-c${id}.txt"
  python3 "$ROOT/.agents/ledger-bisect-2026-11-11/write_part31.py"
  git add ".agents/ledger-bisect-2026-11-11/"
  git commit -m "repo: ledger bisect part 31 cluster ${id} snapshot"
  git push -u origin work/ledger-bisect-part-31
done
