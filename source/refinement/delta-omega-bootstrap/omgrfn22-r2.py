#!/usr/bin/env python3
"""OMGRFN22 R2: independent exact source-to-OMGRSWB relation."""

from __future__ import annotations

import sys

from omgrfn22_frame import split
from omgrfn22_owner import run
from omgrfn22_source import check_witness_relation


def check() -> None:
    frame = split(sys.stdin.buffer.read())
    check_witness_relation(frame.omgcomp, frame.witness)


if __name__ == "__main__":
    run("R2 source/witness", check)
