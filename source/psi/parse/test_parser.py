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
        tuple[int, int, int, int, int, int, int, int, int, int, bool, int, int, int],
        ...,
    ]
    data_members: tuple[tuple[int, int, int, int, int], ...]
    fields: tuple[tuple[int, int, int, int, int, int, int], ...]
    payload_fields: tuple[tuple[int, int, int, int, int, int, int], ...]
    cases: tuple[tuple[int, int, int, int, int, int, int, int], ...]
    type_references: tuple[tuple[int, int, int, int, int, int, int], ...]


@dataclass(frozen=True)
class LexTokenObservation:
    tag: int
    metadata: tuple[int, int, int]
    source: int
    start: int
    end: int
    raw: bytes
    decoded: bytes


@dataclass(frozen=True)
class LexObservation:
    accepted: bool
    diagnostic: int
    diagnostic_span: tuple[int, int, int]
    source: bytes
    tokens: tuple[LexTokenObservation, ...]


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
    assert reader.bytes(8) == b"OMGPAR5\0"
    assert reader.u64() == 5
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
    )


def decode_lex(payload: bytes) -> LexObservation:
    reader = Reader(payload)
    assert reader.bytes(8) == b"OMGLEX1\0"
    assert reader.u64() == 2
    accepted = reader.byte() == 1
    diagnostic = reader.byte()
    diagnostic_span = (reader.u64(), reader.u64(), reader.u64())
    source = reader.bytes(reader.u64())
    tokens = []
    for _ in range(reader.u64()):
        tag = reader.byte()
        metadata = (reader.byte(), reader.byte(), reader.byte())
        source_id = reader.u64()
        start = reader.u64()
        end = reader.u64()
        raw = reader.bytes(reader.u64())
        decoded = reader.bytes(reader.u64())
        tokens.append(
            LexTokenObservation(
                tag,
                metadata,
                source_id,
                start,
                end,
                raw,
                decoded,
            )
        )
    assert reader.cursor == len(payload), "trailing lexical observation bytes"
    return LexObservation(
        accepted,
        diagnostic,
        diagnostic_span,
        source,
        tuple(tokens),
    )


def run(program: str, source: bytes) -> tuple[int, bytes]:
    completed = subprocess.run([program], input=source, capture_output=True, check=False)
    assert completed.stderr == b"", completed.stderr.decode(errors="replace")
    return completed.returncode, completed.stdout


