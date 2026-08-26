#!/usr/bin/env python3
import sys
from omgrfn17_frame import check_r1, split
from omgrfn17_owner import run

if __name__ == "__main__": run("R1", lambda: check_r1(split(sys.stdin.buffer.read())))
