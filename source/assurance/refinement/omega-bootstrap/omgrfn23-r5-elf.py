#!/usr/bin/env python3
"""OMGRFN23 R5: independent exact conservative artifact bytes."""

import sys

from omgrfn23_elf import reconstruct
from omgrfn23_frame import require, split
from omgrfn23_owner import run


def check() -> None:
    frame = split(sys.stdin.buffer.read())
    require(reconstruct(frame.ckir) == frame.elf,
            "exact conservative CKIR20 ELF bytes and EOF")


if __name__ == "__main__":
    run("R5-ELF", check)
