#!/usr/bin/env python3
import sys
from omgrfn17_ckir import IR15, decode
from omgrfn17_frame import require, split
from omgrfn17_owner import run

def check():
    frame = split(sys.stdin.buffer.read())
    actual = IR15.interpret(decode(frame.ckir))
    require(actual is not None and actual == frame.result, "exact CKIR15 result")

if __name__ == "__main__": run("R5-result", check)
