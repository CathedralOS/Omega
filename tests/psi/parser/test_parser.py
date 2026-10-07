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
    data_items: tuple[
        tuple[int, int, int, int, int, int, int, int, int, int, bool, int, int, int, int, int],
        ...,
    ]
    data_members: tuple[tuple[int, int, int, int, int], ...]
    fields: tuple[tuple[int, int, int, int, int, int, int], ...]
    payload_fields: tuple[tuple[int, int, int, int, int, int, int], ...]
    cases: tuple[tuple[int, int, int, int, int, int, int, int], ...]
    type_references: tuple[
        tuple[int, int, int, int, int, int, int, int, int, int, int, int, int], ...
    ]
    generic_parameters: tuple[
        tuple[int, int, int, int, int, int, int, int, int, int], ...
    ]
    machines: tuple[tuple[int, ...], ...] = ()
    parameters: tuple[tuple[int, ...], ...] = ()
    states: tuple[tuple[int, ...], ...] = ()


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
    assert reader.bytes(8) == b"OMGPAR9\0"
    assert reader.u64() == 9
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
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.byte(),
            reader.byte() == 1,
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
        )
        for _ in range(reader.u64())
    )
    data_members = tuple(
        (reader.byte(), reader.u64(), reader.u64(), reader.u64(), reader.u64())
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
    payload_fields = tuple(
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
    cases = tuple(
        (
            reader.u64(),
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
            reader.byte(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.byte(),
            reader.u64(),
        )
        for _ in range(reader.u64())
    )
    generic_parameters = tuple(
        (
            reader.byte(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.byte(),
            reader.u64(),
            reader.byte(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
        )
        for _ in range(reader.u64())
    )
    machines = tuple(
        (
            reader.byte(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.byte(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.byte(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
        )
        for _ in range(reader.u64())
    )
    parameters = tuple(
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
    states = tuple(
        (
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.byte(),
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
        data_members,
        fields,
        payload_fields,
        cases,
        type_references,
        generic_parameters,
        machines,
        parameters,
        states,
    )


# Every distinct source the gate runs is one case; the determinism check runs
# its source twice and counts once.
CASE_SOURCES: set[bytes] = set()


def run(program: str, source: bytes) -> tuple[int, bytes]:
    CASE_SOURCES.add(source)
    completed = subprocess.run([program], input=source, capture_output=True, check=False)
    assert completed.stderr == b"", completed.stderr.decode(errors="replace")
    return completed.returncode, completed.stdout


def with_reference_tail(row: tuple[int, ...]) -> tuple[int, ...]:
    """A named or qualified row spelled without the reference tail has none:
    zero access and a zero referee handle."""
    return row if len(row) == 13 else row + (0, 0)


def accepted(
    program: str,
    name: str,
    source: bytes,
    roots: tuple[tuple[int, int, int, int, int], ...],
    uses: tuple[tuple[int, int, int, int, int], ...],
    members: tuple[tuple[int, int, int], ...],
    data_items: tuple[
        tuple[int, int, int, int, int, int, int, int, int, int, bool, int, int, int],
        ...,
    ] = (),
    data_members: tuple[tuple[int, int, int, int, int], ...] = (),
    fields: tuple[tuple[int, int, int, int, int, int, int], ...] = (),
    cases: tuple[tuple[int, int, int, int, int, int, int, int], ...] = (),
    type_references: tuple[
        tuple[int, int, int, int, int, int, int, int, int, int, int], ...
    ] = (),
    payload_fields: tuple[tuple[int, int, int, int, int, int, int], ...] = (),
    generic_parameters: tuple[
        tuple[int, int, int, int, int, int, int, int, int, int], ...
    ] = (),
    machines: tuple[tuple[int, ...], ...] = (),
    parameters: tuple[tuple[int, ...], ...] = (),
    states: tuple[tuple[int, ...], ...] = (),
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
        data_members,
        fields,
        payload_fields,
        cases,
        tuple(with_reference_tail(row) for row in type_references),
        generic_parameters,
        machines,
        parameters,
        states,
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
    data_member_count: int = 0,
    field_count: int = 0,
    payload_field_count: int = 0,
    case_count: int = 0,
    type_reference_count: int = 0,
    generic_parameter_count: int = 0,
    machine_count: int = 0,
    parameter_count: int = 0,
    state_count: int = 0,
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
    assert len(actual.data_members) == data_member_count, (name, actual)
    assert len(actual.fields) == field_count, (name, actual)
    assert len(actual.payload_fields) == payload_field_count, (name, actual)
    assert len(actual.cases) == case_count, (name, actual)
    assert len(actual.type_references) == type_reference_count, (name, actual)
    assert len(actual.generic_parameters) == generic_parameter_count, (name, actual)
    assert len(actual.machines) == machine_count, (name, actual)
    assert len(actual.parameters) == parameter_count, (name, actual)
    assert len(actual.states) == state_count, (name, actual)


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit(
            "usage: test_parser.py <omega-product-program>"
        )
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
        "contextual-multiplicity-names",
        b"use linear::copy::case;",
        ((1, 0, 1, 0, 23),),
        ((0, 3, 1, 0, 23),),
        ((1, 4, 10), (1, 12, 16), (1, 18, 22)),
    )
    accepted(
        program,
        "linear-record",
        b"data Record [linear] {}",
        ((2, 0, 1, 0, 23),),
        (),
        (),
        ((1, 5, 11, 0, 0, 0, 0, 0, 0, 2, False, 1, 0, 23, 0, 0),),
    )
    accepted(
        program,
        "copy-property-trailing-comma-with-trivia",
        b"data X [copy , ] {}",
        ((2, 0, 1, 0, 19),),
        (),
        (),
        ((1, 5, 6, 0, 0, 0, 0, 0, 0, 1, False, 1, 0, 19, 0, 0),),
    )
    accepted(
        program,
        "linear-property-trailing-comma",
        b"data X [linear,] {}",
        ((2, 0, 1, 0, 19),),
        (),
        (),
        ((1, 5, 6, 0, 0, 0, 0, 0, 0, 2, False, 1, 0, 19, 0, 0),),
    )
    accepted(
        program,
        "linear-sum",
        b"data Sum [linear] { case Empty; }",
        ((2, 0, 1, 0, 33),),
        (),
        (),
        ((1, 5, 8, 0, 1, 0, 0, 0, 1, 2, False, 1, 0, 33, 0, 0),),
        ((2, 0, 1, 20, 31),),
        (),
        ((1, 25, 30, 1, 20, 31, 0, 0),),
    )
    accepted(
        program,
        "linear-mixed-data",
        b"data Mixed [linear] { value: T; case Empty; }",
        ((2, 0, 1, 0, 45),),
        (),
        (),
        ((1, 5, 10, 0, 2, 0, 1, 0, 1, 2, False, 1, 0, 45, 0, 0),),
        ((1, 0, 1, 22, 31), (2, 0, 1, 32, 43)),
        ((1, 22, 27, 0, 1, 22, 31),),
        ((1, 37, 42, 1, 32, 43, 0, 0),),
        ((1, 1, 29, 30, 1, 29, 30, 0, 0, 0, 0),),
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
            (1, 9, 14, 0, 0, 0, 0, 0, 0, 0, True, 1, 0, 17, 0, 0),
            (1, 23, 27, 0, 1, 0, 1, 0, 0, 0, False, 1, 18, 45, 0, 0),
            (1, 60, 64, 1, 3, 1, 3, 0, 0, 0, False, 1, 55, 116, 0, 0),
        ),
        (
            (1, 0, 1, 30, 43),
            (1, 1, 1, 67, 84),
            (1, 2, 1, 85, 98),
            (1, 3, 1, 99, 114),
        ),
        (
            (1, 30, 37, 0, 1, 30, 43),
            (1, 67, 74, 1, 1, 67, 84),
            (1, 85, 90, 2, 1, 85, 98),
            (1, 99, 105, 3, 1, 99, 114),
        ),
        (),
        (
            (1, 1, 39, 42, 1, 39, 42, 0, 0, 0, 0),
            (1, 1, 76, 83, 1, 76, 83, 0, 0, 0, 0),
            (1, 1, 92, 97, 1, 92, 97, 0, 0, 0, 0),
            (1, 1, 107, 113, 1, 107, 113, 0, 0, 0, 0),
        ),
    )
    accepted(
        program,
        "copy-sum-and-mixed-member-order",
        b"pub data Shape [copy] { case Empty; case: Marker; "
        b"value: Value; case Full; }",
        ((2, 0, 1, 0, 76),),
        (),
        (),
        ((1, 9, 14, 0, 4, 0, 2, 0, 2, 1, True, 1, 0, 76, 0, 0),),
        (
            (2, 0, 1, 24, 35),
            (1, 0, 1, 36, 49),
            (1, 1, 1, 50, 63),
            (2, 1, 1, 64, 74),
        ),
        (
            (1, 36, 40, 0, 1, 36, 49),
            (1, 50, 55, 1, 1, 50, 63),
        ),
        (
            (1, 29, 34, 1, 24, 35, 0, 0),
            (1, 69, 73, 1, 64, 74, 0, 0),
        ),
        (
            (1, 1, 42, 48, 1, 42, 48, 0, 0, 0, 0),
            (1, 1, 57, 62, 1, 57, 62, 0, 0, 0, 0),
        ),
    )
    accepted(
        program,
        "empty-case-payload",
        b"data Empty { case None(); }",
        ((2, 0, 1, 0, 27),),
        (),
        (),
        ((1, 5, 10, 0, 1, 0, 0, 0, 1, 0, False, 1, 0, 27, 0, 0),),
        ((2, 0, 1, 13, 25),),
        (),
        ((1, 18, 22, 1, 13, 25, 0, 0),),
    )
    accepted(
        program,
        "one-case-payload-field",
        b"data One { case Some(value: T); }",
        ((2, 0, 1, 0, 33),),
        (),
        (),
        ((1, 5, 8, 0, 1, 0, 0, 0, 1, 0, False, 1, 0, 33, 0, 0),),
        ((2, 0, 1, 11, 31),),
        (),
        ((1, 16, 20, 1, 11, 31, 0, 1),),
        ((1, 1, 28, 29, 1, 28, 29, 0, 0, 0, 0),),
        payload_fields=((1, 21, 26, 0, 1, 21, 29),),
    )
    accepted(
        program,
        "multiple-case-payload-fields",
        b"data Pair { case Both(left: L, right: R); }",
        ((2, 0, 1, 0, 43),),
        (),
        (),
        ((1, 5, 9, 0, 1, 0, 0, 0, 1, 0, False, 1, 0, 43, 0, 0),),
        ((2, 0, 1, 12, 41),),
        (),
        ((1, 17, 21, 1, 12, 41, 0, 2),),
        (
            (1, 1, 28, 29, 1, 28, 29, 0, 0, 0, 0),
            (1, 1, 38, 39, 1, 38, 39, 0, 0, 0, 0),
        ),
        payload_fields=(
            (1, 22, 26, 0, 1, 22, 30),
            (1, 31, 36, 1, 1, 31, 39),
        ),
    )
    accepted(
        program,
        "trailing-comma-case-payload",
        b"data Trailing { case Some(value: T,); }",
        ((2, 0, 1, 0, 39),),
        (),
        (),
        ((1, 5, 13, 0, 1, 0, 0, 0, 1, 0, False, 1, 0, 39, 0, 0),),
        ((2, 0, 1, 16, 37),),
        (),
        ((1, 21, 25, 1, 16, 37, 0, 1),),
        ((1, 1, 33, 34, 1, 33, 34, 0, 0, 0, 0),),
        payload_fields=((1, 26, 31, 0, 1, 26, 35),),
    )
    accepted(
        program,
        "mixed-direct-and-case-payload-fields",
        b"data Mixed { common: C; case None; case Some(value: V); tail: T; }",
        ((2, 0, 1, 0, 66),),
        (),
        (),
        ((1, 5, 10, 0, 4, 0, 2, 0, 2, 0, False, 1, 0, 66, 0, 0),),
        (
            (1, 0, 1, 13, 23),
            (2, 0, 1, 24, 34),
            (2, 1, 1, 35, 55),
            (1, 1, 1, 56, 64),
        ),
        (
            (1, 13, 19, 0, 1, 13, 23),
            (1, 56, 60, 2, 1, 56, 64),
        ),
        (
            (1, 29, 33, 1, 24, 34, 0, 0),
            (1, 40, 44, 1, 35, 55, 0, 1),
        ),
        (
            (1, 1, 21, 22, 1, 21, 22, 0, 0, 0, 0),
            (1, 1, 52, 53, 1, 52, 53, 0, 0, 0, 0),
            (1, 1, 62, 63, 1, 62, 63, 0, 0, 0, 0),
        ),
        payload_fields=((1, 45, 50, 1, 1, 45, 53),),
    )
    accepted(
        program,
        "qualified-field-type",
        b"data X { field: T in Domain; }",
        ((2, 0, 1, 0, 30),),
        (),
        (),
        ((1, 5, 6, 0, 1, 0, 1, 0, 0, 0, False, 1, 0, 30, 0, 0),),
        ((1, 0, 1, 9, 28),),
        ((1, 9, 14, 0, 1, 9, 28),),
        (),
        ((2, 1, 16, 17, 1, 16, 27, 1, 1, 21, 27),),
    )
    accepted(
        program,
        "qualified-payload-field-type",
        b"data X { case A(v: T in D); }",
        ((2, 0, 1, 0, 29),),
        (),
        (),
        ((1, 5, 6, 0, 1, 0, 0, 0, 1, 0, False, 1, 0, 29, 0, 0),),
        ((2, 0, 1, 9, 27),),
        (),
        ((1, 14, 15, 1, 9, 27, 0, 1),),
        ((2, 1, 19, 20, 1, 19, 25, 1, 1, 24, 25),),
        payload_fields=((1, 16, 17, 0, 1, 16, 25),),
    )
    accepted(
        program,
        "qualified-contextual-keyword-domain",
        b"data X { field: T in linear; }",
        ((2, 0, 1, 0, 30),),
        (),
        (),
        ((1, 5, 6, 0, 1, 0, 1, 0, 0, 0, False, 1, 0, 30, 0, 0),),
        ((1, 0, 1, 9, 28),),
        ((1, 9, 14, 0, 1, 9, 28),),
        (),
        ((2, 1, 16, 17, 1, 16, 27, 1, 1, 21, 27),),
    )
    accepted(
        program,
        "generic-type-parameter",
        b"data Vec<T> {}",
        ((2, 0, 1, 0, 14),),
        (),
        (),
        ((1, 5, 8, 0, 0, 0, 0, 0, 0, 0, False, 1, 0, 14, 0, 1),),
        (),
        (),
        (),
        (),
        (),
        ((2, 1, 9, 10, 0, 0, 0, 1, 9, 10),),
    )
    accepted(
        program,
        "generic-type-and-const-parameters",
        b"pub data BoundedQueue<T, const N: u64> [linear] {}",
        ((2, 0, 1, 0, 50),),
        (),
        (),
        ((1, 9, 21, 0, 0, 0, 0, 0, 0, 2, True, 1, 0, 50, 0, 2),),
        (),
        (),
        (),
        ((1, 1, 34, 37, 1, 34, 37, 0, 0, 0, 0),),
        (),
        (
            (2, 1, 22, 23, 0, 0, 0, 1, 22, 23),
            (3, 1, 31, 32, 1, 0, 0, 1, 25, 37),
        ),
    )
    accepted(
        program,
        "generic-lifetime-then-type",
        b"data Pair<'a, T> {}",
        ((2, 0, 1, 0, 19),),
        (),
        (),
        ((1, 5, 9, 0, 0, 0, 0, 0, 0, 0, False, 1, 0, 19, 0, 2),),
        (),
        (),
        (),
        (),
        (),
        (
            (1, 1, 11, 12, 0, 0, 0, 1, 10, 12),
            (2, 1, 14, 15, 0, 0, 0, 1, 14, 15),
        ),
    )
    accepted(
        program,
        "generic-value-binder",
        b"data Sized<u: u64> {}",
        ((2, 0, 1, 0, 21),),
        (),
        (),
        ((1, 5, 10, 0, 0, 0, 0, 0, 0, 0, False, 1, 0, 21, 0, 1),),
        (),
        (),
        (),
        ((1, 1, 14, 17, 1, 14, 17, 0, 0, 0, 0),),
        (),
        ((4, 1, 11, 12, 1, 0, 0, 1, 11, 17),),
    )
    accepted(
        program,
        "generic-machine-binder",
        b"data Exec<machine M> {}",
        ((2, 0, 1, 0, 23),),
        (),
        (),
        ((1, 5, 9, 0, 0, 0, 0, 0, 0, 0, False, 1, 0, 23, 0, 1),),
        (),
        (),
        (),
        (),
        (),
        ((5, 1, 18, 19, 0, 0, 0, 1, 10, 19),),
    )
    accepted(
        program,
        "generic-type-parameter-bounds",
        b"data Cell<T [copy]> {}",
        ((2, 0, 1, 0, 22),),
        (),
        (),
        ((1, 5, 9, 0, 0, 0, 0, 0, 0, 0, False, 1, 0, 22, 0, 1),),
        (),
        (),
        (),
        (),
        (),
        ((2, 1, 10, 11, 0, 0, 1, 1, 10, 18),),
    )
    accepted(
        program,
        "generic-const-parameter-bounds",
        b"data Box<const N: u64 [linear]> {}",
        ((2, 0, 1, 0, 34),),
        (),
        (),
        ((1, 5, 8, 0, 0, 0, 0, 0, 0, 0, False, 1, 0, 34, 0, 1),),
        (),
        (),
        (),
        ((1, 1, 18, 21, 1, 18, 21, 0, 0, 0, 0),),
        (),
        ((3, 1, 15, 16, 1, 0, 2, 1, 9, 30),),
    )
    accepted(
        program,
        "generic-qualified-const-type",
        b"data Rate<const N: u64 in Domain> {}",
        ((2, 0, 1, 0, 36),),
        (),
        (),
        ((1, 5, 9, 0, 0, 0, 0, 0, 0, 0, False, 1, 0, 36, 0, 1),),
        (),
        (),
        (),
        ((2, 1, 19, 22, 1, 19, 32, 1, 1, 26, 32),),
        (),
        ((3, 1, 16, 17, 1, 0, 0, 1, 10, 32),),
    )

    rejected(program, "missing-first-member", b"use;", 3, (3, 4))
    rejected(program, "missing-next-member", b"use a::;", 3, (7, 8), member_count=1)
    rejected(program, "missing-semicolon", b"use a", 4, (5, 5), member_count=1)
    rejected(program, "pub-use-is-not-a-root", b"pub use a;", 9, (4, 7))
    rejected(program, "lower-self-is-not-a-member", b"use self;", 3, (4, 8))
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
        ("missing-item-after-pub", b"pub use a;", 9, (4, 7)),
        ("missing-data-name", b"data {}", 10, (5, 6)),
        ("missing-data-body", b"data X;", 11, (6, 7)),
        ("missing-field-colon", b"data X { field Type; }", 12, (15, 19)),
        ("missing-field-type", b"data X { field: ; }", 13, (16, 17)),
        ("missing-field-semicolon", b"data X { field: T }", 14, (18, 19), 1),
        ("unknown-data-property", b"data X [unknown] {}", 18, (8, 15)),
        ("duplicate-copy-property", b"data X [copy, copy] {}", 29, (14, 18)),
        ("duplicate-linear-property", b"data X [linear, linear] {}", 29, (16, 22)),
        ("conflicting-copy-linear", b"data X [copy, linear] {}", 30, (14, 20)),
        ("conflicting-linear-copy", b"data X [linear, copy] {}", 30, (16, 20)),
        ("malformed-property-comma", b"data X [copy,,] {}", 18, (13, 14)),
        ("unknown-second-property", b"data X [copy, unknown] {}", 18, (14, 21)),
        ("missing-property-comma", b"data X [copy copy] {}", 19, (13, 17)),
        ("missing-case-name", b"data X { case ; }", 20, (14, 15)),
        ("missing-case-semicolon", b"data X { case A }", 21, (16, 17)),
        ("case-discriminant-is-retired", b"data X { case A = 1; }", 21, (16, 17)),
        ("missing-case-payload-field", b"data X { case A(: T); }", 24, (16, 17)),
        ("unnamed-case-payload-field", b"data X { case A(T); }", 25, (17, 18)),
        ("missing-case-payload-colon", b"data X { case A(value T); }", 25, (22, 23)),
        ("missing-case-payload-type", b"data X { case A(value: ); }", 26, (23, 24)),
        (
            "missing-case-payload-comma",
            b"data X { case A(left: L right: R); }",
            27,
            (24, 29),
            1,
        ),
        (
            "unterminated-case-payload",
            b"data X { case A(value: T",
            27,
            (24, 24),
            1,
        ),
        ("legacy-bare-case-is-rejected", b"data X { A; }", 12, (10, 11)),
        ("array-types-not-yet-supported", b"data X { field: [u8; 4]; }", 13, (16, 17)),
        ("missing-qualified-domain", b"data X { field: T in ; }", 31, (21, 22)),
        ("missing-qualified-domain-end", b"data X { field: T in", 31, (20, 20)),
        ("second-type-qualifier", b"data X { field: T in A in B; }", 14, (23, 25), 1),
        ("qualified-domain-union", b"data X { field: T in A | B; }", 14, (23, 24), 1),
        ("missing-payload-qualified-domain", b"data X { case A(v: T in ); }", 31, (24, 25)),
        ("empty-generic-list", b"data X<> {}", 32, (7, 8)),
        ("unterminated-generic-list", b"data X<T", 36, (8, 8)),
        ("generic-lifetime-after-runtime", b"data X<T 'a> {}", 38, (9, 10)),
        ("const-binder-missing-name", b"data X<const> {}", 33, (12, 13)),
        ("const-binder-missing-colon", b"data X<const N u64> {}", 34, (15, 18)),
        ("const-binder-missing-type", b"data X<const N:> {}", 35, (15, 16)),
        ("const-binder-unclosed-after-colon", b"data X<const N:", 35, (15, 15)),
        ("machine-binder-missing-name", b"data X<machine> {}", 33, (14, 15)),
        ("generic-bounds-without-parameter", b"data X<[copy] T> {}", 32, (7, 8)),
        ("value-binder-missing-type", b"data X<u:> {}", 35, (9, 10)),
    )
    # A trailing count is the type references completed before the rejected
    # token: the type grammar retains each type as soon as it is whole.
    for name, source, diagnostic, span, *completed_types in data_rejections:
        rejected(
            program,
            name,
            source,
            diagnostic,
            span,
            type_reference_count=completed_types[0] if completed_types else 0,
        )

    # A rejection after a completed generic parameter keeps that parameter,
    # exactly as the capacity cases below keep every completed one.
    retained_generic_rejections = (
        ("generic-parameter-missing-after-comma", b"data X<T,", 32, (9, 9)),
        ("duplicate-generic-parameter", b"data Dup<T, T> {}", 39, (12, 13)),
        ("proposition-binder-not-admitted", b"data X<prop R> {}", 36, (12, 13)),
        ("generic-lifetime-bounds-rejected", b"data X<'a [copy]> {}", 36, (10, 11)),
    )
    for name, source, diagnostic, span in retained_generic_rejections:
        rejected(program, name, source, diagnostic, span, generic_parameter_count=1)

    rejected(
        program,
        "unsupported-root-after-data",
        b"data X {} trait Run {}",
        2,
        (10, 15),
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
    assert len(exact_field_observation.data_members) == 1024
    assert len(exact_field_observation.fields) == 1024
    assert len(exact_field_observation.type_references) == 1024
    exact_generics = (
        b"data G<" + b",".join(f"p{i}".encode() for i in range(256)) + b"> {}"
    )
    exact_generic_status, exact_generic_payload = run(program, exact_generics)
    assert exact_generic_status == 0
    exact_generic_observation = decode(exact_generic_payload)
    assert len(exact_generic_observation.generic_parameters) == 256
    overflow_generic_start = len(exact_generics) - 4 + 1
    rejected(
        program,
        "generic-capacity-plus-one",
        exact_generics[:-4] + b",overflow> {}",
        37,
        (overflow_generic_start, overflow_generic_start + 8),
        generic_parameter_count=256,
    )

    overflow_field_start = len(exact_fields) - 1
    rejected(
        program,
        "field-capacity-plus-one",
        exact_fields[:-1] + b"overflow: T; }",
        16,
        (overflow_field_start, overflow_field_start + 8),
        data_member_count=1024,
        field_count=1024,
        type_reference_count=1024,
    )

    exact_cases = b"data Cases { " + b"".join(
        f"case C{index}; ".encode() for index in range(512)
    ) + b"}"
    exact_case_status, exact_case_payload = run(program, exact_cases)
    assert exact_case_status == 0
    exact_case_observation = decode(exact_case_payload)
    assert len(exact_case_observation.data_members) == 512
    assert len(exact_case_observation.cases) == 512
    overflowing_case_name_start = len(exact_cases) - 1 + len("case ")
    rejected(
        program,
        "case-capacity-plus-one",
        exact_cases[:-1] + b"case Overflow; }",
        23,
        (overflowing_case_name_start, overflowing_case_name_start + 8),
        data_member_count=512,
        case_count=512,
    )

    payload_prefix = b"data Payloads { case Many("
    exact_payload_body = b"".join(
        f"f{index}: T, ".encode() for index in range(1024)
    )
    exact_payload_fields = payload_prefix + exact_payload_body + b"); }"
    exact_payload_status, exact_payload = run(program, exact_payload_fields)
    assert exact_payload_status == 0
    exact_payload_observation = decode(exact_payload)
    assert len(exact_payload_observation.data_members) == 1
    assert len(exact_payload_observation.fields) == 0
    assert len(exact_payload_observation.payload_fields) == 1024
    assert len(exact_payload_observation.cases) == 1
    assert len(exact_payload_observation.type_references) == 1024
    overflow_payload_field_start = len(payload_prefix) + len(exact_payload_body)
    rejected(
        program,
        "case-payload-field-capacity-plus-one",
        payload_prefix + exact_payload_body + b"overflow: T); }",
        28,
        (overflow_payload_field_start, overflow_payload_field_start + 8),
        payload_field_count=1024,
        type_reference_count=1024,
    )

    exact_mixed = b"data Mixed { " + b"".join(
        f"f{index}: T; case C{index}; ".encode() for index in range(512)
    ) + b"}"
    exact_mixed_status, exact_mixed_payload = run(program, exact_mixed)
    assert exact_mixed_status == 0
    exact_mixed_observation = decode(exact_mixed_payload)
    assert len(exact_mixed_observation.data_members) == 1024
    assert len(exact_mixed_observation.fields) == 512
    assert len(exact_mixed_observation.cases) == 512
    overflowing_mixed_member_start = len(exact_mixed) - 1
    rejected(
        program,
        "mixed-member-capacity-plus-one",
        exact_mixed[:-1] + b"overflow: T; }",
        22,
        (overflowing_mixed_member_start, overflowing_mixed_member_start + 8),
        data_member_count=1024,
        field_count=512,
        case_count=512,
        type_reference_count=512,
    )

    # Machine declarations. Spans are derived from the authored source: `at`
    # names the `occurrence`-th spelling of a fragment.
    def at(source: bytes, fragment: bytes, occurrence: int = 1) -> tuple[int, int, int]:
        start = -1
        for _ in range(occurrence):
            start = source.index(fragment, start + 1)
        return (1, start, start + len(fragment))

    def named(source: bytes, name: bytes, occurrence: int = 1) -> tuple[int, ...]:
        span = at(source, name, occurrence)
        return (1, *span, *span, 0, 0, 0, 0)

    def qualified(source: bytes, name: bytes, domain: bytes) -> tuple[int, ...]:
        name_span = at(source, name)
        domain_span = at(source, domain)
        return (2, *name_span, 1, name_span[1], domain_span[2], 1, *domain_span)

    def reference(source: bytes, start: bytes, end_fragment: bytes, access: int, referee: int) -> tuple[int, ...]:
        start_span = at(source, start)
        end_span = at(source, end_fragment)
        return (3, 1, 0, 0, 1, start_span[1], end_span[2], 0, 1, 0, 0, access, referee)

    def machine_row(
        source: bytes,
        name: bytes,
        first_parameter: int,
        parameter_count: int,
        first_state: int,
        state_count: int,
        span: tuple[int, int],
        owner: bytes = b"",
        return_type: int = -1,
        public: bool = False,
    ) -> tuple[int, ...]:
        owner_span = at(source, owner) if owner else (1, 0, 0)
        return (
            1 if owner else 0,
            *owner_span,
            *at(source, name),
            first_parameter,
            parameter_count,
            0 if return_type < 0 else 1,
            max(return_type, 0),
            first_state,
            state_count,
            1 if public else 0,
            1,
            *span,
        )

    def parameter_row(source: bytes, name: bytes, type_reference: int, end_fragment: bytes) -> tuple[int, ...]:
        name_span = at(source, name)
        return (*name_span, type_reference, 1, name_span[1], at(source, end_fragment)[2])

    def state_row(
        source: bytes,
        name: bytes,
        first_parameter: int,
        parameter_count: int,
        span: tuple[int, int],
        return_type: int = -1,
    ) -> tuple[int, ...]:
        return (
            *at(source, name),
            first_parameter,
            parameter_count,
            0 if return_type < 0 else 1,
            max(return_type, 0),
            1,
            *span,
        )

    source = b"machine main() {}"
    accepted(
        program, "machine-empty", source,
        ((3, 0, 1, 0, 17),), (), (),
        machines=(machine_row(source, b"main", 0, 0, 0, 0, (0, 17)),),
    )
    source = b"pub machine Counter::clear(counter: &mut Counter) {}"
    accepted(
        program, "machine-owned-public-mutable-reference", source,
        ((3, 0, 1, 0, len(source)),), (), (),
        type_references=(
            named(source, b"Counter", 2),
            reference(source, b"&mut", b"&mut Counter", 2, 0),
        ),
        machines=(machine_row(source, b"clear", 0, 1, 0, 0, (0, len(source)), owner=b"Counter", public=True),),
        parameters=(parameter_row(source, b"counter", 1, b"&mut Counter"),),
    )
    source = b"machine add(left: u64, right: &u64) -> u64 {}"
    accepted(
        program, "machine-signature", source,
        ((3, 0, 1, 0, len(source)),), (), (),
        type_references=(
            named(source, b"u64", 1),
            named(source, b"u64", 2),
            reference(source, b"&u64", b"&u64", 1, 1),
            named(source, b"u64", 3),
        ),
        machines=(machine_row(source, b"add", 0, 2, 0, 0, (0, len(source)), return_type=3),),
        parameters=(
            parameter_row(source, b"left", 0, b"left: u64"),
            parameter_row(source, b"right", 2, b"&u64"),
        ),
    )
    source = b"machine tick {}"
    accepted(
        program, "machine-without-parameter-list", source,
        ((3, 0, 1, 0, len(source)),), (), (),
        machines=(machine_row(source, b"tick", 0, 0, 0, 0, (0, len(source))),),
    )
    source = b"machine run(n: u64) { state step(a: u8) -> u8 {} state halt {} }"
    accepted(
        program, "machine-states", source,
        ((3, 0, 1, 0, 64),), (), (),
        type_references=(named(source, b"u64"), named(source, b"u8", 1), named(source, b"u8", 2)),
        machines=((0, 1, 0, 0, 1, 8, 11, 0, 1, 0, 0, 0, 2, 0, 1, 0, 64),),
        parameters=((1, 12, 13, 0, 1, 12, 18), (1, 33, 34, 1, 1, 33, 38)),
        states=(
            (1, 28, 32, 1, 1, 1, 2, 1, 22, 48),
            (1, 55, 59, 2, 0, 0, 0, 1, 49, 62),
        ),
    )
    source = b"machine wrap(x: u64 in Wrapping) {}"
    accepted(
        program, "machine-qualified-parameter", source,
        ((3, 0, 1, 0, len(source)),), (), (),
        type_references=(qualified(source, b"u64", b"Wrapping"),),
        machines=(machine_row(source, b"wrap", 0, 1, 0, 0, (0, len(source))),),
        parameters=(parameter_row(source, b"x", 0, b"Wrapping"),),
    )
    source = b"use a; data D { x: u8; } machine m() {}"
    accepted(
        program, "machine-after-use-and-data", source,
        ((1, 0, 1, 0, 6), (2, 0, 1, 7, 24), (3, 0, 1, 25, len(source))),
        ((0, 1, 1, 0, 6),),
        ((1, 4, 5),),
        ((1, 12, 13, 0, 1, 0, 1, 0, 0, 0, False, 1, 7, 24, 0, 0),),
        ((1, 0, 1, 16, 22),),
        ((1, 16, 17, 0, 1, 16, 22),),
        type_references=(named(source, b"u8"),),
        machines=((0, 1, 0, 0, 1, 33, 34, 0, 0, 0, 0, 0, 0, 0, 1, 25, 39),),
    )
    source = b"data View { target: &Record; }"
    accepted(
        program, "data-field-reference-type", source,
        ((2, 0, 1, 0, len(source)),), (), (),
        ((1, 5, 9, 0, 1, 0, 1, 0, 0, 0, False, 1, 0, len(source), 0, 0),),
        ((1, 0, 1, 12, 28),),
        ((1, 12, 18, 1, 1, 12, 28),),
        type_references=(named(source, b"Record"), reference(source, b"&Record", b"&Record", 1, 0)),
    )

    rejected(program, "machine-name-at-end", b"machine", 40, (7, 7))
    rejected(program, "machine-name", b"machine () {}", 40, (8, 9))
    rejected(program, "machine-owned-name", b"machine A::() {}", 40, (11, 12))
    rejected(program, "machine-generics", b"machine f<T>() {}", 47, (9, 10))
    rejected(program, "machine-parameter-or-end-at-end", b"machine f(", 41, (10, 10))
    rejected(program, "machine-parameter-or-end", b"machine f(1) {}", 41, (10, 11))
    rejected(program, "machine-parameter-colon", b"machine f(a) {}", 43, (11, 12))
    rejected(program, "machine-parameter-type", b"machine f(a:) {}", 44, (12, 13))
    rejected(program, "machine-reference-referee", b"machine f(a: &) {}", 44, (14, 15))
    rejected(program, "machine-parameter-trailing-comma", b"machine f(a: u8,) {}", 42, (16, 17),
        parameter_count=1, type_reference_count=1)
    rejected(program, "machine-parameter-end", b"machine f(a: u8 b: u8) {}", 45, (16, 17),
        parameter_count=1, type_reference_count=1)
    rejected(program, "machine-parameter-domain", b"machine f(a: u8 in) {}", 31, (18, 19))
    rejected(program, "machine-return-type", b"machine f() -> {}", 46, (15, 16))
    rejected(program, "machine-bodyless", b"machine f();", 47, (11, 12))
    rejected(program, "machine-clause", b"machine f() reaches Console {}", 47, (12, 19))
    rejected(program, "machine-statement", b"machine f() { let x: u8; }", 48, (14, 17))
    rejected(program, "machine-body-end-at-end", b"machine f() {", 48, (13, 13))
    rejected(program, "state-name", b"machine f() { state }", 49, (20, 21))
    rejected(program, "state-body", b"machine f() { state s() }", 50, (24, 25))
    rejected(program, "state-body-end", b"machine f() { state s() { x } }", 51, (26, 27))

    exact_machines = b"".join(f"machine m{index}() {{}} ".encode() for index in range(256))
    exact_machine_status, exact_machine_payload = run(program, exact_machines)
    assert exact_machine_status == 0
    assert len(decode(exact_machine_payload).machines) == 256
    rejected(
        program,
        "machine-capacity-plus-one",
        exact_machines + b"machine overflow() {}",
        52,
        (len(exact_machines), len(exact_machines) + 7),
        root_count=256,
        machine_count=256,
    )
    exact_parameters = b"machine f(" + b", ".join(
        f"p{index}: T".encode() for index in range(1024)
    ) + b") {}"
    exact_parameter_status, exact_parameter_payload = run(program, exact_parameters)
    assert exact_parameter_status == 0
    assert len(decode(exact_parameter_payload).parameters) == 1024
    overflow_parameter = exact_parameters[:-4] + b", overflow: T) {}"
    overflow_parameter_start = len(exact_parameters) - 4 + 2
    rejected(
        program,
        "parameter-capacity-plus-one",
        overflow_parameter,
        53,
        (overflow_parameter_start, overflow_parameter_start + 8),
        parameter_count=1024,
        type_reference_count=1024,
    )
    exact_states = b"machine f() { " + b"".join(
        f"state s{index} {{}} ".encode() for index in range(1024)
    ) + b"}"
    exact_state_status, exact_state_payload = run(program, exact_states)
    assert exact_state_status == 0
    assert len(decode(exact_state_payload).states) == 1024
    overflow_state_start = len(exact_states) - 1
    rejected(
        program,
        "state-capacity-plus-one",
        exact_states[:-1] + b"state overflow {} }",
        54,
        (overflow_state_start, overflow_state_start + 5),
        state_count=1024,
    )

    repeat_source = (
        b"use repeated::observation; data Stable [linear] { "
        b"value: Value; case Ready; case Payload(left: L, right: R,); }"
    )
    first = run(program, repeat_source)
    second = run(program, repeat_source)
    assert first == second, "parser observation is not deterministic"

    print(f"Psi parser slices: {len(CASE_SOURCES)} cases passed")


if __name__ == "__main__":
    main()
