#!/bin/sh
# The checking gate: see ../../product/gate.sh for the review, build and run
# protocol. The harness package is this directory.
here=$(dirname -- "$0")
exec sh "$here/../../product/gate.sh" checking "$here" "$here/test_checking.py"
