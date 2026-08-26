#!/usr/bin/env python3
import sys
from omgrfn17_elf import reconstruct
from omgrfn17_frame import require, split
from omgrfn17_owner import run

def check():
    frame = split(sys.stdin.buffer.read())
    require(reconstruct(frame.ckir) == frame.elf, "exact conservative ELF bytes and EOF")

if __name__ == "__main__": run("R5-ELF", check)
