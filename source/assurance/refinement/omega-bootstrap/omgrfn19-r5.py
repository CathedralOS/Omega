#!/usr/bin/env python3
"""R5: complete selected plan and requirement-call separation."""

import sys

from omgrfn19_frame import require, split
from omgrfn19_owner import run
from omgrfn19_source import check_header_join, decode_sources
from omgrfn19_witness import decode, source_slice


CALL_REQUIREMENTS = (4, 4, 3, 4, 5, 5)
CALL_NAMES = (b"write_byte", b"write_byte", b"read_byte", b"write_byte",
              b"exit_process", b"exit_process")


def check() -> None:
    frame = split(sys.stdin.buffer.read())
    witness = decode(frame.witness)
    envelope, sources = decode_sources(frame)
    check_header_join(envelope, witness)

    plan = witness.tables["plans"][0]
    provider_source = witness.tables["providers"][0][1]
    provider_package = envelope.sources[provider_source].owner_package_id
    require(plan == (0, 0, 0, 1, 0, 0, 6,
                     provider_package, 1),
            "one complete selected provider plan")
    for index, row in enumerate(witness.tables["plan_rows"]):
        expected_binding = 1 if index < 2 else 2
        require(row == (index, 0, index, index, index, expected_binding),
                f"complete plan row {index}")

    calls = witness.tables["requirement_calls"]
    require(tuple(row[4] for row in calls) == CALL_REQUIREMENTS,
            "requirement calls retain requirements rather than candidate IDs")
    for index, (row, name) in enumerate(zip(calls, CALL_NAMES)):
        (identifier, source, caller_kind, caller_id, requirement, call_at,
         call_len, receiver_type, argument_count, result_type, flags) = row
        expected_source = 0 if index < 2 else witness.root_source
        expected_kind = 1 if index < 2 else 2
        expected_arguments = 0 if requirement == 3 else 1
        expected_result = 4 if requirement == 3 else 0
        require((identifier, source, caller_kind, receiver_type, argument_count,
                 result_type, flags) == (
            index, expected_source, expected_kind, 7, expected_arguments,
            expected_result, 0), "requirement-call structure")
        require(caller_id == 0, "focused helper/app caller identity")
        call = source_slice(sources, source, call_at, call_len)
        require(name in call and b"(" in call and b")" in call,
                "requirement-call source custody")

    require(all(row[4] < 6 for row in calls), "requirement-call target domain")


if __name__ == "__main__":
    run("R5", check)
