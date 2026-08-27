#!/usr/bin/env python3
import sys

from omgrfn21_ckir import decode, interpret
from omgrfn21_frame import require, split
from omgrfn21_owner import run


def check() -> None:
    frame = split(sys.stdin.buffer.read())
    result = interpret(decode(frame.ckir))
    require(result is not None and result == frame.result == 70,
            "exact CKIR18 fixed-buffer result")


if __name__ == "__main__":
    run("R5-result", check)
