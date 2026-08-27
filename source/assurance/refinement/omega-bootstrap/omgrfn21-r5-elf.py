#!/usr/bin/env python3
import sys

from omgrfn21_elf import reconstruct
from omgrfn21_frame import require, split
from omgrfn21_owner import run


def check() -> None:
    frame = split(sys.stdin.buffer.read())
    require(reconstruct(frame.ckir) == frame.elf,
            "exact conservative CKIR18 ELF bytes and EOF")


if __name__ == "__main__":
    run("R5-ELF", check)
