#!/usr/bin/env python3
"""OMGRFN16 R5 exact conservative ELF reconstruction owner."""

from __future__ import annotations

import sys

from omgrfn16_ckir import V5
from omgrfn16_elf_reference import reconstruct
from omgrfn16_frame import RefinementError, RefinementResourceError, require, split


def main() -> None:
    frame = split(sys.stdin.buffer.read())
    require(reconstruct(frame.ckir) == frame.elf, "exact ELF bytes and EOF")


if __name__ == "__main__":
    try:
        main()
    except (RefinementResourceError, V5.Ckir5ResourceError) as error:
        print(f"OMGRFN16 R5-ELF: {error}", file=sys.stderr)
        raise SystemExit(252)
    except (RefinementError, OSError) as error:
        print(f"OMGRFN16 R5-ELF: {error}", file=sys.stderr)
        raise SystemExit(251)
