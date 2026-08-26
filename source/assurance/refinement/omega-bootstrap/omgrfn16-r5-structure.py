#!/usr/bin/env python3
"""OMGRFN16 R5 frozen-reference CKIR14 structure owner."""

from __future__ import annotations

import sys

from omgrfn16_ckir import V5, check_arithmetic_closure, decode
from omgrfn16_frame import RefinementError, RefinementResourceError, split


def main() -> None:
    frame = split(sys.stdin.buffer.read())
    check_arithmetic_closure(decode(frame.ckir))


if __name__ == "__main__":
    try:
        main()
    except (RefinementResourceError, V5.Ckir5ResourceError) as error:
        print(f"OMGRFN16 R5-structure: {error}", file=sys.stderr)
        raise SystemExit(252)
    except (RefinementError, OSError) as error:
        print(f"OMGRFN16 R5-structure: {error}", file=sys.stderr)
        raise SystemExit(251)
