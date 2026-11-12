#!/bin/bash
# Write /tmp/ledger_bisect_part23_spec_<cluster_id>.json from pin observation.
set -euo pipefail
CLUSTER_ID="${1:?}"
WITNESS="${2:?}"
PIN_STATUS="${3:?}"
PIN_DIAG="${4:-}"
PIN_PREFIX="${5:-}"
EXPECTED="${6:?}"
python3 - "$CLUSTER_ID" "$WITNESS" "$PIN_STATUS" "$PIN_DIAG" "$PIN_PREFIX" "$EXPECTED" << 'PY'
import json, sys
cid, witness, pin_status, pin_diag, pin_prefix, expected = sys.argv[1:]
spec = {
    "cluster_id": cid,
    "witness": witness,
    "pin_status": pin_status,
    "expected_good_status": expected,
}
if pin_prefix:
    spec["pin_first_diagnostic_prefix"] = pin_prefix
elif pin_diag:
    spec["pin_first_diagnostic"] = pin_diag
path = f"/tmp/ledger_bisect_part23_spec_{cid}.json"
open(path, "w").write(json.dumps(spec, indent=2) + "\n")
print(path)
PY
