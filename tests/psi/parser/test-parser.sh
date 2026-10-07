#!/bin/sh
# The parser gate: see ../gate.sh for the review, build and run protocol.
exec sh "$(dirname -- "$0")/../gate.sh" parser test_parser.py
