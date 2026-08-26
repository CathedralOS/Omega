#!/usr/bin/env python3
"""Exact OMGCOMP3 source projection for OMGRFN20."""

import hashlib

from omgrfn20_frame import RefinementError, RefinementResourceError, require


SOURCE_SHA256 = (
    "4bd0a4881556e1fdf7765b09c64d9040fea0d0341a041603c54deb4369ffa3e6",
    "98dcaae95624399d456867b18d21eb32601821c0e8db9e547b9471ace9ddb0dc",
    "70ae2d9a65e5d06158ff78f4ef7075595bf5ddb58743555a2e4bc35a419a8821",
    "d86566cf25a209a523208bfedc564eb5ff2728efe9ead5aad0c3617795c42ad9",
)


def decode_sources(frame):
    from omgrfn20_frame import compilation_v3

    try:
        envelope = compilation_v3.decode(frame.omgcomp)
    except compilation_v3.CompilationError as error:
        if getattr(error, "status", 251) == 252:
            raise RefinementResourceError(f"OMGCOMP3 source: {error}") from error
        raise RefinementError(f"OMGCOMP3 source: {error}") from error
    sources = tuple(
        envelope.bundle_entries[row.bundle_entry_id].content for row in envelope.sources
    )
    require(len(envelope.packages) == 2 and len(envelope.sources) == 4
            and len(envelope.aliases) == 1, "focused source graph counts")
    require(tuple(package.key for package in envelope.packages)
            == (bytes.fromhex("11" * 32), bytes.fromhex("22" * 32)),
            "focused package identities")
    require(tuple((row.source_id, row.owner_package_id, row.bundle_entry_id,
                   row.module_string_id) for row in envelope.sources) == (
        (0, 0, 2, 2), (1, 0, 3, 2), (2, 1, 0, 1), (3, 1, 1, 1)),
        "focused source/module custody")
    require(tuple(envelope.strings) == ("Main", "app", "console", "main", "omega_std"),
            "focused canonical strings")
    alias = envelope.aliases[0]
    require((alias.requester_package_id, alias.alias_string_id,
             alias.target_package_id) == (1, 4, 0), "direct package alias")
    require(tuple(hashlib.sha256(source).hexdigest() for source in sources)
            == SOURCE_SHA256, "frozen explicit-cast source profile")
    return envelope, sources


def check_join(envelope, witness) -> None:
    require(witness.input_length == envelope.encoded_length,
            "OMGRSW9 input envelope length")
    require((witness.build_source, witness.root_source)
            == (envelope.build_source_id, envelope.root_source_id),
            "build/root source join")
    require((witness.target, witness.configuration, witness.selected_plan,
             witness.selected_trait, witness.selected_provider) == (1, 1, 0, 0, 0),
            "selected target/configuration/plan identities")
