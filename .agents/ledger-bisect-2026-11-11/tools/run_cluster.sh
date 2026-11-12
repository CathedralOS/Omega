#!/bin/bash
# Run one PIN cluster bisect: setup baseline, find good, bisect, write JSON.
set -euo pipefail
CLUSTER="${1:?0-3}"
ROOT="${OMEGA_REPO:-/agent/repos/omega}"
ART="$ROOT/.agents/ledger-bisect-2026-11-11"
PIN=51e8c45c793007f211d0d9baee7de16767172444
export OMEGA_REPO="$ROOT" LEDGER_BISECT_PIN="$PIN"

mkdir -p /tmp/ledger-bisect-tools
# Snapshot tools before any historical checkout (untracked .agents paths are not stable).
if [[ -d "$ART/tools/.bisect_tool_snapshot" ]]; then
  cp -a "$ART/tools/.bisect_tool_snapshot/"* /tmp/ledger-bisect-tools/
else
  mkdir -p "$ART/tools/.bisect_tool_snapshot"
  for f in bisect_helpers.py bisect_test.py setup_pin_baseline.py find_good_binary.py \
    bisect_one.sh bisect_witness.sh run_witness_at_rev.sh is_ledger_only_commit.sh setup_pin_baseline.sh; do
    cp -a "$ART/tools/$f" "$ART/tools/.bisect_tool_snapshot/"
  done
  cp -a "$ART/tools/.bisect_tool_snapshot/"* /tmp/ledger-bisect-tools/
fi
chmod +x /tmp/ledger-bisect-tools/*.sh
export BISECT_TOOLS=/tmp/ledger-bisect-tools

case "$CLUSTER" in
  0)
    WITNESS=pass/operators/runtime_named_unit_statement_effects_exit
    OUT="$ART/bisect_c0.json"
    ;;
  1)
    WITNESS=pass/providers/runtime_foreign_allocation_across_component_exit
    OUT="$ART/bisect_c1.json"
    ;;
  2)
    WITNESS=pass/facts/opaque_independent_closed_carrier_through_other_machine_exit
    OUT="$ART/bisect_opaque_independent_closed_carrier_through_other_machine_exit.json"
    ;;
  3)
    WITNESS=pass/providers/module_named_operator_result_domain_satisfaction
    OUT="$ART/bisect_c3_ledger_proxy.json"
    ;;
  *) echo "bad cluster"; exit 1 ;;
esac

cd "$ROOT"
git checkout -f work/ledger-bisect-part-11
python3 /tmp/ledger-bisect-tools/setup_pin_baseline.py \
  "$ART/pin_slice11_observed.txt" "$PIN" "$WITNESS"

if [[ "$CLUSTER" == "2" ]]; then
  GOOD=$(python3 /tmp/ledger-bisect-tools/find_good_binary.py "$WITNESS" "$PIN")
  export OMEGA_REPO BISECT_TOOLS
  {
    echo "witness=$WITNESS good=$GOOD pin=$PIN"
    for rev in "$GOOD" "$PIN"; do
      git checkout -f "$rev" >/dev/null 2>&1
      for n in 1 2 3; do
        set +e
        rc=$(/tmp/ledger-bisect-tools/run_witness_at_rev.sh "$WITNESS")
        set -e
        echo "rev=$rev run=$n rc=$rc"
      done
    done
  } | tee "$ART/flaky_opaque.txt"
  git checkout -f work/ledger-bisect-part-11
  if ! python3 - "$ART/flaky_opaque.txt" <<'PY'
import sys
vals = [int(l.split("rc=")[1]) for l in open(sys.argv[1]) if "rc=" in l]
if len(vals) < 6:
    raise SystemExit(1)
raise SystemExit(0 if set(vals[:3]) == {0} and set(vals[3:]) == {1} else 1)
PY
  then
    python3 -c "import json; json.dump({'witness':'$WITNESS','status':'propagated(flaky)','log':'flaky_opaque.txt','good':'$GOOD','method':'fixed-pin-corpus'}, open('$OUT','w'), indent=2)"
    exit 0
  fi
  export BISECT_GOOD="$GOOD"
fi

/tmp/ledger-bisect-tools/bisect_one.sh "$WITNESS" "$OUT"
git checkout -f work/ledger-bisect-part-11
