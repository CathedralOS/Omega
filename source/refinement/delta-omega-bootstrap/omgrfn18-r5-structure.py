#!/usr/bin/env python3
import sys
from omgrfn18_ckir import decode
from omgrfn18_frame import split
from omgrfn18_owner import run

if __name__ == "__main__":
    run("R5-structure", lambda: decode(split(sys.stdin.buffer.read()).ckir))
