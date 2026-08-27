#!/usr/bin/env python3
import sys

from omgrfn21_ckir import check_lowering, decode
from omgrfn21_frame import split
from omgrfn21_owner import run
from omgrfn21_source import check_witness_relation


def check() -> None:
    frame = split(sys.stdin.buffer.read())
    witness, source = check_witness_relation(frame.omgcomp, frame.witness)
    check_lowering(decode(frame.ckir), witness, source)


if __name__ == "__main__":
    run("R4-lowering", check)
