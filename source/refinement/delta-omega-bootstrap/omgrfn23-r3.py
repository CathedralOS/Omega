#!/usr/bin/env python3
"""OMGRFN23 R3: complete producer-facing CKIR20 structure."""

import sys

from omgrfn23_ckir import producer_decode
from omgrfn23_frame import split
from omgrfn23_owner import run


def check() -> None:
    producer_decode(split(sys.stdin.buffer.read()).ckir)


if __name__ == "__main__":
    run("R3 CKIR20 structure", check)
