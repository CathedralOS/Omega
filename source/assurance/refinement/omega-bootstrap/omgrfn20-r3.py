#!/usr/bin/env python3
"""R3: complete CKIR17 structural, service, reach, and ranking custody."""

import sys

from omgrfn20_ckir import decode, reference
from omgrfn20_frame import require, split
from omgrfn20_owner import run


def check() -> None:
    module = decode(split(sys.stdin.buffer.read()).ckir)
    tables = module.tables
    require(tables["services"] == ((0, 0, 0, 0, 1, 0),),
            "one nonadmitting Console service row")
    require(tables["machine_reaches"] == ((0, 0, 0), (1, 1, 0), (2, 2, 0)),
            "complete machine reach closure")
    require(tables["rankings"] == ((0, 0, 1, 1, 1),),
            "strict SliceLength ranking")
    require(tables["boundary_targets"]
            == ((0, 0, 4, 4, 4, 0, 4, reference.NO_ID, 2),),
            "requirement-targeted abstract write_byte row")
    require(tables["machines"] == (
        (0, reference.NO_ID, 0, reference.FREE, 0, reference.NO_ID, 0, 3, 0, 7, 0),
        (1, 0, 0, reference.STATIC_ATTACHED, 0, reference.NO_ID, 3, 2, 7, 1, 7),
        (2, 0, 0, reference.STATIC_ATTACHED, 0, reference.NO_ID, 5, 2, 8, 1, 8),
    ), "free helper and two static-attached adapters")
    require(tuple(row[3] for row in tables["machine_params"])
            == (5, 3, 1, 5, 3, 5, 3), "machine service/view/bool signatures")

    opcodes = tuple(row[3] for row in tables["operations"])
    require({opcode: opcodes.count(opcode) for opcode in set(opcodes)}
            == {1: 3, 23: 2, 24: 2, 25: 2, 28: 2, 29: 2, 30: 2},
            "complete selected operation family")
    require(tuple(row[0] for row in tables["operations"] if row[3] == 29) == (4, 10),
            "ordered abstract boundary event sites")
    require(tuple(index for index, row in enumerate(tables["blocks"]) if row[3] == 1)
            == (1, 3), "two synthetic nonempty true blocks")
    require(len(module.value_types) == 32 and not tables["fields"],
            "dense values and no service-bearing places")


if __name__ == "__main__":
    run("R3", check)
