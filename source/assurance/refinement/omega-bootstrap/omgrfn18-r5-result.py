#!/usr/bin/env python3
import sys
from omgrfn18_ckir import decode, interpret
from omgrfn18_frame import require, split
from omgrfn18_owner import run


def check() -> None:
    frame = split(sys.stdin.buffer.read())
    result = interpret(decode(frame.ckir))
    require(result is not None and result == frame.result,
            "exact CKIR16 u64-Less result")


if __name__ == "__main__":
    run("R5-result", check)
