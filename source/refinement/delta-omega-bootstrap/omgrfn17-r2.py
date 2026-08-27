#!/usr/bin/env python3
import sys
from omgrfn17_frame import split
from omgrfn17_owner import run
from omgrfn17_source import check_witness_relation

def check():
    frame = split(sys.stdin.buffer.read())
    check_witness_relation(frame.omgcomp, frame.witness)

if __name__ == "__main__": run("R2", check)
