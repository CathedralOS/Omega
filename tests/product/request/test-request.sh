#!/bin/sh
# The request gate: builds the product compiler root, source/omega, and runs
# the OCREQ request vectors against it. See ../gate.sh for the review, build
# and run protocol.
here=$(dirname -- "$0")
exec sh "$here/../gate.sh" request "$here/../../../source/omega" "$here/test_request.py"
