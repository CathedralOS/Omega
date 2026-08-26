#!/usr/bin/env python3
"""OMGRFN16 R5 CKIR execution, trap, and publication owner."""

from __future__ import annotations

import sys

from omgrfn16_ckir import V5, check_arithmetic_closure, decode
from omgrfn16_frame import RefinementError, RefinementResourceError, require, split


TRAP_MARKERS = ("runtime add range", "runtime subtract range", "runtime multiply range")


def main() -> None:
    frame = split(sys.stdin.buffer.read())
    module = decode(frame.ckir)
    check_arithmetic_closure(module)
    try:
        actual = V5.interpret(module)
    except V5.Ckir5Error as error:
        require(frame.traps, "unexpected CKIR runtime trap")
        require(any(marker in str(error) for marker in TRAP_MARKERS),
                "selected arithmetic trap family")
        return
    require(not frame.traps, "expected CKIR runtime trap")
    require(actual is not None and actual == frame.result, "exact CKIR result")


if __name__ == "__main__":
    try:
        main()
    except (RefinementResourceError, V5.Ckir5ResourceError) as error:
        print(f"OMGRFN16 R5-result: {error}", file=sys.stderr)
        raise SystemExit(252)
    except (RefinementError, OSError) as error:
        print(f"OMGRFN16 R5-result: {error}", file=sys.stderr)
        raise SystemExit(251)
