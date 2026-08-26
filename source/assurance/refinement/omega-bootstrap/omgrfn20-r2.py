#!/usr/bin/env python3
"""R2: exact explicit-cast source and selected-plan custody."""

import sys

from omgrfn19_witness import decode, source_slice, span_word
from omgrfn20_frame import require, split
from omgrfn20_owner import run
from omgrfn20_source import check_join, decode_sources


def check() -> None:
    frame = split(sys.stdin.buffer.read())
    witness = decode(frame.witness)
    envelope, sources = decode_sources(frame)
    check_join(envelope, witness)

    require(witness.tables["plans"][0] == (0, 0, 0, 1, 0, 0, 6, 0, 1),
            "complete selected Console plan")
    require(witness.tables["plan_rows"][1] == (1, 0, 1, 1, 1, 1)
            and witness.tables["plan_rows"][4] == (4, 0, 4, 4, 4, 2),
            "checked write row and write_byte leaf row")

    provider = witness.tables["providers"][0]
    require(provider[:3] == (0, 0, 0) and provider[5] == 1,
            "nominal provider custody")
    span_word(sources, provider[1], provider[3], provider[4],
              b"ConsoleNativeProvider")

    helper = witness.tables["helpers"][0]
    require(helper[:2] == (0, 0) and helper[4:10] == (7, 5, 3, 1, 1, 0),
            "ranked helper identity")
    span_word(sources, helper[1], helper[2], helper[3], b"console_write_bytes")
    body = source_slice(sources, helper[1], helper[10], helper[11])
    require(body.count(b"bytes.len > 0") == 2
            and body.count(b"bytes[0]") == 2
            and body.count(b"bytes[1..]") == 2
            and body.count(b"output as i32") == 2
            and body.count(b"console.write_byte") == 2,
            "complete helper with two explicit i32 casts")
    require(b"terminates by bytes -> Slice::Length" in sources[0][:helper[10]]
            and b"reaches Console" in sources[0][:helper[10]],
            "ranking and reach source clauses")

    for adapter_id, name, requirement, ordinary_call, newline in (
        (0, b"write", 1, 0, b"false"),
        (1, b"write_line", 0, 1, b"true"),
    ):
        row = witness.tables["adapters"][adapter_id]
        require(row[:4] == (adapter_id, 0, 0, requirement)
                and row[6:10] == (7, 5, 0, ordinary_call) and row[12] == 1,
                "checked adapter identity")
        span_word(sources, row[1], row[4], row[5], name)
        adapter_body = source_slice(sources, row[1], row[10], row[11])
        require(adapter_body.count(b"console_write_bytes") == 1
                and newline in adapter_body, "adapter helper invocation")

    candidate = witness.tables["candidates"][1]
    require(candidate[:6] == (1, 1, 0, 0, 1, 0)
            and candidate[8:] == (2, 2, 0, 0, 0, 1),
            "selected checked write candidate")
    leaf = witness.tables["candidates"][4]
    require(leaf[:6] == (4, 2, 1, 0, 4, 2)
            and leaf[8:] == (5, 1, 0, 0, 1, 2),
            "write_byte intrinsic candidate remains structural")

    for call_id in (0, 1):
        call = witness.tables["requirement_calls"][call_id]
        require(call[:5] == (call_id, 0, 1, 0, 4)
                and call[7:] == (7, 1, 0, 0),
                "helper call retains requirement 4")
        raw = source_slice(sources, call[1], call[5], call[6])
        require(raw.startswith(b"write_byte(") and b"output as i32" in raw,
                "requirement call owns explicit cast span")


if __name__ == "__main__":
    run("R2", check)
