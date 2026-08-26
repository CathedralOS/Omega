#!/usr/bin/env python3
import sys
from omgrfn17_ckir import decode
from omgrfn17_frame import split
from omgrfn17_owner import run

if __name__ == "__main__": run("R5-structure", lambda: decode(split(sys.stdin.buffer.read()).ckir))
