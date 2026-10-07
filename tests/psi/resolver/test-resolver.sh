#!/bin/sh
# The resolver gate: see ../gate.sh for the review, build and run protocol.
exec sh "$(dirname -- "$0")/../gate.sh" resolver test_resolver.py
