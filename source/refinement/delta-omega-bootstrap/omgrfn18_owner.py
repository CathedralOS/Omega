#!/usr/bin/env python3
"""Uniform quiet CLI harness for OMGRFN18 responsibility entrypoints."""

from __future__ import annotations

import sys

from omgrfn18_ckir import V5
from omgrfn18_frame import RefinementError, RefinementResourceError


def run(label: str, check) -> None:
    try:
        check()
    except (RefinementResourceError, V5.Ckir5ResourceError) as error:
        print(f"OMGRFN18 {label}: {error}", file=sys.stderr)
        raise SystemExit(252)
    except (RefinementError, OSError, ValueError) as error:
        print(f"OMGRFN18 {label}: {error}", file=sys.stderr)
        raise SystemExit(251)
