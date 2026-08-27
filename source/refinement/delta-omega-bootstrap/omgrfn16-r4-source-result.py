#!/usr/bin/env python3
"""OMGRFN16 R4 source-only result and first-trap owner."""

from __future__ import annotations

import sys

from omgrfn16_frame import RefinementError, RefinementResourceError, require, split
from omgrfn16_source import SourceTrap, execute, selected_run, source_contents


def main() -> None:
    frame = split(sys.stdin.buffer.read())
    sources = source_contents(frame.omgcomp)
    candidates = [source for source in sources if b"::run" in source]
    require(len(candidates) == 1, "unique selected source unit")
    program = selected_run(candidates[0])
    try:
        actual = execute(program)
    except SourceTrap:
        require(frame.traps, "unexpected source arithmetic trap")
        return
    require(not frame.traps, "expected source arithmetic trap")
    require(actual == frame.result, "exact source mathematical result")


if __name__ == "__main__":
    try:
        main()
    except RefinementResourceError as error:
        print(f"OMGRFN16 R4-source-result: {error}", file=sys.stderr)
        raise SystemExit(252)
    except (RefinementError, OSError) as error:
        print(f"OMGRFN16 R4-source-result: {error}", file=sys.stderr)
        raise SystemExit(251)
