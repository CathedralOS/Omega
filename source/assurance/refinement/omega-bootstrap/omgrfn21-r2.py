#!/usr/bin/env python3
import sys

from omgrfn21_frame import split
from omgrfn21_owner import run
from omgrfn21_source import check_witness_relation


def check() -> None:
    frame = split(sys.stdin.buffer.read())
    check_witness_relation(frame.omgcomp, frame.witness)


if __name__ == "__main__":
    run("R2", check)
