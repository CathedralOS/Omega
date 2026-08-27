#!/usr/bin/env python3
"""OMGRFN22 R5: independent exact conservative artifact bytes."""

from __future__ import annotations

import sys

from omgrfn22_elf import reconstruct
from omgrfn22_frame import require, split
from omgrfn22_owner import run


def check() -> None:
    frame = split(sys.stdin.buffer.read())
    require(reconstruct(frame.ckir) == frame.elf,
            "exact conservative CKIR19 ELF bytes and EOF")


if __name__ == "__main__":
    run("R5-ELF", check)
