#!/bin/sh
# The typing gate: see ../../product/gate.sh for the review, build and run
# protocol. The harness package is this directory.
here=$(dirname -- "$0")
exec sh "$here/../../product/gate.sh" typing "$here" "$here/test_typing.py"
