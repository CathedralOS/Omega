#!/usr/bin/env python3
"""R5: independent bounded abstract checked-adapter event observations."""

import sys

from omgrfn20_ckir import decode, invoke
from omgrfn20_frame import RefinementResourceError
from omgrfn20_frame import require, split
from omgrfn20_owner import run


def check() -> None:
    module = decode(split(sys.stdin.buffer.read()).ckir)
    observations = (
        ("write", b"", ()),
        ("write", bytes((70,)), (70,)),
        ("write_line", bytes((70, 71)), (70, 71, 10)),
        ("write_line", b"", (10,)),
    )
    for adapter, data, expected in observations:
        require(invoke(module, adapter, data) == expected,
                f"abstract {adapter} event trace")
    try:
        invoke(module, "write", b"F", trace_limit=0)
    except RefinementResourceError:
        pass
    else:
        require(False, "event-trace exhaustion must select 252")
    try:
        invoke(module, "write", b"", step_limit=0)
    except RefinementResourceError:
        pass
    else:
        require(False, "dynamic-step exhaustion must select 252")


if __name__ == "__main__":
    run("R5", check)
