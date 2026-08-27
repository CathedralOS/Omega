#!/usr/bin/env python3
"""OMGRFN22 R3: complete producer-facing CKIR19 structure."""

from __future__ import annotations

import sys

from omgrfn22_ckir import producer_decode
from omgrfn22_frame import split
from omgrfn22_owner import run


def check() -> None:
    producer_decode(split(sys.stdin.buffer.read()).ckir)


if __name__ == "__main__":
    run("R3 CKIR19 structure", check)
