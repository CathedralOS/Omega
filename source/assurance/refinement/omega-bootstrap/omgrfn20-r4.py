#!/usr/bin/env python3
"""R4: exact OMGRSW9 source identities to CKIR17 operation join."""

import sys

from omgrfn19_witness import decode as decode_witness, source_slice
from omgrfn20_ckir import decode as decode_ckir, reference
from omgrfn20_frame import require, split
from omgrfn20_owner import run
from omgrfn20_source import check_join, decode_sources


def check() -> None:
    frame = split(sys.stdin.buffer.read())
    witness = decode_witness(frame.witness)
    envelope, sources = decode_sources(frame)
    check_join(envelope, witness)
    module = decode_ckir(frame.ckir)
    tables = module.tables
    operations = tables["operations"]
    operands = tuple(row[0] for row in tables["operands"])

    service = tables["services"][0]
    require(service == (0, witness.selected_trait, witness.selected_provider,
                        witness.selected_plan, witness.target, 0),
            "OMGRSW9 selected plan to opaque service join")
    target = tables["boundary_targets"][0]
    plan_row = witness.tables["plan_rows"][4]
    candidate = witness.tables["candidates"][4]
    require(target == (0, 0, plan_row[3], plan_row[0], plan_row[4],
                       candidate[3], 4, reference.NO_ID, plan_row[5]),
            "write_byte requirement/plan/candidate identity join")
    require(target[2] == 4 and target[4] == 4,
            "requirement and candidate IDs remain distinct fields")

    helper = witness.tables["helpers"][0]
    require(tables["rankings"][0] == (0, 0, helper[7], helper[8], 1),
            "source SliceLength to strict CKIR ranking")
    require(tables["machine_reaches"] == tuple((index, index, helper[9])
                                                for index in range(3)),
            "source Console reach to all executable machines")

    # Adapter source rows 0/1 map in order to static machines 1/2. Each exact
    # helper call becomes one receiverless call with an authored false/true.
    calls = [row for row in operations if row[3] == 28]
    require(tuple((row[1], row[10], row[11]) for row in calls)
            == ((1, 0, 0), (2, 0, 0)), "adapter receiverless-call mapping")
    for index, operation in enumerate(calls):
        args = operands[operation[8]:operation[8] + operation[9]]
        require(tuple(module.value_types[value] for value in args) == (5, 3, 1),
                "receiverless explicit service/view/bool arguments")
        bool_value = args[2]
        producer_id = module.value_operations[bool_value]
        producer = operations[producer_id]
        require(producer[3] == 1 and producer[10] == index,
                "authored false/true adapter argument")

    events = [row for row in operations if row[3] == 29]
    widens = [row for row in operations if row[3] == 30]
    require(tuple(row[0] for row in events) == (4, 10) and len(widens) == 2,
            "two ordered explicit-cast event sites")
    for call_row, event in zip(witness.tables["requirement_calls"][:2], events):
        require(call_row[4] == target[2] == 4 and event[10:] == (0, 0),
                "requirement call to boundary target")
        raw_call = source_slice(sources, call_row[1], call_row[5], call_row[6])
        require(raw_call.startswith(b"write_byte(") and b"output as i32" in raw_call,
                "authored explicit cast source node")
        args = operands[event[8]:event[8] + event[9]]
        require(tuple(module.value_types[value] for value in args) == (5, 4),
                "event service and full-i32 signature")
        widen_id = module.value_operations[args[1]]
        widen = operations[widen_id]
        require(widen[3] == 30 and widen[10:] == (0, 0),
                "event byte comes from U8ToI32")
        source_value = operands[widen[8]]
        require(module.value_types[source_value] == 2,
                "explicit widen consumes exact u8")


if __name__ == "__main__":
    run("R4", check)
