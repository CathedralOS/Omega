#!/usr/bin/env python3
"""OMGRFN23 R2: independent exact source-to-OMGRSWC12 relation."""

import sys

from omgrfn23_frame import split
from omgrfn23_owner import run
from omgrfn23_source import check_witness_relation


def check() -> None:
    frame = split(sys.stdin.buffer.read())
    check_witness_relation(frame.omgcomp, frame.witness)


if __name__ == "__main__":
    run("R2 source/witness", check)
