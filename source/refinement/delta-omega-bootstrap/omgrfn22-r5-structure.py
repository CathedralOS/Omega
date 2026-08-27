#!/usr/bin/env python3
"""OMGRFN22 R5: frozen independent CKIR19 meaning structure."""

from __future__ import annotations

import sys

from omgrfn22_ckir import meaning_decode
from omgrfn22_frame import split
from omgrfn22_owner import run


def check() -> None:
    meaning_decode(split(sys.stdin.buffer.read()).ckir)


if __name__ == "__main__":
    run("R5 CKIR19 meaning structure", check)
