#!/bin/sh
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$here/../../.." && pwd)
# Exercise the actual construction application and its production library.
# No copied emitter or diagnostic-only orchestration supplies compiler meaning.
exec sh "$here/../gate.sh" alpha \
    "$root/source/omega/application/construction" "$here/test.py" "$@" </dev/null
