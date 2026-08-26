#!/usr/bin/env python3
import sys

from omgrfn20_frame import check_r1, split
from omgrfn20_owner import run


def check() -> None:
    check_r1(split(sys.stdin.buffer.read()))


if __name__ == "__main__":
    run("R1", check)
