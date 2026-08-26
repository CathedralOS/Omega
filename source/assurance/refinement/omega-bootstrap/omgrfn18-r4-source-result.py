#!/usr/bin/env python3
import sys
from omgrfn18_frame import require, split
from omgrfn18_owner import run
from omgrfn18_source import parse_selected_source


def check() -> None:
    frame = split(sys.stdin.buffer.read())
    require(parse_selected_source(frame.omgcomp).result == frame.result,
            "exact source-only u64-Less result")


if __name__ == "__main__":
    run("R4-source-result", check)
