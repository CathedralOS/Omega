#!/usr/bin/env python3
"""OMGRFN23 R4: exact authored source/witness-to-CKIR20 lowering."""

import sys

from omgrfn23_frame import split
from omgrfn23_lowering import check_lowering
from omgrfn23_owner import run


def check() -> None:
    frame = split(sys.stdin.buffer.read())
    check_lowering(frame.omgcomp, frame.witness, frame.ckir)


if __name__ == "__main__":
    run("R4 lowering", check)
