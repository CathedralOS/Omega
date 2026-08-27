#!/usr/bin/env python3
"""OMGRFN23 R4: artifact-free selected source result."""

import sys

from omgrfn23_frame import require, split
from omgrfn23_owner import run
from omgrfn23_source import source_result


def check() -> None:
    frame = split(sys.stdin.buffer.read())
    require(source_result(frame.omgcomp, frame.witness) == frame.result == 70,
            "artifact-free source result 70")


if __name__ == "__main__":
    run("R4 source result", check)