def lexical_parity(
    product_program: str,
    rust_observer: str,
    name: str,
    source: bytes,
) -> LexObservation:
    product_status, product_payload = run(product_program, b"\0" + source)
    observer_status, observer_payload = run(rust_observer, source)
    assert product_status in (0, 251, 252), (name, product_status)
    assert observer_status == 0, (name, observer_status)
    product = decode_lex(product_payload)
    reference = decode_lex(observer_payload)
    assert product == reference, (
        f"{name}: Omega/Rust lexical observation mismatch\n"
        f"Omega: {product}\nRust:  {reference}"
    )
    assert product_payload == observer_payload, (
        f"{name}: decoded observations agree but canonical bytes differ"
    )
    return product


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
    type_references: tuple[tuple[int, int, int, int, int, int, int], ...] = (),
    payload_fields: tuple[tuple[int, int, int, int, int, int, int], ...] = (),
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
    data_member_count: int = 0,
    field_count: int = 0,
    payload_field_count: int = 0,
    case_count: int = 0,
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
    assert len(actual.data_members) == data_member_count, (name, actual)
    assert len(actual.fields) == field_count, (name, actual)
    assert len(actual.payload_fields) == payload_field_count, (name, actual)
    assert len(actual.cases) == case_count, (name, actual)
    assert len(actual.type_references) == type_reference_count, (name, actual)


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit(
            "usage: test_parser.py <omega-product-program> <rust-lexer-observer>"
        )
    program = sys.argv[1]
    rust_observer = sys.argv[2]

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
        ((1, 5, 11, 0, 0, 0, 0, 0, 0, 2, False, 1, 0, 23),),
    )
    accepted(
        program,
        "linear-sum",
        b"data Sum [linear] { case Empty; }",
        ((2, 0, 1, 0, 33),),
        (),
        (),
        ((1, 5, 8, 0, 1, 0, 0, 0, 1, 2, False, 1, 0, 33),),
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
        ((1, 5, 10, 0, 2, 0, 1, 0, 1, 2, False, 1, 0, 45),),
        ((1, 0, 1, 22, 31), (2, 0, 1, 32, 43)),
        ((1, 22, 27, 0, 1, 22, 31),),
        ((1, 37, 42, 1, 32, 43, 0, 0),),
        ((1, 1, 29, 30, 1, 29, 30),),
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
            (1, 9, 14, 0, 0, 0, 0, 0, 0, 0, True, 1, 0, 17),
            (1, 23, 27, 0, 1, 0, 1, 0, 0, 0, False, 1, 18, 45),
            (1, 60, 64, 1, 3, 1, 3, 0, 0, 0, False, 1, 55, 116),
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
            (1, 1, 39, 42, 1, 39, 42),
            (1, 1, 76, 83, 1, 76, 83),
            (1, 1, 92, 97, 1, 92, 97),
            (1, 1, 107, 113, 1, 107, 113),
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
        ((1, 9, 14, 0, 4, 0, 2, 0, 2, 1, True, 1, 0, 76),),
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
            (1, 1, 42, 48, 1, 42, 48),
            (1, 1, 57, 62, 1, 57, 62),
        ),
    )
    accepted(
        program,
        "empty-case-payload",
        b"data Empty { case None(); }",
        ((2, 0, 1, 0, 27),),
        (),
        (),
        ((1, 5, 10, 0, 1, 0, 0, 0, 1, 0, False, 1, 0, 27),),
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
        ((1, 5, 8, 0, 1, 0, 0, 0, 1, 0, False, 1, 0, 33),),
        ((2, 0, 1, 11, 31),),
        (),
        ((1, 16, 20, 1, 11, 31, 0, 1),),
        ((1, 1, 28, 29, 1, 28, 29),),
        payload_fields=((1, 21, 26, 0, 1, 21, 29),),
    )
    accepted(
        program,
        "multiple-case-payload-fields",
        b"data Pair { case Both(left: L, right: R); }",
        ((2, 0, 1, 0, 43),),
        (),
        (),
        ((1, 5, 9, 0, 1, 0, 0, 0, 1, 0, False, 1, 0, 43),),
        ((2, 0, 1, 12, 41),),
        (),
        ((1, 17, 21, 1, 12, 41, 0, 2),),
        (
            (1, 1, 28, 29, 1, 28, 29),
            (1, 1, 38, 39, 1, 38, 39),
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
        ((1, 5, 13, 0, 1, 0, 0, 0, 1, 0, False, 1, 0, 39),),
        ((2, 0, 1, 16, 37),),
        (),
        ((1, 21, 25, 1, 16, 37, 0, 1),),
        ((1, 1, 33, 34, 1, 33, 34),),
        payload_fields=((1, 26, 31, 0, 1, 26, 35),),
    )
    accepted(
        program,
        "mixed-direct-and-case-payload-fields",
        b"data Mixed { common: C; case None; case Some(value: V); tail: T; }",
        ((2, 0, 1, 0, 66),),
        (),
        (),
        ((1, 5, 10, 0, 4, 0, 2, 0, 2, 0, False, 1, 0, 66),),
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
            (1, 1, 21, 22, 1, 21, 22),
            (1, 1, 52, 53, 1, 52, 53),
            (1, 1, 62, 63, 1, 62, 63),
        ),
        payload_fields=((1, 45, 50, 1, 1, 45, 53),),
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
        ("unknown-data-property", b"data X [unknown] {}", 18, (8, 15)),
        ("property-list-not-yet-supported", b"data X [copy,] {}", 19, (12, 13)),
        ("linear-property-list-not-yet-supported", b"data Bad [linear,] {}", 19, (16, 17)),
        ("duplicate-copy-property", b"data X [copy copy] {}", 19, (13, 17)),
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
        ),
        (
            "unterminated-case-payload",
            b"data X { case A(value: T",
            27,
            (24, 24),
        ),
        ("legacy-bare-case-is-rejected", b"data X { A; }", 12, (10, 11)),
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
    assert len(exact_field_observation.data_members) == 1024
    assert len(exact_field_observation.fields) == 1024
    assert len(exact_field_observation.type_references) == 1024
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

    lexical_cases = (
        ("lex-empty", b""),
        ("lex-ascii-identifiers", b"_ alpha Z9 snake_case"),
        ("lex-contextual-identifiers", b"case copy linear"),
        ("lex-exact-whitespace", b"a \t\r\nb"),
        ("lex-ascii-tokens", b"machine item 42 3.14 :: -> != && ||"),
        ("lex-nested-comment-payload", "/* café /* 变量 */ μέτρο */".encode()),
        ("lex-line-comment-payload", "// café 变量".encode()),
        ("lex-literal-payload", '"café😀"'.encode()),
        ("lex-fixed-byte-escapes", br'"\n\r\t\0\\\"\x41"'),
        ("lex-r-hash-number", b"r#1"),
        ("lex-unicode-identifier", "变量".encode()),
        ("lex-nonascii-identifier-tail", "café".encode()),
        ("lex-nonascii-number-tail", "1é".encode()),
        ("lex-vertical-tab", b"a\x0bb"),
        ("lex-form-feed", b"a\x0cb"),
        ("lex-nonbreaking-space", "a\u00a0b".encode()),
        ("lex-line-separator", "a\u2028b".encode()),
        ("lex-ideographic-space", "a\u3000b".encode()),
        ("lex-codepoint-escape", br'"\u{1f600}"'),
        ("lex-short-codepoint-escape", br'"\u"'),
        ("lex-empty-codepoint-escape", br'"\u{}"'),
        ("lex-raw-string", b'r"raw"'),
        ("lex-hashed-raw-string", b'r##"raw"##'),
        ("lex-lf-in-literal", b'"line\nvalue"'),
        ("lex-cr-in-literal", b'"line\rvalue"'),
        ("lex-apostrophe-escape", bytes((34, 92, 39, 34))),
        ("lex-unsupported-ascii", b"alpha @"),
        ("lex-invalid-utf8", b"ok\xfftail"),
        ("lex-unterminated-block-comment", b"/* open"),
        ("lex-invalid-hex-first", br'"\xG0"'),
        ("lex-invalid-hex-second", br'"\x0G"'),
        ("lex-unterminated-hex", br'"\x'),
    )
    lexical_observations = {
        name: lexical_parity(program, rust_observer, name, source)
        for name, source in lexical_cases
    }
    assert lexical_observations["lex-exact-whitespace"].accepted
    assert lexical_observations["lex-r-hash-number"].accepted
    assert lexical_observations["lex-exact-whitespace"].tokens[1].raw == b" \t\r\n"
    assert lexical_observations["lex-literal-payload"].tokens[0].decoded == "café😀".encode()
    assert lexical_observations["lex-nested-comment-payload"].tokens[0].raw == (
        "/* café /* 变量 */ μέτρο */".encode()
    )
    for name in (
        "lex-unicode-identifier",
        "lex-nonascii-identifier-tail",
        "lex-nonascii-number-tail",
        "lex-vertical-tab",
        "lex-form-feed",
        "lex-nonbreaking-space",
        "lex-line-separator",
        "lex-ideographic-space",
        "lex-codepoint-escape",
        "lex-short-codepoint-escape",
        "lex-empty-codepoint-escape",
        "lex-raw-string",
        "lex-hashed-raw-string",
        "lex-lf-in-literal",
        "lex-cr-in-literal",
        "lex-unsupported-ascii",
    ):
        assert lexical_observations[name].diagnostic == 2, name
    assert lexical_observations["lex-apostrophe-escape"].diagnostic == 6

    repeat_source = (
        b"use repeated::observation; data Stable [linear] { "
        b"value: Value; case Ready; case Payload(left: L, right: R,); }"
    )
    first = run(program, repeat_source)
    second = run(program, repeat_source)
    assert first == second, "parser observation is not deterministic"

    print(
        "Psi parser slices: 62 cases passed; "
        f"lexical profile parity: {len(lexical_cases)} cases passed"
    )


if __name__ == "__main__":
    main()
