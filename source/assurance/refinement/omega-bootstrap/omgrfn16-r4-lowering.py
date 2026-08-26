#!/usr/bin/env python3
"""OMGRFN16 R4 source-token to CKIR14 postorder lowering owner."""

from __future__ import annotations

import sys

from omgrfn16_ckir import (
    V5, check_context_join, check_expression_join, check_view_join, decode,
)
from omgrfn16_frame import RefinementError, RefinementResourceError, require, split
from omgrfn16_source import selected_run, source_contents, witness_leaf_names


def main() -> None:
    frame = split(sys.stdin.buffer.read())
    sources = source_contents(frame.omgcomp)
    candidates = [source for source in sources if b"::run" in source]
    require(len(candidates) == 1, "unique selected source unit")
    program = selected_run(candidates[0])
    module = decode(frame.ckir)
    names = witness_leaf_names(frame.omgcomp, frame.witness)
    check_expression_join(module, program, names)
    check_context_join(module, program, names)
    check_view_join(module, program)


if __name__ == "__main__":
    try:
        main()
    except (RefinementResourceError, V5.Ckir5ResourceError) as error:
        print(f"OMGRFN16 R4-lowering: {error}", file=sys.stderr)
        raise SystemExit(252)
    except (RefinementError, OSError) as error:
        print(f"OMGRFN16 R4-lowering: {error}", file=sys.stderr)
        raise SystemExit(251)
