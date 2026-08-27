#!/usr/bin/env python3
import sys

from omgrfn21_frame import require, split
from omgrfn21_owner import run
from omgrfn21_source import parse_selected_source


def check() -> None:
    frame = split(sys.stdin.buffer.read())
    require(parse_selected_source(frame.omgcomp, frame.witness).result == frame.result,
            "exact source-only fixed-buffer result")


if __name__ == "__main__":
    run("R4-source-result", check)
