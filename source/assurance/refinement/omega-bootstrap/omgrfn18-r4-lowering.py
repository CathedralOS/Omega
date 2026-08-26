#!/usr/bin/env python3
import sys
from omgrfn18_ckir import check_lowering, decode
from omgrfn18_frame import split
from omgrfn18_owner import run
from omgrfn18_source import parse_selected_source


def check() -> None:
    frame = split(sys.stdin.buffer.read())
    check_lowering(decode(frame.ckir), parse_selected_source(frame.omgcomp))


if __name__ == "__main__":
    run("R4-lowering", check)
