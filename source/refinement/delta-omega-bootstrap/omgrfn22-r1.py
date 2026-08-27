#!/usr/bin/env python3
"""OMGRFN22 R1: frame and complete OMGCOMP1 custody."""

from __future__ import annotations

import sys

from omgrfn22_frame import check_r1, split
from omgrfn22_owner import run


def check() -> None:
    check_r1(split(sys.stdin.buffer.read()))


if __name__ == "__main__":
    run("R1 frame/source custody", check)
