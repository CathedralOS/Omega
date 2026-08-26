#!/usr/bin/env python3
import sys
from omgrfn17_frame import require, split
from omgrfn17_owner import run
from omgrfn17_source import parse_selected_source

def check():
    frame = split(sys.stdin.buffer.read())
    require(parse_selected_source(frame.omgcomp).result == frame.result,
            "exact source-only recurrent result")

if __name__ == "__main__": run("R4-source-result", check)
