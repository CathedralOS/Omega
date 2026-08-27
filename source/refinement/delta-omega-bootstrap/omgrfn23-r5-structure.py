#!/usr/bin/env python3
"""OMGRFN23 R5: frozen independent CKIR20 meaning structure."""

import sys

from omgrfn23_ckir import meaning_decode
from omgrfn23_frame import split
from omgrfn23_owner import run


def check() -> None:
    meaning_decode(split(sys.stdin.buffer.read()).ckir)


if __name__ == "__main__":
    run("R5 CKIR20 meaning structure", check)
