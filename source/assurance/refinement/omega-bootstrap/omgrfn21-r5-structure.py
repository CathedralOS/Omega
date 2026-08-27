#!/usr/bin/env python3
import sys

from omgrfn21_ckir import decode
from omgrfn21_frame import split
from omgrfn21_owner import run

if __name__ == "__main__":
    run("R5-structure", lambda: decode(split(sys.stdin.buffer.read()).ckir))
