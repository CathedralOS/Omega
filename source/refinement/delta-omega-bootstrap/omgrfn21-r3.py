#!/usr/bin/env python3
import sys

from omgrfn21_ckir import producer_decode
from omgrfn21_frame import split
from omgrfn21_owner import run

if __name__ == "__main__":
    run("R3", lambda: producer_decode(split(sys.stdin.buffer.read()).ckir))
