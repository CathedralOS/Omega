#!/usr/bin/env python3
"""OMGRFN23 R5: independent CKIR20 result and exit observation."""

import sys

from omgrfn23_ckir import IR20, meaning_decode
from omgrfn23_frame import require, split
from omgrfn23_owner import run


def check() -> None:
    frame = split(sys.stdin.buffer.read())
    require(IR20.interpret(meaning_decode(frame.ckir)) == 70,
            "CKIR20 exact result 70")
    require(frame.result == 70 and frame.exit_code == 70,
            "frame result/exit observation 70")


if __name__ == "__main__":
    run("R5 CKIR20 result", check)
