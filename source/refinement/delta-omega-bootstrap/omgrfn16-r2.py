#!/usr/bin/env python3
"""OMGRFN16 R2 source reconstruction and OMGRSW7 relation owner."""

from __future__ import annotations

import sys

from omgrfn16_frame import RefinementError, RefinementResourceError, require, split
from omgrfn16_source import check_witness_relation, selected_run, source_contents


def main() -> None:
    frame = split(sys.stdin.buffer.read())
    sources = source_contents(frame.omgcomp)
    candidates = [source for source in sources if b"::run" in source]
    require(len(candidates) == 1, "unique selected source unit")
    program = selected_run(candidates[0])
    check_witness_relation(frame.omgcomp, frame.witness, program)


if __name__ == "__main__":
    try:
        main()
    except RefinementResourceError as error:
        print(f"OMGRFN16 R2: {error}", file=sys.stderr)
        raise SystemExit(252)
    except (RefinementError, OSError) as error:
        print(f"OMGRFN16 R2: {error}", file=sys.stderr)
        raise SystemExit(251)
