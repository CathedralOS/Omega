#!/usr/bin/env python3
"""Load the independent CKIR17 decoder without importing producer conclusions."""

import sys
from pathlib import Path

from omgrfn20_frame import RefinementError, RefinementResourceError

HERE = Path(__file__).resolve().parent
GATES = HERE.parents[2] / "source/on-ramp/omega-bootstrap/gates"
sys.path.insert(0, str(GATES))
import checked_ir_v17_reference as reference  # noqa: E402


def decode(raw: bytes):
    try:
        return reference.decode(raw)
    except reference.Ckir17ResourceError as error:
        raise RefinementResourceError(f"CKIR17: {error}") from error
    except reference.Ckir17Error as error:
        raise RefinementError(f"CKIR17: {error}") from error


def invoke(module, adapter, data: bytes, **limits):
    try:
        return reference.invoke(module, adapter, data, **limits)
    except reference.Ckir17ResourceError as error:
        raise RefinementResourceError(f"CKIR17 execution: {error}") from error
    except reference.Ckir17Error as error:
        raise RefinementError(f"CKIR17 execution: {error}") from error
