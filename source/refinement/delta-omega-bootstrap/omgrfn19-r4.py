#!/usr/bin/env python3
"""R4: helper/ranking, checked adapters, and intrinsic-candidate closure."""

import sys

from omgrfn19_frame import require, split
from omgrfn19_owner import run
from omgrfn19_source import check_header_join, decode_sources
from omgrfn19_witness import decode, source_slice, span_word


def check() -> None:
    frame = split(sys.stdin.buffer.read())
    witness = decode(frame.witness)
    envelope, sources = decode_sources(frame)
    check_header_join(envelope, witness)

    provider = witness.tables["providers"][0]
    identifier, source, owner, name_at, name_len, flags = provider
    require((identifier, source, owner, flags)
            == (0, 0, envelope.sources[0].owner_package_id, 1), "nominal provider")
    span_word(sources, source, name_at, name_len, b"ConsoleNativeProvider")

    helper = witness.tables["helpers"][0]
    (identifier, source, name_at, name_len, console_type, bytes_type, newline_type,
     rank_parameter, rank_kind, reach_trait, body_at, body_len) = helper
    require((identifier, source, console_type, bytes_type, newline_type,
             rank_parameter, rank_kind, reach_trait) == (0, 0, 7, 5, 3, 1, 1, 0),
            "ranked console_write_bytes helper identity")
    span_word(sources, source, name_at, name_len, b"console_write_bytes")
    body = source_slice(sources, source, body_at, body_len)
    require(body.count(b"bytes.len > 0") == 2
            and body.count(b"bytes[0]") == 2
            and body.count(b"bytes[1..]") == 2
            and body.count(b"console.write_byte") == 2,
            "complete recurrent helper body")
    before_body = sources[source][:body_at]
    require(b"terminates by bytes -> Slice::Length" in before_body
            and b"reaches Console" in before_body,
            "helper ranking and reach source custody")

    expected_adapters = ((b"write", 1, 0), (b"write_line", 0, 1))
    for row, (name, requirement, ordinary_call) in zip(
            witness.tables["adapters"], expected_adapters):
        (identifier, source, provider_id, requirement_id, name_at, name_len,
         console_type, argument_type, helper_id, call_id, body_at, body_len, flags) = row
        require((source, provider_id, requirement_id, console_type, argument_type,
                 helper_id, call_id, flags) == (
            0, 0, requirement, 7, 5, 0, ordinary_call, 1),
            f"checked adapter {name!r}")
        span_word(sources, source, name_at, name_len, name)
        adapter_body = source_slice(sources, source, body_at, body_len)
        require(adapter_body.count(b"console_write_bytes") == 1
                and (b"true" if name == b"write_line" else b"false") in adapter_body,
                "checked adapter body/helper selection")

    candidates = witness.tables["candidates"]
    expected = (
        (1, 0, 0, 1, b"write_line", 0, 2, 0, 0, 1),
        (1, 0, 0, 0, b"write", 2, 2, 0, 0, 1),
        (2, 1, 0, 0, b"read_line", 4, 1, 0, 1, 2),
        (2, 1, 0, 1, b"read_byte", 5, 0, 4, 1, 2),
        (2, 1, 0, 2, b"write_byte", 5, 1, 0, 1, 2),
        (2, 1, 0, 3, b"exit_process", 6, 1, 0, 1, 2),
    )
    for identifier, (row, exp) in enumerate(zip(candidates, expected)):
        (kind, source, provider_id, implementation, name, parameter_start,
         parameter_count, result, target, binding) = exp
        (row_id, actual_kind, actual_source, actual_provider, requirement,
         implementation_id, name_at, name_len, actual_start, actual_count,
         result_type, reach_trait, actual_target, actual_binding) = row
        require((row_id, actual_kind, actual_source, actual_provider, requirement,
                 implementation_id, actual_start, actual_count, result_type,
                 reach_trait, actual_target, actual_binding) == (
            identifier, kind, source, provider_id, identifier, implementation,
            parameter_start, parameter_count, result, 0, target, binding),
            f"candidate {identifier} identity")
        span_word(sources, source, name_at, name_len, name)

    param_owners = (0, 0, 1, 1, 2, 4, 5)
    param_ordinals = (0, 1, 0, 1, 0, 0, 0)
    param_types = (7, 5, 7, 5, 6, 1, 1)
    for index, row in enumerate(witness.tables["candidate_params"]):
        identifier, candidate, ordinal, type_id, name_at, name_len = row
        require((identifier, candidate, ordinal, type_id) == (
            index, param_owners[index], param_ordinals[index], param_types[index]),
            "candidate parameter structure")
        require(source_slice(sources, candidates[candidate][2], name_at, name_len)
                in (b"console", b"text", b"out_line", b"byte", b"return_code"),
                "candidate parameter source name")

    for index, row in enumerate(witness.tables["ordinary_calls"]):
        (identifier, source, caller_kind, caller_id, helper_id, call_at,
         call_len, argument_count, flags) = row
        require((identifier, source, caller_kind, caller_id, helper_id,
                 argument_count, flags) == (index, 0, 1, index, 0, 3, 0),
                "adapter-to-helper ordinary call")
        call = source_slice(sources, source, call_at, call_len)
        require(b"console_write_bytes" in call and b"(" in call and b")" in call,
                "adapter-to-helper call source custody")


if __name__ == "__main__":
    run("R4", check)
