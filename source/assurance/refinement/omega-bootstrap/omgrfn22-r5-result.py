#!/usr/bin/env python3
"""OMGRFN22 R5: independent CKIR19 result and exit observation."""

from __future__ import annotations

import sys

from omgrfn22_ckir import IR19, meaning_decode
from omgrfn22_frame import require, split
from omgrfn22_owner import run


def check() -> None:
    frame = split(sys.stdin.buffer.read())
    require(IR19.interpret(meaning_decode(frame.ckir)) == 70,
            "CKIR19 exact result 70")
    require(frame.result == 70 and frame.exit_code == 70,
            "frame result/exit observation 70")


if __name__ == "__main__":
    run("R5 CKIR19 result", check)
