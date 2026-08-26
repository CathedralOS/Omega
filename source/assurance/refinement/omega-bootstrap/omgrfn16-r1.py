#!/usr/bin/env python3
"""OMGRFN16 R1 identity, custody, extents, and result/trap framing owner."""

from __future__ import annotations

import sys

from omgrfn16_frame import (
    RefinementError, RefinementResourceError, check_r1, split,
)


def main() -> None:
    check_r1(split(sys.stdin.buffer.read()))


if __name__ == "__main__":
    try:
        main()
    except RefinementResourceError as error:
        print(f"OMGRFN16 R1: {error}", file=sys.stderr)
        raise SystemExit(252)
    except (RefinementError, OSError) as error:
        print(f"OMGRFN16 R1: {error}", file=sys.stderr)
        raise SystemExit(251)
