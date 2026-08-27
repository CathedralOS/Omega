#!/bin/sh
set -eu
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
exec python3 -B "$HERE/omgrfn17_gate.py" producer
