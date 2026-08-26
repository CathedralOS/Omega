#!/usr/bin/env sh
set -eu
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
python3 -B "$HERE/omgrfn20_owner_test.py"
"$HERE/omgrfn20-beta-join.sh"
