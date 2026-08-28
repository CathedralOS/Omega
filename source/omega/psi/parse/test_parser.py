#!/usr/bin/env python3
"""Black-box checks for the live Omega-written Psi parser slices."""

from __future__ import annotations

import struct
import subprocess
import sys
from dataclasses import dataclass


@dataclass(frozen=True)
class Observation:
    accepted: bool
    diagnostic: int
    diagnostic_span: tuple[int, int, int]
    roots: tuple[tuple[int, int, int, int, int], ...]
    uses: tuple[tuple[int, int, int, int, int], ...]
    members: tuple[tuple[int, int, int], ...]
    data_items: tuple[tuple[int, int, int, int, int, bool, int, int, int], ...]
    fields: tuple[tuple[int, int, int, int, int, int, int], ...]
    type_references: tuple[tuple[int, int, int, int, int, int, int], ...]


class Reader:
    def __init__(self, payload: bytes) -> None:
        self.payload = payload
        self.cursor = 0

    def bytes(self, count: int) -> bytes:
        end = self.cursor + count
        if end > len(self.payload):
            raise AssertionError("truncated parser observation")
        value = self.payload[self.cursor:end]
        self.cursor = end
        return value

    def byte(self) -> int:
        return self.bytes(1)[0]

    def u64(self) -> int:
        return struct.unpack("<Q", self.bytes(8))[0]


