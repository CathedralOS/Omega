#!/usr/bin/env python3
"""Uniform quiet CLI harness for OMGRFN17 responsibility entrypoints."""

from __future__ import annotations

import sys

from omgrfn17_ckir import V5
from omgrfn17_frame import RefinementError, RefinementResourceError


def run(label: str, check) -> None:
    try:
        check()
    except (RefinementResourceError, V5.Ckir5ResourceError) as error:
        print(f"OMGRFN17 {label}: {error}", file=sys.stderr)
        raise SystemExit(252)
    except (RefinementError, OSError, ValueError) as error:
        print(f"OMGRFN17 {label}: {error}", file=sys.stderr)
        raise SystemExit(251)
