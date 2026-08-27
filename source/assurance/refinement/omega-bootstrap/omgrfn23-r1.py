#!/usr/bin/env python3
"""OMGRFN23 R1: frame and complete OMGCOMP1 custody."""

import sys

from omgrfn23_frame import check_r1, split
from omgrfn23_owner import run


def check() -> None:
    check_r1(split(sys.stdin.buffer.read()))


if __name__ == "__main__":
    run("R1 frame/source custody", check)