def decode(payload: bytes) -> Observation:
    reader = Reader(payload)
    assert reader.bytes(8) == b"OMGPAR2\0"
    assert reader.u64() == 2
    accepted = reader.byte() == 1
    diagnostic = reader.byte()
    diagnostic_span = (reader.u64(), reader.u64(), reader.u64())
    roots = tuple(
        (reader.byte(), reader.u64(), reader.u64(), reader.u64(), reader.u64())
        for _ in range(reader.u64())
    )
    uses = tuple(
        (reader.u64(), reader.u64(), reader.u64(), reader.u64(), reader.u64())
        for _ in range(reader.u64())
    )
    members = tuple(
        (reader.u64(), reader.u64(), reader.u64())
        for _ in range(reader.u64())
    )
    data_items = tuple(
        (
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.byte() == 1,
            reader.u64(),
            reader.u64(),
            reader.u64(),
        )
        for _ in range(reader.u64())
    )
    fields = tuple(
        (
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
        )
        for _ in range(reader.u64())
    )
    type_references = tuple(
        (
            reader.byte(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
        )
        for _ in range(reader.u64())
    )
    assert reader.cursor == len(payload), "trailing parser observation bytes"
    return Observation(
        accepted,
        diagnostic,
        diagnostic_span,
        roots,
        uses,
        members,
        data_items,
        fields,
        type_references,
    )


def run(program: str, source: bytes) -> tuple[int, bytes]:
    completed = subprocess.run([program], input=source, capture_output=True, check=False)
    assert completed.stderr == b"", completed.stderr.decode(errors="replace")
    return completed.returncode, completed.stdout


def accepted(
    program: str,
    name: str,
    source: bytes,
    roots: tuple[tuple[int, int, int, int, int], ...],
    uses: tuple[tuple[int, int, int, int, int], ...],
    members: tuple[tuple[int, int, int], ...],
    data_items: tuple[tuple[int, int, int, int, int, bool, int, int, int], ...] = (),
    fields: tuple[tuple[int, int, int, int, int, int, int], ...] = (),
    type_references: tuple[tuple[int, int, int, int, int, int, int], ...] = (),
) -> None:
    status, payload = run(program, source)
    assert status == 0, f"{name}: status {status}"
    actual = decode(payload)
    expected = Observation(
        True,
        0,
        (1, 0, 0),
        roots,
        uses,
        members,
        data_items,
        fields,
        type_references,
    )
    assert actual == expected, f"{name}:\nexpected {expected}\nactual   {actual}"


def rejected(
    program: str,
    name: str,
    source: bytes,
    diagnostic: int,
    span: tuple[int, int],
    root_count: int = 0,
    member_count: int = 0,
    data_count: int = 0,
    field_count: int = 0,
    type_reference_count: int = 0,
) -> None:
    status, payload = run(program, source)
    assert status == 250, f"{name}: status {status}"
    actual = decode(payload)
    assert not actual.accepted, name
    assert actual.diagnostic == diagnostic, (name, actual)
    assert actual.diagnostic_span == (1, span[0], span[1]), (name, actual)
    assert len(actual.roots) == root_count, (name, actual)
    assert len(actual.members) == member_count, (name, actual)
    assert len(actual.data_items) == data_count, (name, actual)
    assert len(actual.fields) == field_count, (name, actual)
    assert len(actual.type_references) == type_reference_count, (name, actual)


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: test_parser.py <omega-product-program>")
    program = sys.argv[1]

    accepted(program, "empty", b"", (), (), ())
    accepted(program, "trivia-only", b" \n// hello\n/* world */", (), (), ())
    accepted(
        program,
        "simple-use",
        b"use a;",
        ((1, 0, 1, 0, 6),),
        ((0, 1, 1, 0, 6),),
        ((1, 4, 5),),
    )
    accepted(
        program,
        "nested-use",
        b"use foo::bar;",
        ((1, 0, 1, 0, 13),),
        ((0, 2, 1, 0, 13),),
        ((1, 4, 7), (1, 9, 12)),
    )
    accepted(
        program,
        "trivia-between-parts",
        b"/*a*/use/*b*/foo /*c*/::\nbar/*d*/;",
        ((1, 0, 1, 5, 34),),
        ((0, 2, 1, 5, 34),),
        ((1, 13, 16), (1, 25, 28)),
    )
    accepted(
        program,
        "multiple-order",
        b"use a;\nuse b::c;",
        ((1, 0, 1, 0, 6), (1, 1, 1, 7, 16)),
        ((0, 1, 1, 0, 6), (1, 2, 1, 7, 16)),
        ((1, 4, 5), (1, 11, 12), (1, 14, 15)),
    )
    accepted(
        program,
        "contextual-keywords",
        b"use data::machine;",
        ((1, 0, 1, 0, 18),),
        ((0, 2, 1, 0, 18),),
        ((1, 4, 8), (1, 10, 17)),
    )
    accepted(
        program,
        "records-visibility-order-and-named-types",
        b"pub data Empty {} data data { machine: use; } use dep; "
        b"data Main { console: Console; lexer: Lexer; parser: Parser; }",
        (
            (2, 0, 1, 0, 17),
            (2, 1, 1, 18, 45),
            (1, 0, 1, 46, 54),
            (2, 2, 1, 55, 116),
        ),
        ((0, 1, 1, 46, 54),),
        ((1, 50, 53),),
        (
            (1, 9, 14, 0, 0, True, 1, 0, 17),
            (1, 23, 27, 0, 1, False, 1, 18, 45),
            (1, 60, 64, 1, 3, False, 1, 55, 116),
        ),
        (
            (1, 30, 37, 0, 1, 30, 43),
            (1, 67, 74, 1, 1, 67, 84),
            (1, 85, 90, 2, 1, 85, 98),
            (1, 99, 105, 3, 1, 99, 114),
        ),
        (
            (1, 1, 39, 42, 1, 39, 42),
            (1, 1, 76, 83, 1, 76, 83),
            (1, 1, 92, 97, 1, 92, 97),
            (1, 1, 107, 113, 1, 107, 113),
        ),
    )

    rejected(program, "missing-first-member", b"use;", 3, (3, 4))
    rejected(program, "missing-next-member", b"use a::;", 3, (7, 8), member_count=1)
    rejected(program, "missing-semicolon", b"use a", 4, (5, 5), member_count=1)
    rejected(program, "pub-use-is-not-a-root", b"pub use a;", 9, (4, 7))
    rejected(program, "lower-self-is-not-a-member", b"use self;", 3, (4, 8))
    rejected(program, "upper-self-is-not-a-member", b"use Self;", 3, (4, 8))
    rejected(
        program,
        "incomplete-data-after-use",
        b"use a; data X;",
        11,
        (13, 14),
        root_count=1,
        member_count=1,
    )

    data_rejections = (
        ("missing-data-after-pub", b"pub machine X {}", 9, (4, 11)),
        ("missing-data-name", b"data {}", 10, (5, 6)),
        ("missing-data-body", b"data X;", 11, (6, 7)),
        ("missing-field-colon", b"data X { field Type; }", 12, (15, 19)),
        ("missing-field-type", b"data X { field: ; }", 13, (16, 17)),
        ("missing-field-semicolon", b"data X { field: T }", 14, (18, 19)),
        ("properties-not-yet-supported", b"data X [copy] {}", 11, (7, 8)),
        ("case-members-fail-closed", b"data X { case A; }", 12, (14, 15)),
        ("array-types-not-yet-supported", b"data X { field: [u8; 4]; }", 13, (16, 17)),
        ("qualified-types-not-yet-supported", b"data X { field: T in Domain; }", 14, (18, 20)),
    )
    for name, source, diagnostic, span in data_rejections:
        rejected(program, name, source, diagnostic, span)

    rejected(
        program,
        "unsupported-root-after-data",
        b"data X {} machine run() {}",
        2,
        (10, 17),
        root_count=1,
        data_count=1,
    )

    exact_roots = b"".join(f"use n{i};".encode() for i in range(256))
    accepted_status, accepted_payload = run(program, exact_roots)
    assert accepted_status == 0
    exact_observation = decode(accepted_payload)
    assert len(exact_observation.roots) == 256
    assert len(exact_observation.uses) == 256
    assert len(exact_observation.members) == 256
    overflow_start = len(exact_roots)
    rejected(
        program,
        "root-capacity-plus-one",
        exact_roots + b"use overflow;",
        5,
        (overflow_start, overflow_start + 3),
        root_count=256,
        member_count=256,
    )

    exact_members = b"use " + b"::".join(f"n{i}".encode() for i in range(1024)) + b";"
    exact_member_status, exact_member_payload = run(program, exact_members)
    assert exact_member_status == 0
    assert len(decode(exact_member_payload).members) == 1024
    overflowing_member_start = len(exact_members) - 1 + 2
    rejected(
        program,
        "member-capacity-plus-one",
        exact_members[:-1] + b"::overflow;",
        7,
        (overflowing_member_start, overflowing_member_start + 8),
        member_count=1024,
    )

    exact_fields = b"data Huge { " + b"".join(
        f"f{index}: T; ".encode() for index in range(1024)
    ) + b"}"
    exact_field_status, exact_field_payload = run(program, exact_fields)
    assert exact_field_status == 0
    exact_field_observation = decode(exact_field_payload)
    assert len(exact_field_observation.fields) == 1024
    assert len(exact_field_observation.type_references) == 1024
    overflow_field_start = len(exact_fields) - 1
    rejected(
        program,
        "field-capacity-plus-one",
        exact_fields[:-1] + b"overflow: T; }",
        16,
        (overflow_field_start, overflow_field_start + 8),
        field_count=1024,
        type_reference_count=1024,
    )

    lexical_status, lexical_payload = run(program, b"use \\;")
    assert lexical_status == 251
    assert lexical_payload.startswith(b"OMGLEX1\0")

    repeat_source = b"use repeated::observation; data Stable { value: Value; }"
    first = run(program, repeat_source)
    second = run(program, repeat_source)
    assert first == second, "parser observation is not deterministic"

    print("Psi parser slices: 34 cases passed")


if __name__ == "__main__":
    main()
