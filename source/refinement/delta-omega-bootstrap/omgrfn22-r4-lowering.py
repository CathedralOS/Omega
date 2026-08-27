#!/usr/bin/env python3
"""OMGRFN22 R4: exact authored source/witness-to-CKIR19 lowering."""

from __future__ import annotations

import sys

from omgrfn22_frame import split
from omgrfn22_lowering import check_lowering
from omgrfn22_owner import run


def check() -> None:
    frame = split(sys.stdin.buffer.read())
    check_lowering(frame.omgcomp, frame.witness, frame.ckir)


if __name__ == "__main__":
    run("R4 lowering", check)
