#!/usr/bin/env python3
"""R2: build-role, source-selection, and BuildOverride reconstruction."""

import sys

from omgrfn19_frame import require, split
from omgrfn19_owner import run
from omgrfn19_source import check_header_join, decode_sources
from omgrfn19_witness import decode, source_slice, span_word, token_values


BUILD_TOKENS = (
    b"machine", b"build", b"(", b"builder", b":", b"&", b"mut", b"Build", b")", b"{",
    b"builder", b".", b"application", b"(", b'"omega-bootstrap-provider-plan"', b")", b";",
    b"builder", b".", b"select_provider", b"<", b"Console", b",", b"ConsoleNativeProvider", b">",
    b"(", b")", b";", b"}",
)


def check() -> None:
    frame = split(sys.stdin.buffer.read())
    witness = decode(frame.witness)
    envelope, sources = decode_sources(frame)
    check_header_join(envelope, witness)
    require(token_values(sources[witness.build_source]) == BUILD_TOKENS,
            "exact free build role and two-statement selection body")

    units = witness.tables["units"]
    require(len(units) == len(envelope.sources), "unit/source bijection")
    for unit, source in zip(units, envelope.sources):
        identifier, source_id, owner, module, flags, start, length = unit
        expected_flags = (1 if source_id == envelope.build_source_id else 0) \
            | (2 if source_id == envelope.root_source_id else 0)
        require((identifier, source_id, owner, module, flags, start, length) == (
            source_id, source_id, source.owner_package_id, source.module_string_id,
            expected_flags, 0, len(sources[source_id])), "unit/source role custody")

    build = witness.tables["build_machines"][0]
    (identifier, source, name_at, name_len, app_at, app_len, selection_at,
     selection_len, flags, reserved) = build
    require((identifier, source, flags, reserved) == (0, witness.build_source, 1, 0),
            "free build-machine identity")
    span_word(sources, source, name_at, name_len, b"build")
    require(source_slice(sources, source, app_at, app_len)
            == b'application("omega-bootstrap-provider-plan")',
            "application-role invocation span")
    selection_source = source_slice(sources, source, selection_at, selection_len)
    require(selection_source
            == b"select_provider<Console, ConsoleNativeProvider>()",
            "typed provider-selection invocation span")
    require(app_at < selection_at and app_at + app_len <= selection_at,
            "application precedes selection")

    selection = witness.tables["selections"][0]
    (identifier, source, machine, trait, provider, call_at, call_len,
     provenance, flags) = selection
    require((identifier, source, machine, trait, provider, provenance, flags)
            == (0, witness.build_source, 0, 0, 0, 1, 0),
            "exact BuildOverride provenance")
    require((call_at, call_len) == (selection_at, selection_len),
            "selection/build-machine span identity")


if __name__ == "__main__":
    run("R2", check)
