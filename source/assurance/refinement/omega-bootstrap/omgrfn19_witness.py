#!/usr/bin/env python3
"""Raw OMGRSW9 table decoder shared by conclusion-local OMGRFN19 owners."""

from __future__ import annotations

import dataclasses
import re
import struct

from omgrfn19_frame import RefinementError, RefinementResourceError, require


NO_ID = 0xFFFF_FFFF
HEADER = struct.Struct("<8s4H32I")
TOTAL_BYTES = 2_304
COUNT_NAMES = (
    "units", "types", "traits", "requirements", "requirement_params",
    "reaches", "providers", "helpers", "adapters", "candidates",
    "candidate_params", "build_machines", "selections", "plans",
    "plan_rows", "requirement_calls", "ordinary_calls",
)
EXPECTED_COUNTS = (4, 8, 1, 6, 5, 6, 1, 1, 2, 6, 7, 1, 1, 1, 6, 6, 2)
ROWS = {
    "units": struct.Struct("<7I"),
    "types": struct.Struct("<IBBH4I"),
    "traits": struct.Struct("<8I"),
    "requirements": struct.Struct("<12I"),
    "requirement_params": struct.Struct("<6I"),
    "reaches": struct.Struct("<6I"),
    "providers": struct.Struct("<6I"),
    "helpers": struct.Struct("<12I"),
    "adapters": struct.Struct("<13I"),
    "candidates": struct.Struct("<14I"),
    "candidate_params": struct.Struct("<6I"),
    "build_machines": struct.Struct("<10I"),
    "selections": struct.Struct("<9I"),
    "plans": struct.Struct("<9I"),
    "plan_rows": struct.Struct("<6I"),
    "requirement_calls": struct.Struct("<11I"),
    "ordinary_calls": struct.Struct("<9I"),
}


@dataclasses.dataclass(frozen=True)
class Witness:
    raw: bytes
    input_length: int
    counts: dict[str, int]
    build_source: int
    root_source: int
    target: int
    configuration: int
    selected_plan: int
    selected_trait: int
    selected_provider: int
    tables: dict[str, tuple[tuple[int, ...], ...]]
    offsets: dict[str, tuple[int, int]]


def decode(raw: bytes) -> Witness:
    if len(raw) > 524_288:
        raise RefinementResourceError("OMGRSW9 publication ceiling")
    require(len(raw) >= HEADER.size, "truncated OMGRSW9 header")
    fields = HEADER.unpack_from(raw)
    magic, major, minor, flags, header_size = fields[:5]
    words = fields[5:]
    require((magic, major, minor, flags, header_size)
            == (b"OMGRSW9\0", 9, 0, 0, HEADER.size), "exact OMGRSW9 identity")
    total, input_length = words[:2]
    counts_tuple = words[2:19]
    build_source, root_source, target, configuration = words[19:23]
    selected_plan, selected_trait, selected_provider = words[23:26]
    require(words[26:] == (0,) * 6, "OMGRSW9 reserved words")
    require(total == TOTAL_BYTES == len(raw), "exact OMGRSW9 length/EOF")
    require(counts_tuple == EXPECTED_COUNTS, "exact OMGRSW9 table counts")
    counts = dict(zip(COUNT_NAMES, counts_tuple))
    cursor = HEADER.size
    tables: dict[str, tuple[tuple[int, ...], ...]] = {}
    offsets: dict[str, tuple[int, int]] = {}
    for name in COUNT_NAMES:
        row = ROWS[name]
        length = counts[name] * row.size
        require(length <= len(raw) - cursor, f"{name} table extent")
        offsets[name] = (cursor, length)
        tables[name] = tuple(
            row.unpack_from(raw, cursor + index * row.size)
            for index in range(counts[name])
        )
        cursor += length
    require(cursor == len(raw), "OMGRSW9 exact table EOF")
    for name, rows in tables.items():
        require(all(row[0] == index for index, row in enumerate(rows)),
                f"{name} dense IDs")
    return Witness(raw, input_length, counts, build_source, root_source, target,
                   configuration, selected_plan, selected_trait,
                   selected_provider, tables, offsets)


TOKEN = re.compile(
    rb"[A-Za-z_][A-Za-z0-9_]*|[0-9]+|\"(?:[^\"\\]|\\.)*\"|::|->|\.\.|==|!=|<=|>=|&&|\|\||."
)


def lex(source: bytes) -> list[tuple[bytes, int, int]]:
    result: list[tuple[bytes, int, int]] = []
    cursor = 0
    while cursor < len(source):
        if source[cursor] in b" \t\r\n":
            cursor += 1
            continue
        if source.startswith(b"//", cursor):
            end = source.find(b"\n", cursor + 2)
            cursor = len(source) if end < 0 else end + 1
            continue
        if source.startswith(b"/*", cursor):
            end = source.find(b"*/", cursor + 2)
            require(end >= 0, "unterminated source block comment")
            cursor = end + 2
            continue
        match = TOKEN.match(source, cursor)
        require(match is not None, "unlexable source byte")
        token = match.group(0)
        result.append((token, cursor, match.end()))
        cursor = match.end()
    return result


def source_slice(sources: tuple[bytes, ...], source: int, start: int, length: int) -> bytes:
    require(source < len(sources), "source span owner")
    require(start <= len(sources[source]) and length <= len(sources[source]) - start,
            "source span extent")
    raw = sources[source][start:start + length]
    require(raw != b"", "nonempty source span")
    boundaries = {0, len(sources[source])}
    for _, first, end in lex(sources[source]):
        boundaries.add(first)
        boundaries.add(end)
    require(start in boundaries and start + length in boundaries,
            "source span token boundaries")
    return raw


def span_word(sources: tuple[bytes, ...], source: int, start: int,
              length: int, expected: bytes) -> None:
    require(source_slice(sources, source, start, length) == expected,
            f"exact source word {expected!r}")


def token_values(source: bytes) -> tuple[bytes, ...]:
    return tuple(token for token, _, _ in lex(source))
