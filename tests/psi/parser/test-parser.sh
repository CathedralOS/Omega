#!/bin/sh
# The parser gate: see ../../product/gate.sh for the review, build and run
# protocol. The harness package is this directory.
here=$(dirname -- "$0")
exec sh "$here/../../product/gate.sh" parser "$here" "$here/test_parser.py"
