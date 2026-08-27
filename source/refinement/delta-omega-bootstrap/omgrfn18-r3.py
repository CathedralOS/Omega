#!/usr/bin/env python3
import sys
from omgrfn18_ckir import producer_decode
from omgrfn18_frame import split
from omgrfn18_owner import run

if __name__ == "__main__":
    run("R3", lambda: producer_decode(split(sys.stdin.buffer.read()).ckir))
