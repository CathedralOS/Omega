#!/usr/bin/env python3
"""R3: complete six-requirement Console schema and reach reconstruction."""

import sys

from omgrfn19_frame import require, split
from omgrfn19_owner import run
from omgrfn19_source import check_header_join, decode_sources
from omgrfn19_witness import NO_ID, decode, span_word


NAMES = (b"write_line", b"write", b"read_line", b"read_byte", b"write_byte", b"exit_process")
PARAM_OWNERS = (0, 1, 2, 4, 5)
PARAM_TYPES = (5, 5, 6, 1, 1)
RESULT_TYPES = (0, 0, 0, 4, 0, 0)


def check() -> None:
    frame = split(sys.stdin.buffer.read())
    witness = decode(frame.witness)
    envelope, sources = decode_sources(frame)
    check_header_join(envelope, witness)

    types = witness.tables["types"]
    require(tuple(row[1] for row in types) == tuple(range(8)), "normalized type kinds")
    require(all(row[2:4] == (0, 0) for row in types), "normalized type flags/reserved")
    require(types[0][4:6] == (NO_ID, NO_ID) and types[0][6:] == (0, 0), "Unit type")
    require(types[1][4:6] == (NO_ID, NO_ID) and types[1][6:] == (0x80000000, 0x7fffffff), "full i32")
    require(types[2][4:6] == (NO_ID, NO_ID) and types[2][6:] == (0, 255), "full u8")
    require(types[3][4:6] == (NO_ID, NO_ID) and types[3][6:] == (0, 1), "bool")
    require(types[4][4] == 0 and types[5][6:] == (2, 0)
            and types[6][6:] == (2, 0) and types[7][4] == 0 and types[7][6] == 0,
            "nominal/view/Console type identities")

    trait = witness.tables["traits"][0]
    identifier, source, owner, name_at, name_len, req_start, req_count, flags = trait
    require((identifier, source, owner, req_start, req_count, flags)
            == (0, 0, envelope.sources[0].owner_package_id, 0, 6, 3),
            "public boundary Console trait")
    span_word(sources, source, name_at, name_len, b"Console")

    requirements = witness.tables["requirements"]
    for ordinal, (row, name, result_type) in enumerate(zip(requirements, NAMES, RESULT_TYPES)):
        (identifier, trait_id, row_ordinal, source, name_at, name_len,
         parameter_start, parameter_count, result, reach_start, reach_count, flags) = row
        expected_parameter_start = sum(1 for owner in PARAM_OWNERS if owner < ordinal)
        expected_parameter_count = int(ordinal in PARAM_OWNERS)
        require((identifier, trait_id, row_ordinal, source, parameter_start,
                 parameter_count, result, reach_start, reach_count, flags) == (
            ordinal, 0, ordinal, 0, expected_parameter_start,
            expected_parameter_count, result_type, ordinal, 1, 0),
            f"requirement {ordinal} structure")
        span_word(sources, source, name_at, name_len, name)

    for index, (row, owner, type_id) in enumerate(zip(
            witness.tables["requirement_params"], PARAM_OWNERS, PARAM_TYPES)):
        identifier, requirement, ordinal, actual_type, name_at, name_len = row
        require((identifier, requirement, ordinal, actual_type)
                == (index, owner, 0, type_id), "requirement parameter identity")
        span_word(sources, 0, name_at, name_len,
                  (b"text", b"text", b"out_line", b"byte", b"return_code")[index])

    for index, row in enumerate(witness.tables["reaches"]):
        identifier, requirement, trait_id, source, name_at, name_len = row
        require((identifier, requirement, trait_id, source) == (index, index, 0, 0),
                "one exact Console reach per requirement")
        span_word(sources, source, name_at, name_len, b"Console")


if __name__ == "__main__":
    run("R3", check)
