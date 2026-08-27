#!/usr/bin/env python3
"""Uniform quiet CLI harness for OMGRFN23 responsibility owners."""

from __future__ import annotations

import sys

from omgrfn23_frame import RefinementError, RefinementResourceError


def run(label: str, check) -> None:
    try:
        check()
    except RefinementResourceError as error:
        print(f"OMGRFN23 {label}: {error}", file=sys.stderr)
        raise SystemExit(252)
    except (RefinementError, OSError, ValueError) as error:
        print(f"OMGRFN23 {label}: {error}", file=sys.stderr)
        raise SystemExit(251)
