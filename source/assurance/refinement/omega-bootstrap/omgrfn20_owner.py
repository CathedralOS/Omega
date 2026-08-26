#!/usr/bin/env python3
"""Quiet status/publication harness for OMGRFN20 owners."""

import struct
import sys

from omgrfn20_frame import RefinementError, RefinementResourceError


def run(label: str, check) -> None:
    try:
        check()
    except RefinementResourceError as error:
        print(f"OMGRFN20 {label}: {error}", file=sys.stderr)
        raise SystemExit(252)
    except (RefinementError, OSError, ValueError, struct.error) as error:
        print(f"OMGRFN20 {label}: {error}", file=sys.stderr)
        raise SystemExit(251)
