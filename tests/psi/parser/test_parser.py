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
    type_references: tuple[tuple[int, ...], ...]
    generic_parameters: tuple[
        tuple[int, int, int, int, int, int, int, int, int, int], ...
    ]
    machines: tuple[tuple[int, ...], ...] = ()
    parameters: tuple[tuple[int, ...], ...] = ()
    states: tuple[tuple[int, ...], ...] = ()
    expressions: tuple[tuple[int, ...], ...] = ()
    arguments: tuple[int, ...] = ()
    construct_fields: tuple[tuple[int, ...], ...] = ()
    statements: tuple[tuple[int, ...], ...] = ()
    arms: tuple[tuple[int, ...], ...] = ()
    pattern_fields: tuple[tuple[int, ...], ...] = ()
    clauses: tuple[tuple[int, ...], ...] = ()
    type_arguments: tuple[int, ...] = ()


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
            reader.u64(),
            reader.u64(),
        )
        for _ in range(reader.u64())
    )
    # kind, name span, source span, domain flag and span, access, then the
    # inner, length, first-argument, argument-count and first-row handles.
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
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
        )
        for _ in range(reader.u64())
    )
    type_arguments = tuple(reader.u64() for _ in range(reader.u64()))
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
            reader.u64(),
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
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
        )
        for _ in range(reader.u64())
    )
    expressions = tuple(
        (
            reader.byte(),
            reader.byte(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.byte(),
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
    arguments = tuple(reader.u64() for _ in range(reader.u64()))
    construct_fields = tuple(
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
    statements = tuple(
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
            reader.byte(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
            reader.u64(),
        )
        for _ in range(reader.u64())
    )
    arms = tuple(
        (
            reader.byte(),
            reader.u64(),
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
    pattern_fields = tuple(
        (
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
    clauses = tuple(
        (
            reader.byte(),
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
        expressions,
        arguments,
        construct_fields,
        statements,
        arms,
        pattern_fields,
        clauses,
        type_arguments,
    )


# Every distinct source the gate runs is one case; the determinism check runs
# its source twice and counts once.
CASE_SOURCES: set[bytes] = set()


def run(program: str, source: bytes) -> tuple[int, bytes]:
    CASE_SOURCES.add(source)
    completed = subprocess.run([program], input=source, capture_output=True, check=False)
    assert completed.stderr == b"", completed.stderr.decode(errors="replace")
    return completed.returncode, completed.stdout


def with_clause_range(rows: tuple[tuple, ...], width: int) -> tuple[tuple, ...]:
    """Rows spelled before data and cases owned clauses own an empty range
    at the table start."""
    return tuple(row if len(row) == width else row + (0, 0) for row in rows)


def type_rows(rows: tuple[tuple[int, ...], ...]) -> tuple[tuple[int, ...], ...]:
    """Complete rows spelled in the single-level shorthand: kind 1 named,
    2 qualified or 3 reference, the name, source and domain spans, then an
    optional access and referee. A qualified row is a named row with a
    domain, an unqualified row reports a zero domain span, a reference's
    referee is its inner row, and a leaf's run begins at itself. Full rows
    pass through."""
    completed: list[tuple[int, ...]] = []
    for index, row in enumerate(rows):
        if len(row) == 17:
            completed.append(row)
            continue
        row = row if len(row) == 13 else row + (0, 0)
        kind, name, span, has_domain, domain, access, referee = (
            row[0], row[1:4], row[4:7], row[7], row[8:11], row[11], row[12]
        )
        if not has_domain:
            domain = (0, 0, 0)
        if kind == 3:
            first_row = completed[referee][16]
            completed.append((3, *name, *span, has_domain, *domain, access, referee, 0, 0, 0, first_row))
        else:
            completed.append((1, *name, *span, has_domain, *domain, access, 0, 0, 0, 0, index))
    return tuple(completed)


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
    expressions: tuple[tuple[int, ...], ...] = (),
    arguments: tuple[int, ...] = (),
    construct_fields: tuple[tuple[int, ...], ...] = (),
    statements: tuple[tuple[int, ...], ...] = (),
    type_arguments: tuple[int, ...] = (),
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
        with_clause_range(data_items, 18),
        data_members,
        fields,
        payload_fields,
        with_clause_range(cases, 10),
        type_rows(type_references),
        generic_parameters,
        machines,
        parameters,
        states,
        expressions,
        arguments,
        construct_fields,
        statements,
        type_arguments=type_arguments,
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
    expression_count: int = 0,
    statement_count: int = 0,
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
    assert len(actual.expressions) == expression_count, (name, actual)
    assert len(actual.statements) == statement_count, (name, actual)


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
        ("missing-qualified-domain", b"data X { field: T in ; }", 31, (21, 22)),
        ("missing-qualified-domain-end", b"data X { field: T in", 31, (20, 20)),
        ("second-type-qualifier", b"data X { field: T in A in B; }", 14, (23, 25), 1),
        ("qualified-domain-union", b"data X { field: T in A | B; }", 14, (23, 24), 1),
        ("missing-payload-qualified-domain", b"data X { case A(v: T in ); }", 31, (24, 25)),
        ("empty-generic-list", b"data X<> {}", 32, (7, 8)),
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
        ("unterminated-generic-list", b"data X<T", 36, (8, 8)),
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

    exact_members = b"use " + b"::".join(f"n{i}".encode() for i in range(8192)) + b";"
    exact_member_status, exact_member_payload = run(program, exact_members)
    assert exact_member_status == 0
    assert len(decode(exact_member_payload).members) == 8192
    overflowing_member_start = len(exact_members) - 1 + 2
    rejected(
        program,
        "member-capacity-plus-one",
        exact_members[:-1] + b"::overflow;",
        7,
        (overflowing_member_start, overflowing_member_start + 8),
        member_count=8192,
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
        first_statement: int = 0,
        statement_count: int = 0,
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
            0,
            0,
            first_statement,
            statement_count,
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
        first_statement: int = 0,
        statement_count: int = 0,
        clause_range: tuple[int, int] = (0, 0),
    ) -> tuple[int, ...]:
        return (
            *at(source, name),
            first_parameter,
            parameter_count,
            0 if return_type < 0 else 1,
            max(return_type, 0),
            *clause_range,
            first_statement,
            statement_count,
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
        machines=((0, 1, 0, 0, 1, 8, 11, 0, 1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 1, 0, 64),),
        parameters=((1, 12, 13, 0, 1, 12, 18), (1, 33, 34, 1, 1, 33, 38)),
        states=(
            (1, 28, 32, 1, 1, 1, 2, 0, 0, 0, 0, 1, 22, 48),
            (1, 55, 59, 2, 0, 0, 0, 0, 0, 0, 0, 1, 49, 62),
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
        machines=((0, 1, 0, 0, 1, 33, 34, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 25, 39),),
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
    rejected(program, "machine-unknown-clause", b"machine f() decreases n {}", 47, (12, 21))
    rejected(program, "machine-unsupported-statement", b"machine f() { if x {} }", 48, (14, 16))
    rejected(program, "machine-body-end-at-end", b"machine f() {", 48, (13, 13))
    rejected(program, "state-name", b"machine f() { state }", 49, (20, 21))
    rejected(program, "state-body", b"machine f() { state s() }", 50, (24, 25))
    rejected(program, "state-unsupported-statement", b"machine f() { state s() { if } }", 51, (26, 28))

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
        f"p{index}: T".encode() for index in range(4096)
    ) + b") {}"
    exact_parameter_status, exact_parameter_payload = run(program, exact_parameters)
    assert exact_parameter_status == 0
    assert len(decode(exact_parameter_payload).parameters) == 4096
    overflow_parameter = exact_parameters[:-4] + b", overflow: T) {}"
    overflow_parameter_start = len(exact_parameters) - 4 + 2
    rejected(
        program,
        "parameter-capacity-plus-one",
        overflow_parameter,
        53,
        (overflow_parameter_start, overflow_parameter_start + 8),
        parameter_count=4096,
        type_reference_count=4096,
    )
    exact_states = b"machine f() { " + b"".join(
        f"state s{index} {{}} ".encode() for index in range(2048)
    ) + b"}"
    exact_state_status, exact_state_payload = run(program, exact_states)
    assert exact_state_status == 0
    assert len(decode(exact_state_payload).states) == 2048
    overflow_state_start = len(exact_states) - 1
    rejected(
        program,
        "state-capacity-plus-one",
        exact_states[:-1] + b"state overflow {} }",
        54,
        (overflow_state_start, overflow_state_start + 5),
        state_count=2048,
    )

    # Machine bodies. Each accepted case pins the four body tables exactly:
    # expression rows (kind, operator, left, right, first, count, flag, name
    # span, type handle, span), argument handles, construction fields and
    # statement rows (kind, name, mutability, type, value flag and handle,
    # target, crash cause, span). Operator tags follow OperatorKind order.
    def body(
        name: str,
        source: bytes,
        expressions: tuple[tuple[int, ...], ...],
        statements: tuple[tuple[int, ...], ...],
        arguments: tuple[int, ...] = (),
        construct_fields: tuple[tuple[int, ...], ...] = (),
        arms: tuple[tuple[int, ...], ...] = (),
        pattern_fields: tuple[tuple[int, ...], ...] = (),
    ) -> None:
        status, payload = run(program, source)
        assert status == 0, f"{name}: status {status}"
        actual = decode(payload)
        assert actual.expressions == expressions, f"{name}:\n{actual.expressions}"
        assert actual.arguments == arguments, f"{name}:\n{actual.arguments}"
        assert actual.construct_fields == construct_fields, f"{name}:\n{actual.construct_fields}"
        assert actual.statements == statements, f"{name}:\n{actual.statements}"
        assert actual.arms == arms, f"{name}:\n{actual.arms}"
        assert actual.pattern_fields == pattern_fields, f"{name}:\n{actual.pattern_fields}"

    body(
        "precedence-and-tail",
        b"machine f() -> u64 { 1 + 2 * 3 }",
        (
            (3, 0, 0, 0, 0, 0, 0, 1, 21, 22, 0, 1, 21, 22),
            (3, 0, 0, 0, 0, 0, 0, 1, 25, 26, 0, 1, 25, 26),
            (3, 0, 0, 0, 0, 0, 0, 1, 29, 30, 0, 1, 29, 30),
            (9, 16, 1, 2, 0, 0, 0, 1, 0, 0, 0, 1, 25, 30),
            (9, 14, 0, 3, 0, 0, 0, 1, 0, 0, 0, 1, 21, 30),
        ),
        ((4, 1, 0, 0, 0, 0, 1, 4, 0, 1, 0, 0, 1, 21, 30),),
    )
    body(
        "logical-tiers",
        b"machine f(a: bool, b: bool) -> bool { !a && b || a }",
        (
            (1, 0, 0, 0, 0, 1, 0, 1, 39, 40, 0, 1, 39, 40),
            (8, 19, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 38, 40),
            (1, 0, 0, 0, 1, 1, 0, 1, 44, 45, 0, 1, 44, 45),
            (9, 2, 1, 2, 0, 0, 0, 1, 0, 0, 0, 1, 38, 45),
            (1, 0, 0, 0, 2, 1, 0, 1, 49, 50, 0, 1, 49, 50),
            (9, 1, 3, 4, 0, 0, 0, 1, 0, 0, 0, 1, 38, 50),
        ),
        ((4, 1, 0, 0, 0, 0, 1, 5, 0, 1, 0, 0, 1, 38, 50),),
    )
    body(
        "field-index-assignment-and-path-call",
        b"machine f(p: &mut P) { p.x = q[i].y; Q::make(p, 1); }",
        (
            (1, 0, 0, 0, 0, 1, 0, 1, 23, 24, 0, 1, 23, 24),
            (6, 0, 0, 0, 0, 0, 0, 1, 25, 26, 0, 1, 23, 26),
            (1, 0, 0, 0, 1, 1, 0, 1, 29, 30, 0, 1, 29, 30),
            (1, 0, 0, 0, 2, 1, 0, 1, 31, 32, 0, 1, 31, 32),
            (7, 0, 2, 3, 0, 0, 0, 1, 0, 0, 0, 1, 29, 33),
            (6, 0, 4, 0, 0, 0, 0, 1, 34, 35, 0, 1, 29, 35),
            (2, 0, 0, 0, 3, 2, 0, 1, 40, 44, 0, 1, 37, 44),
            (1, 0, 0, 0, 5, 1, 0, 1, 45, 46, 0, 1, 45, 46),
            (3, 0, 0, 0, 0, 0, 0, 1, 48, 49, 0, 1, 48, 49),
            (11, 0, 6, 0, 0, 2, 0, 1, 0, 0, 0, 1, 37, 50),
        ),
        (
            (2, 1, 0, 0, 0, 0, 1, 5, 1, 1, 0, 0, 1, 23, 36),
            (3, 1, 0, 0, 0, 0, 1, 9, 0, 1, 0, 0, 1, 37, 51),
        ),
        arguments=(7, 8),
    )
    body(
        "nested-call-arguments-stay-contiguous",
        b"machine f() { g(h(1, 2), 3); }",
        (
            (1, 0, 0, 0, 0, 1, 0, 1, 14, 15, 0, 1, 14, 15),
            (1, 0, 0, 0, 1, 1, 0, 1, 16, 17, 0, 1, 16, 17),
            (3, 0, 0, 0, 0, 0, 0, 1, 18, 19, 0, 1, 18, 19),
            (3, 0, 0, 0, 0, 0, 0, 1, 21, 22, 0, 1, 21, 22),
            (11, 0, 1, 0, 0, 2, 0, 1, 0, 0, 0, 1, 16, 23),
            (3, 0, 0, 0, 0, 0, 0, 1, 25, 26, 0, 1, 25, 26),
            (11, 0, 0, 0, 2, 2, 0, 1, 0, 0, 0, 1, 14, 27),
        ),
        ((3, 1, 0, 0, 0, 0, 1, 6, 0, 1, 0, 0, 1, 14, 28),),
        arguments=(2, 3, 4, 5),
    )
    body(
        "group-and-qualified-cast",
        b"machine f() -> u8 { (1 + 2) as u8 in Wrapping }",
        (
            (3, 0, 0, 0, 0, 0, 0, 1, 21, 22, 0, 1, 21, 22),
            (3, 0, 0, 0, 0, 0, 0, 1, 25, 26, 0, 1, 25, 26),
            (9, 14, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 21, 26),
            (10, 0, 2, 0, 0, 0, 0, 1, 0, 0, 1, 1, 21, 45),
        ),
        ((4, 1, 0, 0, 0, 0, 1, 3, 0, 1, 0, 0, 1, 20, 45),),
    )
    body(
        "nested-construction-fields-stay-contiguous",
        b"machine f() { let v: V = V { a: 1, b: W { c: true } }; }",
        (
            (1, 0, 0, 0, 0, 1, 0, 1, 25, 26, 0, 1, 25, 26),
            (3, 0, 0, 0, 0, 0, 0, 1, 32, 33, 0, 1, 32, 33),
            (1, 0, 0, 0, 1, 1, 0, 1, 38, 39, 0, 1, 38, 39),
            (4, 0, 0, 0, 0, 0, 1, 1, 45, 49, 0, 1, 45, 49),
            (12, 0, 2, 0, 0, 1, 0, 1, 0, 0, 0, 1, 38, 51),
            (12, 0, 0, 0, 1, 2, 0, 1, 0, 0, 0, 1, 25, 53),
        ),
        ((1, 1, 18, 19, 0, 0, 1, 5, 0, 1, 0, 0, 1, 14, 54),),
        construct_fields=(
            (1, 42, 43, 3, 1, 42, 49),
            (1, 29, 30, 1, 1, 29, 33),
            (1, 35, 36, 4, 1, 35, 51),
        ),
    )
    body(
        "prefix-negation-and-subtraction",
        b"machine f(x: i64) -> i64 { -x - -1 }",
        (
            (1, 0, 0, 0, 0, 1, 0, 1, 28, 29, 0, 1, 28, 29),
            (8, 20, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 27, 29),
            (3, 0, 0, 0, 0, 0, 0, 1, 33, 34, 0, 1, 33, 34),
            (8, 20, 2, 0, 0, 0, 0, 1, 0, 0, 0, 1, 32, 34),
            (9, 15, 1, 3, 0, 0, 0, 1, 0, 0, 0, 1, 27, 34),
        ),
        ((4, 1, 0, 0, 0, 0, 1, 4, 0, 1, 0, 0, 1, 27, 34),),
    )
    body(
        "borrow-arguments",
        b"machine f() { g(&mut a, &b); }",
        (
            (1, 0, 0, 0, 0, 1, 0, 1, 14, 15, 0, 1, 14, 15),
            (1, 0, 0, 0, 1, 1, 0, 1, 21, 22, 0, 1, 21, 22),
            (8, 23, 1, 0, 0, 0, 0, 1, 0, 0, 0, 1, 16, 22),
            (1, 0, 0, 0, 2, 1, 0, 1, 25, 26, 0, 1, 25, 26),
            (8, 22, 3, 0, 0, 0, 0, 1, 0, 0, 0, 1, 24, 26),
            (11, 0, 0, 0, 0, 2, 0, 1, 0, 0, 0, 1, 14, 27),
        ),
        ((3, 1, 0, 0, 0, 0, 1, 5, 0, 1, 0, 0, 1, 14, 28),),
        arguments=(2, 4),
    )
    body(
        "mutable-local-and-local-named-mut",
        b"machine f() { let mut n: u8; let mut: u8; }",
        (),
        (
            (1, 1, 22, 23, 1, 0, 0, 0, 0, 1, 0, 0, 1, 14, 28),
            (1, 1, 33, 36, 0, 1, 0, 0, 0, 1, 0, 0, 1, 29, 41),
        ),
    )
    body(
        "string-argument-and-state-tail",
        b'machine f() { log("hi"); state s(a: u8) -> u8 { a } }',
        (
            (1, 0, 0, 0, 0, 1, 0, 1, 14, 17, 0, 1, 14, 17),
            (5, 0, 0, 0, 0, 0, 0, 1, 18, 22, 0, 1, 18, 22),
            (11, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 1, 14, 23),
            (1, 0, 0, 0, 1, 1, 0, 1, 48, 49, 0, 1, 48, 49),
        ),
        (
            (3, 1, 0, 0, 0, 0, 1, 2, 0, 1, 0, 0, 1, 14, 24),
            (4, 1, 0, 0, 0, 0, 1, 3, 0, 1, 0, 0, 1, 48, 49),
        ),
        arguments=(1,),
    )
    body("crash-abort", b"machine f() { crash Abort; }", (), ((5, 1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 1, 14, 26),))

    # Transitions: arm rows carry the pattern kind (guard 1, wildcard 2,
    # literal 3, case 4), pattern handle, field range, target call handle,
    # terminal flag and span; pattern fields carry a binder or a value.
    body(
        "wildcard-transition",
        b"machine f() { transition { _ -> g() } state g() {} }",
        (
            (1, 0, 0, 0, 0, 1, 0, 1, 32, 33, 0, 1, 32, 33),
            (11, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 32, 35),
        ),
        ((6, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 1, 14, 37),),
        arms=((2, 0, 0, 0, 1, 0, 1, 27, 35),),
    )
    body(
        "subject-literal-and-terminal-arms",
        b"machine f(x: u8) { transition x < 2 { true -> a(x) _ -> {} } state a(v: u8) {} }",
        (
            (1, 0, 0, 0, 0, 1, 0, 1, 30, 31, 0, 1, 30, 31),
            (3, 0, 0, 0, 0, 0, 0, 1, 34, 35, 0, 1, 34, 35),
            (9, 5, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 30, 35),
            (4, 0, 0, 0, 0, 0, 1, 1, 38, 42, 0, 1, 38, 42),
            (1, 0, 0, 0, 1, 1, 0, 1, 46, 47, 0, 1, 46, 47),
            (1, 0, 0, 0, 2, 1, 0, 1, 48, 49, 0, 1, 48, 49),
            (11, 0, 4, 0, 0, 1, 0, 1, 0, 0, 0, 1, 46, 50),
        ),
        ((6, 1, 0, 0, 0, 0, 1, 2, 0, 1, 0, 2, 1, 19, 60),),
        arguments=(5,),
        arms=((3, 3, 0, 0, 6, 0, 1, 38, 50), (2, 0, 0, 0, 0, 1, 1, 51, 58)),
    )
    body(
        "case-pattern-binders-and-values",
        b"machine f(t: Token) { transition t { Token::Identifier -> a() "
        b"Token::Keyword { keyword } -> b(keyword) "
        b"Token::Punctuation { punctuation: PunctuationKind::Comma } -> c() _ -> {} } }",
        (
            (1, 0, 0, 0, 0, 1, 0, 1, 33, 34, 0, 1, 33, 34),
            (2, 0, 0, 0, 1, 2, 0, 1, 44, 54, 0, 1, 37, 54),
            (1, 0, 0, 0, 3, 1, 0, 1, 58, 59, 0, 1, 58, 59),
            (11, 0, 2, 0, 0, 0, 0, 1, 0, 0, 0, 1, 58, 61),
            (2, 0, 0, 0, 4, 2, 0, 1, 69, 76, 0, 1, 62, 76),
            (1, 0, 0, 0, 6, 1, 0, 1, 92, 93, 0, 1, 92, 93),
            (1, 0, 0, 0, 7, 1, 0, 1, 94, 101, 0, 1, 94, 101),
            (11, 0, 5, 0, 0, 1, 0, 1, 0, 0, 0, 1, 92, 102),
            (2, 0, 0, 0, 8, 2, 0, 1, 110, 121, 0, 1, 103, 121),
            (2, 0, 0, 0, 10, 2, 0, 1, 154, 159, 0, 1, 137, 159),
            (1, 0, 0, 0, 12, 1, 0, 1, 165, 166, 0, 1, 165, 166),
            (11, 0, 10, 0, 1, 0, 0, 1, 0, 0, 0, 1, 165, 168),
        ),
        ((6, 1, 0, 0, 0, 0, 1, 0, 0, 1, 0, 4, 1, 22, 178),),
        arguments=(6,),
        arms=(
            (4, 1, 0, 0, 3, 0, 1, 37, 61),
            (4, 4, 0, 1, 7, 0, 1, 62, 102),
            (4, 8, 1, 1, 11, 0, 1, 103, 168),
            (2, 0, 0, 0, 0, 1, 1, 169, 176),
        ),
        pattern_fields=((1, 79, 86, 0, 0, 1, 79, 86), (1, 124, 135, 1, 9, 1, 124, 159)),
    )
    body(
        "guard-arms",
        b"machine f(n: u64) { transition { n == 0 -> a() n > 9 -> b(n - 1) _ -> {} } }",
        (
            (1, 0, 0, 0, 0, 1, 0, 1, 33, 34, 0, 1, 33, 34),
            (3, 0, 0, 0, 0, 0, 0, 1, 38, 39, 0, 1, 38, 39),
            (9, 3, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 33, 39),
            (1, 0, 0, 0, 1, 1, 0, 1, 43, 44, 0, 1, 43, 44),
            (11, 0, 3, 0, 0, 0, 0, 1, 0, 0, 0, 1, 43, 46),
            (1, 0, 0, 0, 2, 1, 0, 1, 47, 48, 0, 1, 47, 48),
            (3, 0, 0, 0, 0, 0, 0, 1, 51, 52, 0, 1, 51, 52),
            (9, 7, 5, 6, 0, 0, 0, 1, 0, 0, 0, 1, 47, 52),
            (1, 0, 0, 0, 3, 1, 0, 1, 56, 57, 0, 1, 56, 57),
            (1, 0, 0, 0, 4, 1, 0, 1, 58, 59, 0, 1, 58, 59),
            (3, 0, 0, 0, 0, 0, 0, 1, 62, 63, 0, 1, 62, 63),
            (9, 15, 9, 10, 0, 0, 0, 1, 0, 0, 0, 1, 58, 63),
            (11, 0, 8, 0, 0, 1, 0, 1, 0, 0, 0, 1, 56, 64),
        ),
        ((6, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 3, 1, 20, 74),),
        arguments=(11,),
        arms=(
            (1, 2, 0, 0, 4, 0, 1, 33, 46),
            (1, 7, 0, 0, 12, 0, 1, 47, 64),
            (2, 0, 0, 0, 0, 1, 1, 65, 72),
        ),
    )
    transition_rejections = (
        ("transition-subject", b"machine f() { transition }", 82, (25, 26), {}),
        ("transition-open", b"machine f() { transition x }", 83, (27, 28), {"member_count": 1, "expression_count": 1}),
        ("transition-arm", b"machine f() { transition { -> a() } }", 83, (27, 29), {}),
        ("transition-arrow", b"machine f() { transition { _ a() } }", 84, (29, 30), {}),
        ("transition-target", b"machine f() { transition { _ -> } }", 85, (32, 33), {}),
        ("transition-target-open", b"machine f() { transition { _ -> a } }", 86, (34, 35), {"member_count": 1, "expression_count": 1}),
        ("transition-terminal-close", b"machine f() { transition { _ -> { x } } }", 87, (34, 35), {}),
        ("transition-target-callee", b"machine f() { transition { _ -> a.b() } }", 85, (32, 35), {"member_count": 1, "expression_count": 3}),
        (
            "pattern-field",
            b"machine f(t: T) { transition t { A { , } -> a() } }",
            88,
            (37, 38),
            {"member_count": 2, "type_reference_count": 1, "parameter_count": 1, "expression_count": 2},
        ),
        (
            "pattern-field-end",
            b"machine f(t: T) { transition t { A { x y } -> a() } }",
            89,
            (39, 40),
            {"member_count": 2, "type_reference_count": 1, "parameter_count": 1, "expression_count": 2},
        ),
    )
    for name, source, diagnostic, span, counts in transition_rejections:
        rejected(program, name, source, diagnostic, span, **counts)

    # Machine clauses: rows carry the kind (reaches 1, invokes 2, suspends 3,
    # blocks 4, crashes 5, requires 6, ensures 7), member range, crash cause
    # tag, fact expression handle and span; the machine row keeps their range.
    def clauses(
        name: str,
        source: bytes,
        expected: tuple[tuple[int, ...], ...],
        members: tuple[tuple[int, int, int], ...],
        expression_count: int,
        clause_range: tuple[int, int],
    ) -> None:
        status, payload = run(program, source)
        assert status == 0, f"{name}: status {status}"
        actual = decode(payload)
        assert actual.clauses == expected, f"{name}:\n{actual.clauses}"
        assert actual.members == members, f"{name}:\n{actual.members}"
        assert len(actual.expressions) == expression_count, f"{name}:\n{actual.expressions}"
        assert actual.machines[0][11:13] == clause_range, f"{name}:\n{actual.machines}"

    clauses(
        "operational-clauses",
        b"machine f() reaches Console, Clock + Log invokes Console; crashes Trap blocks; suspends; {}",
        (
            (1, 0, 3, 1, 0, 1, 12, 40),
            (2, 3, 1, 1, 0, 1, 41, 57),
            (5, 0, 0, 1, 0, 1, 58, 70),
            (4, 4, 0, 1, 0, 1, 71, 78),
            (3, 4, 0, 1, 0, 1, 79, 88),
        ),
        ((1, 20, 27), (1, 29, 34), (1, 37, 40), (1, 49, 56)),
        0,
        (0, 5),
    )
    clauses(
        "fact-clauses",
        b"machine f(n: u64) requires n < 9 ensures n > 0 crashes Abort {}",
        (
            (6, 0, 0, 1, 2, 1, 18, 32),
            (7, 0, 0, 1, 5, 1, 33, 46),
            (5, 0, 0, 2, 0, 1, 47, 60),
        ),
        ((1, 27, 28), (1, 41, 42)),
        6,
        (0, 3),
    )
    clause_rejections = (
        ("reach-service", b"machine f() reaches {}", 92, (20, 21), {}),
        ("invokes-binding", b"machine f() invokes ;", 93, (20, 21), {}),
        ("invokes-end", b"machine f() invokes A {}", 94, (22, 23), {"member_count": 1}),
        ("blocks-end", b"machine f() blocks {}", 94, (19, 20), {}),
        ("requires-fact", b"machine f() requires {}", 95, (21, 22), {}),
        ("crashes-cause", b"machine f() crashes Halt {}", 79, (20, 24), {}),
    )
    for name, source, diagnostic, span, counts in clause_rejections:
        rejected(program, name, source, diagnostic, span, **counts)

    # A state's arrival `requires` facts share the clause table; a state
    # admits no other clause and a machine admits no `where`.
    source = b"machine f(n: u64) { state s(n: u64) requires n < 9 requires n > 0 {} }"
    status, payload = run(program, source)
    assert status == 0, f"state-requires: status {status}"
    actual = decode(payload)
    first_requires = source.index(b"requires")
    second_requires = source.index(b"requires", first_requires + 1)
    assert actual.clauses == (
        (6, 0, 0, 1, 2, 1, first_requires, source.index(b"9") + 1),
        (6, 0, 0, 1, 5, 1, second_requires, source.index(b"0 {") + 1),
    ), f"state-requires:\n{actual.clauses}"
    assert actual.states[0][7:9] == (0, 2), f"state-requires:\n{actual.states}"
    assert actual.machines[0][11:13] == (0, 0), f"state-requires:\n{actual.machines}"
    admission_rejections = (
        ("state-ensures", b"machine f() { state s ensures 1 {} }", 105, (22, 29), {}),
        ("state-reaches", b"machine f() { state s reaches A {} }", 105, (22, 29), {}),
        ("machine-where", b"machine f() where 1 {}", 105, (12, 17), {}),
    )
    for name, source, diagnostic, span, counts in admission_rejections:
        rejected(program, name, source, diagnostic, span, **counts)

    # Data and case `where` facts share the clause table with machines.
    source = b"data D where a <= 4, b > 0, { a: u64; b: u64; case C(x: u8) where x < 9; }"
    status, payload = run(program, source)
    assert status == 0, f"data-where: status {status}"
    actual = decode(payload)
    assert [row[0] for row in actual.clauses] == [8, 8, 8], f"data-where:\n{actual.clauses}"
    assert actual.data_items[0][16:18] == (0, 2), f"data-where:\n{actual.data_items}"
    assert actual.cases[0][8:10] == (2, 1), f"data-where:\n{actual.cases}"
    where_rejections = (
        ("where-fact", b"data D where {}", 95, (13, 14), {}),
        ("where-separator", b"data D where a b {}", 11, (15, 16), {"member_count": 1, "expression_count": 1}),
        ("data-requires", b"data D requires a {}", 105, (7, 15), {}),
        ("case-where-fact", b"data D { case C where ; }", 95, (22, 23), {}),
    )
    for name, source, diagnostic, span, counts in where_rejections:
        rejected(program, name, source, diagnostic, span, **counts)

    # `suspend [block]` and `block` acknowledge the call they prefix, as one
    # prefix node over it; followed by `(` either word is a name.
    def operators(source: bytes) -> list[tuple[int, int]]:
        status, payload = run(program, source)
        assert status == 0, f"{source!r}: status {status}"
        return [(row[0], row[1]) for row in decode(payload).expressions]

    UNARY = 8
    assert (UNARY, 25) in operators(b"machine f() { let x: u8 = block g(); }")
    assert (UNARY, 24) in operators(b"machine f() { suspend g(); }")
    assert (UNARY, 26) in operators(b"machine f() { transition { _ -> suspend block h(1) } }")
    assert all(row[0] != UNARY for row in operators(b"machine f() { block(x); suspend(y); }"))
    acknowledgement_rejections = (
        ("block-then-suspend", b"machine f() { block suspend g(); }", 107, (20, 27), {}),
        ("block-twice", b"machine f() { block block g(); }", 106, (20, 25), {}),
        ("suspend-twice", b"machine f() { suspend suspend g(); }", 106, (22, 29), {}),
        ("suspend-block-twice", b"machine f() { suspend block block g(); }", 106, (28, 33), {}),
    )
    for name, source, diagnostic, span, counts in acknowledgement_rejections:
        rejected(program, name, source, diagnostic, span, **counts)

    # Inside brackets a braced path always constructs, even in a transition
    # target where an unbracketed `{` would open a body.
    status, payload = run(program, b"machine f() { transition { _ -> g(V { a: 1 }) } }")
    assert status == 0, f"bracketed-construction: status {status}"
    assert len(decode(payload).construct_fields) == 1, "bracketed-construction"

    # A name or path applies static arguments when the matching `>` is
    # followed by `(`; otherwise `<` compares. Adjacent `>` `>` shift.
    APPLY, BINARY = 13, 9
    source = b"machine f() { Build::select_provider<a::Slot, Box<Item>>(builder); }"
    status, payload = run(program, source)
    assert status == 0, f"static-application: status {status}"
    actual = decode(payload)
    applications = [row for row in actual.expressions if row[0] == APPLY]
    assert len(applications) == 1 and applications[0][5] == 2, f"static-application:\n{actual.expressions}"
    names = [source[row[2]:row[3]] for row in actual.type_references if row[0] == 1]
    assert names == [b"a::Slot", b"Item", b"Box"], f"static-application:\n{names}"
    assert len(actual.type_arguments) == 3, f"static-application:\n{actual.type_arguments}"
    status, payload = run(program, b"machine f() { g<8>(x); }")
    assert status == 0, f"static-integer: status {status}"
    assert [row[0] for row in decode(payload).type_references] == [6], "static-integer"
    comparisons = operators(b"machine f() { g(a < b, c > d); h(e >> 2); }")
    assert {(BINARY, 5), (BINARY, 7), (BINARY, 13)} <= set(comparisons), comparisons
    assert all(row[0] != APPLY for row in comparisons), comparisons
    static_rejections = (
        ("static-argument-missing", b"machine f() { g<>(x); }", 101, (16, 17), {"member_count": 1, "expression_count": 1}),
        ("static-argument-trailing", b"machine f() { g<A,>(x); }", 101, (18, 19),
         {"member_count": 1, "expression_count": 1, "type_reference_count": 1}),
        ("type-path-member", b"data D { a: x::; }", 3, (15, 16), {}),
        ("spaced-shift", b"machine f() { h(k > > 1); }", 55, (20, 21), {"member_count": 2, "expression_count": 2}),
    )
    for name, source, diagnostic, span, counts in static_rejections:
        rejected(program, name, source, diagnostic, span, **counts)

    # Every keyword but `self` is a name in an expression, as in every other
    # name position.
    source = b"machine f() { g(target, data); transition { _ -> data(target) } state data(target: u8) {} }"
    status, payload = run(program, source)
    assert status == 0, f"keyword-names: status {status}"
    actual = decode(payload)
    assert [source[span[1]:span[2]] for span in actual.members] == [b"g", b"target", b"data", b"data", b"target"], (
        f"keyword-names:\n{actual.members}"
    )
    rejected(program, "self-is-not-a-name", b"machine f() { g(self); }", 60, (16, 20),
             member_count=1, expression_count=1)

    # Body rejections keep the nodes completed before the rejected token.
    body_rejections = (
        ("local-name", b"machine f() { let = 1; }", 71, (18, 19), {}),
        ("local-colon", b"machine f() { let x = 1; }", 72, (20, 21), {}),
        ("local-type", b"machine f() { let x: ; }", 73, (21, 22), {}),
        ("local-initial-value", b"machine f() { let x: u8 = ; }", 74, (26, 27), {"type_reference_count": 1}),
        ("local-end", b"machine f() { let x: u8 }", 75, (24, 25), {"type_reference_count": 1}),
        ("assignment-value", b"machine f() { a = ; }", 76, (18, 19), {"member_count": 1, "expression_count": 1}),
        ("assignment-semicolon", b"machine f() { a = 1 }", 77, (20, 21), {"member_count": 1, "expression_count": 2}),
        ("statement-end", b"machine f() { a b }", 78, (16, 17), {"member_count": 1, "expression_count": 1}),
        ("crash-cause", b"machine f() { crash Panic; }", 79, (20, 25), {}),
        ("crash-end", b"machine f() { crash Trap }", 80, (25, 26), {}),
        ("binary-operand", b"machine f() { 1 + ; }", 55, (18, 19), {"expression_count": 1}),
        ("path-member", b"machine f() { a::; }", 56, (17, 18), {"member_count": 1}),
        ("field-name", b"machine f() { a.; }", 57, (16, 17), {"member_count": 1, "expression_count": 1}),
        ("group-end", b"machine f() { (1; }", 58, (16, 17), {"expression_count": 1}),
        ("index-end", b"machine f() { a[1; }", 59, (17, 18), {"member_count": 1, "expression_count": 2}),
        ("call-argument", b"machine f() { g(,); }", 60, (16, 17), {"member_count": 1, "expression_count": 1}),
        ("call-argument-end", b"machine f() { g(1 2); }", 61, (18, 19), {"member_count": 1, "expression_count": 2}),
        ("cast-type", b"machine f() { a as ; }", 62, (19, 20), {"member_count": 1, "expression_count": 1}),
        ("construct-field", b"machine f() { V { 1 }; }", 63, (18, 19), {"member_count": 1, "expression_count": 1}),
        ("construct-colon", b"machine f() { V { a 1 }; }", 64, (20, 21), {"member_count": 1, "expression_count": 1}),
        ("construct-value", b"machine f() { V { a: }; }", 65, (21, 22), {"member_count": 1, "expression_count": 1}),
        ("construct-field-end", b"machine f() { V { a: 1 b: 2 }; }", 66, (23, 24), {"member_count": 1, "expression_count": 2}),
        ("statement-start", b"machine f() { ) }", 48, (14, 15), {}),
    )
    for name, source, diagnostic, span, counts in body_rejections:
        rejected(program, name, source, diagnostic, span, **counts)

    # Type forms. Rows follow their children, so a type fills one run that
    # ends at its root; each row records where its run begins.
    NAMED, REFERENCE, ARRAY, SLICE, LITERAL = 1, 3, 4, 5, 6

    def span_of(source: bytes, fragment: bytes, occurrence: int = 1) -> tuple[int, int]:
        start = -1
        for _ in range(occurrence):
            start = source.index(fragment, start + 1)
        return (start, start + len(fragment))

    def type_row(
        kind: int,
        span: tuple[int, int],
        first_row: int,
        name: tuple[int, int] | None = None,
        domain: tuple[int, int] | None = None,
        access: int = 0,
        inner: int = 0,
        length: int = 0,
        first_argument: int = 0,
        argument_count: int = 0,
    ) -> tuple[int, ...]:
        name_span = (1, *name) if name else (1, 0, 0)
        domain_part = (1, 1, *domain) if domain else (0, 0, 0, 0)
        return (kind, *name_span, 1, *span, *domain_part, access, inner, length,
                first_argument, argument_count, first_row)

    def leaf(kind: int, source: bytes, fragment: bytes, first_row: int, occurrence: int = 1) -> tuple[int, ...]:
        span = span_of(source, fragment, occurrence)
        return type_row(kind, span, first_row, name=span)

    def types(
        name: str,
        source: bytes,
        rows: tuple[tuple[int, ...], ...],
        type_arguments: tuple[int, ...] = (),
    ) -> None:
        status, payload = run(program, source)
        assert status == 0, f"{name}: status {status}"
        actual = decode(payload)
        assert actual.type_references == rows, f"{name}:\nexpected {rows}\nactual   {actual.type_references}"
        assert actual.type_arguments == type_arguments, f"{name}:\n{actual.type_arguments}"

    source = b"data D { a: [u8; 4]; }"
    types("array-literal-length", source, (
        leaf(NAMED, source, b"u8", 0),
        leaf(LITERAL, source, b"4", 1),
        type_row(ARRAY, span_of(source, b"[u8; 4]"), 0, inner=0, length=1),
    ))
    source = b"data D { a: [u64; 1024] in Trapping; }"
    types("array-domain", source, (
        leaf(NAMED, source, b"u64", 0),
        leaf(LITERAL, source, b"1024", 1),
        type_row(ARRAY, span_of(source, b"[u64; 1024] in Trapping"), 0,
                 domain=span_of(source, b"Trapping"), inner=0, length=1),
    ))
    source = b"machine f(bytes: &[u8]) {}"
    types("slice-reference", source, (
        leaf(NAMED, source, b"u8", 0),
        type_row(SLICE, span_of(source, b"[u8]"), 0, inner=0),
        type_row(REFERENCE, span_of(source, b"&[u8]"), 0, access=1, inner=1),
    ))
    source = b"machine f(cells: &mut [u8; 2]) {}"
    types("mutable-array-reference", source, (
        leaf(NAMED, source, b"u8", 0),
        leaf(LITERAL, source, b"2", 1),
        type_row(ARRAY, span_of(source, b"[u8; 2]"), 0, inner=0, length=1),
        type_row(REFERENCE, span_of(source, b"&mut [u8; 2]"), 0, access=2, inner=2),
    ))
    source = b"data D<const N: u64> { a: [u8; N]; }"
    types("named-length", source, (
        leaf(NAMED, source, b"u64", 0),
        leaf(NAMED, source, b"u8", 1),
        leaf(NAMED, source, b"N", 2, occurrence=2),
        type_row(ARRAY, span_of(source, b"[u8; N]"), 1, inner=1, length=2),
    ))
    source = b"data Main { console: Binding<Console>; }"
    types("application", source, (
        leaf(NAMED, source, b"Console", 0),
        type_row(NAMED, span_of(source, b"Binding<Console>"), 0, name=span_of(source, b"Binding"),
                 first_argument=0, argument_count=1),
    ), (0,))
    source = b"data D { a: A<B<C>>; }"
    split = span_of(source, b">>")[0]
    types("nested-application-split-close", source, (
        leaf(NAMED, source, b"C", 0),
        type_row(NAMED, (span_of(source, b"B<C")[0], split + 1), 0, name=span_of(source, b"B"),
                 first_argument=0, argument_count=1),
        type_row(NAMED, span_of(source, b"A<B<C>>"), 0, name=span_of(source, b"A"),
                 first_argument=1, argument_count=1),
    ), (0, 1))
    source = b"machine f(x: A<&B<C>>) {}"
    split = span_of(source, b">>")[0]
    types("reference-argument-split-close", source, (
        leaf(NAMED, source, b"C", 0),
        type_row(NAMED, (span_of(source, b"B<C")[0], split + 1), 0, name=span_of(source, b"B"),
                 first_argument=0, argument_count=1),
        type_row(REFERENCE, (span_of(source, b"&B")[0], split + 1), 0, access=1, inner=1),
        type_row(NAMED, span_of(source, b"A<&B<C>>"), 0, name=span_of(source, b"A"),
                 first_argument=1, argument_count=1),
    ), (0, 2))
    source = b"data D { a: AtMost<8> in Bounds; }"
    types("application-literal-domain", source, (
        leaf(LITERAL, source, b"8", 0),
        type_row(NAMED, span_of(source, b"AtMost<8> in Bounds"), 0, name=span_of(source, b"AtMost"),
                 domain=span_of(source, b"Bounds"), first_argument=0, argument_count=1),
    ), (0,))
    source = b"data D { a: [[u8; 2]; 3]; }"
    types("nested-arrays", source, (
        leaf(NAMED, source, b"u8", 0),
        leaf(LITERAL, source, b"2", 1),
        type_row(ARRAY, span_of(source, b"[u8; 2]"), 0, inner=0, length=1),
        leaf(LITERAL, source, b"3", 3),
        type_row(ARRAY, span_of(source, b"[[u8; 2]; 3]"), 0, inner=2, length=3),
    ))
    source = b"data D { a: Pair<u8, [u16; 2]>; }"
    types("two-arguments", source, (
        leaf(NAMED, source, b"u8", 0),
        leaf(NAMED, source, b"u16", 1),
        leaf(LITERAL, source, b"2", 2),
        type_row(ARRAY, span_of(source, b"[u16; 2]"), 1, inner=1, length=2),
        type_row(NAMED, span_of(source, b"Pair<u8, [u16; 2]>"), 0, name=span_of(source, b"Pair"),
                 first_argument=0, argument_count=2),
    ), (0, 3))

    too_deep = b"data D { a: " + b"[" * 17 + b"u8; }"
    capacity_arguments = b"data D { a: A<" + b"u8, " * 32 + b"u8>; }"
    type_rejections = (
        ("array-element", b"data D { a: [; }", 97, (13, 14), {}),
        ("array-separator", b"data D { a: [u8 x]; }", 98, (16, 17), {"type_reference_count": 1}),
        ("array-length", b"data D { a: [u8; ]; }", 99, (17, 18), {"type_reference_count": 1}),
        ("array-end", b"data D { a: [u8; 4 x]; }", 100, (19, 20), {"type_reference_count": 2}),
        ("static-argument", b"data D { a: A<>; }", 101, (14, 15), {}),
        ("static-argument-end", b"data D { a: A<B x>; }", 102, (16, 17), {"type_reference_count": 1}),
        ("extra-greater", b"data D { a: A<B>>; }", 14, (16, 17), {"type_reference_count": 2}),
        ("extra-greater-in-array", b"data D { a: [A<B>>; }", 98, (17, 18), {"type_reference_count": 2}),
        ("type-nesting", too_deep, 103, (29, 31), {}),
        ("type-argument-capacity", capacity_arguments, 104, span_of(capacity_arguments, b">"),
         {"type_reference_count": 33}),
        ("integer-type", b"data D { a: 5; }", 13, (12, 13), {}),
    )
    for name, source, diagnostic, span, counts in type_rejections:
        rejected(program, name, source, diagnostic, span, **counts)

    nested = b"machine f() { " + b"(" * 32 + b"1" + b")" * 32 + b"; }"
    nested_status, _ = run(program, nested)
    assert nested_status == 0, "32 nested groups"
    too_nested = b"machine f() { " + b"(" * 33 + b"1" + b")" * 33 + b"; }"
    rejected(program, "expression-nesting-plus-one", too_nested, 68, (46, 47))

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
