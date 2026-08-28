#!/usr/bin/env sh
# Focused tests for the non-authoritative Delta assembly/executable custody join.
set -eu
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
python3 -B "$HERE/lower_rooted_artifact_custody_v1_test.py"
python3 -B "$HERE/realize_delta_artifact_v1_test.py"
python3 -B "$HERE/reconstruct_and_verify_installed_artifact_v1_test.py"
echo "Delta lower-rooted artifact custody V1 tests PASS"
