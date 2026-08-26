#!/usr/bin/env sh
# Platform-neutral structural OMGRFN19 reference gate.
set -eu

HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
python3 -B "$HERE/omgrfn19_owner_test.py"
"$HERE/omgrfn19-beta-join.sh"
echo "OMGRFN19 reference integration: modular Python owners and persisted-Beta projections PASS"
