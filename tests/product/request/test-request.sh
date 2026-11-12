#!/bin/sh
# The request gate: builds the product compiler root, source/omega/application/ocreq, and runs
# the OCREQ request vectors against it. See ../gate.sh for the review, build
# and run protocol.
here=$(dirname -- "$0")
exec sh "$here/../gate.sh" request "$here/../../../source/omega/application/ocreq" "$here/test_request.py"
