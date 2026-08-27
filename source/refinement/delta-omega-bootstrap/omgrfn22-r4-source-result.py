#!/usr/bin/env python3
"""OMGRFN22 R4: artifact-free selected source result."""

from __future__ import annotations

import sys

from omgrfn22_frame import require, split
from omgrfn22_owner import run
from omgrfn22_source import source_result


def check() -> None:
    frame = split(sys.stdin.buffer.read())
    require(source_result(frame.omgcomp, frame.witness) == frame.result == 70,
            "artifact-free source result 70")


if __name__ == "__main__":
    run("R4 source result", check)
