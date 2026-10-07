#!/usr/bin/env python3
"""Black-box checks for the live Omega-written Psi name resolution slice."""

from __future__ import annotations

import struct
import subprocess
import sys
from dataclasses import dataclass

# Observation tags, mirrored from the harness's serialization tables.
UNRESOLVED_IMPORT = 1
DATA_SHADOWS_BUILTIN_TYPE = 2
DUPLICATE_DATA = 3
DUPLICATE_DATA_MEMBER = 4
DUPLICATE_CASE_PAYLOAD_FIELD = 5
PAYLOAD_FIELD_COLLIDES_WITH_COMMON_FIELD = 6
UNKNOWN_DATA_TYPE = 7
UNKNOWN_DOMAIN = 8
DUPLICATE_PARAMETER = 9
DUPLICATE_STATE = 10
DUPLICATE_MACHINE = 11

BUILTIN = 1
DATA = 2
GENERIC = 3
REFERENCE = 4

UNQUALIFIED = 0
WRAPPING = 1
SATURATING = 2
TRAPPING = 3
FINITE = 4

BUILTIN_SPELLINGS = (
    "bool", "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "addr",
    "f32", "f64", "Slice", "Result", "SyscallResult", "Terminal", "Never",
    "UInt", "Int", "AtomicBool", "AtomicU32", "AtomicU64",
)


def builtin(spelling: str, domain: int = UNQUALIFIED) -> tuple[int, int, int]:
    return (BUILTIN, BUILTIN_SPELLINGS.index(spelling), domain)


def data(handle: int, domain: int = UNQUALIFIED) -> tuple[int, int, int]:
    return (DATA, handle, domain)


def generic(handle: int, domain: int = UNQUALIFIED) -> tuple[int, int, int]:
    return (GENERIC, handle, domain)


REFERENCE_ROW = (REFERENCE, 0, UNQUALIFIED)


@dataclass(frozen=True)
class Observation:
    accepted: bool
    diagnostic: int
    diagnostic_span: tuple[int, int, int]
    bindings: tuple[tuple[int, int, int], ...]


class Reader:
    def __init__(self, payload: bytes) -> None:
        self.payload = payload
        self.cursor = 0

    def bytes(self, count: int) -> bytes:
        end = self.cursor + count
        if end > len(self.payload):
            raise AssertionError("truncated resolution observation")
        value = self.payload[self.cursor:end]
        self.cursor = end
        return value

    def byte(self) -> int:
        return self.bytes(1)[0]

    def u64(self) -> int:
        return struct.unpack("<Q", self.bytes(8))[0]


def decode(payload: bytes) -> Observation:
    reader = Reader(payload)
    assert reader.bytes(8) == b"OMGRES1\0"
    assert reader.u64() == 1
    accepted = reader.byte() == 1
    diagnostic = reader.byte()
    diagnostic_span = (reader.u64(), reader.u64(), reader.u64())
    bindings = tuple(
        (reader.byte(), reader.u64(), reader.byte())
        for _ in range(reader.u64())
    )
    assert reader.cursor == len(payload), "trailing resolution observation bytes"
    return Observation(accepted, diagnostic, diagnostic_span, bindings)


# Every distinct source the gate runs is one case; the determinism check runs
# its source twice and counts once.
CASE_SOURCES: set[bytes] = set()


def run(program: str, source: bytes) -> tuple[int, bytes]:
    CASE_SOURCES.add(source)
    completed = subprocess.run([program], input=source, capture_output=True, check=False)
    assert completed.stderr == b"", completed.stderr.decode(errors="replace")
    return completed.returncode, completed.stdout


def accepted(
    program: str,
    name: str,
    source: bytes,
    bindings: tuple[tuple[int, int, int], ...],
) -> None:
    status, payload = run(program, source)
    assert status == 0, f"{name}: status {status}"
    actual = decode(payload)
    expected = Observation(True, 0, (1, 0, 0), bindings)
    assert actual == expected, f"{name}:\nexpected {expected}\nactual   {actual}"


def rejected(
    program: str,
    name: str,
    source: bytes,
    diagnostic: int,
    spelling: bytes,
    occurrence: int = 1,
    length: int | None = None,
) -> None:
    """The diagnostic span starts at the `occurrence`-th authored `spelling`
    and covers it, or its first `length` bytes."""
    start = -1
    for _ in range(occurrence):
        start = source.index(spelling, start + 1)
    end = start + (len(spelling) if length is None else length)
    status, payload = run(program, source)
    assert status == 249, f"{name}: status {status}"
    actual = decode(payload)
    expected = Observation(False, diagnostic, (1, start, end), ())
    assert actual == expected, f"{name}:\nexpected {expected}\nactual   {actual}"


def phase_rejected(program: str, name: str, source: bytes, status: int) -> None:
    actual_status, payload = run(program, source)
    assert actual_status == status, f"{name}: status {actual_status}"
    assert payload == b"", name


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: test_resolver.py <omega-resolver-harness>")
    program = sys.argv[1]

    accepted(program, "empty", b"", ())
    every_builtin = b"data A {" + b"".join(
        f" f{index}: {spelling};".encode() for index, spelling in enumerate(BUILTIN_SPELLINGS)
    ) + b" }"
    accepted(
        program,
        "every-builtin-type",
        every_builtin,
        tuple(builtin(spelling) for spelling in BUILTIN_SPELLINGS),
    )
    accepted(
        program,
        "forward-backward-and-self-data",
        b"data A { b: B; s: A; }\ndata B { x: u8; }",
        (data(1), data(0), builtin("u8")),
    )
    accepted(
        program,
        "generic-parameters-of-every-kind",
        b"data A<T, const N: u64, u: u32> { x: T; n: N; v: u; }",
        (builtin("u64"), builtin("u32"), generic(0), generic(1), generic(2)),
    )
    accepted(
        program,
        "generic-parameter-shadows-data",
        b"data T { y: u8; }\ndata A<T> { x: T; }",
        (builtin("u8"), generic(0)),
    )
    accepted(
        program,
        "generic-scope-per-declaration",
        b"data A<T> { x: T; }\ndata B<T> { y: T; }",
        (generic(0), generic(1)),
    )
    accepted(
        program,
        "data-named-like-its-parameter",
        b"data T<T> { x: T; }",
        (generic(0),),
    )
    accepted(
        program,
        "lifetime-parameters-are-skipped",
        b"data A<'a, T> { x: T; }",
        (generic(1),),
    )
    accepted(
        program,
        "case-payload-types",
        b"data A<T> { case B(v: T, w: A); case C; }",
        (generic(0), data(0)),
    )
    accepted(
        program,
        "compiler-known-domains",
        b"data A { a: u64 in Wrapping; b: u64 in Saturating; c: i8 in Trapping; d: f64 in Finite; }",
        (
            builtin("u64", WRAPPING),
            builtin("u64", SATURATING),
            builtin("i8", TRAPPING),
            builtin("f64", FINITE),
        ),
    )
    accepted(
        program,
        "qualified-annotation-and-payload",
        b"data A<const N: u64 in Trapping> { case B(v: u8 in Wrapping); }",
        (builtin("u64", TRAPPING), builtin("u8", WRAPPING)),
    )
    accepted(
        program,
        "domain-names-are-not-type-names",
        b"data Wrapping { x: u8; }\ndata A { w: Wrapping; y: u8 in Wrapping; }",
        (builtin("u8"), data(0), builtin("u8", WRAPPING)),
    )
    accepted(
        program,
        "separate-member-namespaces",
        b"data A<T> { T: u8; tag: u8; case A; case B(v: u16, B: u16); case C(v: u16); }",
        (builtin("u8"), builtin("u8"), builtin("u16"), builtin("u16"), builtin("u16")),
    )

    rejected(program, "use-outside-source-set", b"use a;", UNRESOLVED_IMPORT, b"use a;")
    rejected(
        program,
        "import-before-declarations",
        b"use a::b; data A { x: Q; }",
        UNRESOLVED_IMPORT,
        b"use a::b;",
    )
    rejected(program, "data-shadows-u8", b"data u8 { x: bool; }", DATA_SHADOWS_BUILTIN_TYPE, b"u8")
    rejected(
        program,
        "data-shadows-last-builtin",
        b"data AtomicU64 { x: bool; }",
        DATA_SHADOWS_BUILTIN_TYPE,
        b"AtomicU64",
    )
    rejected(
        program,
        "duplicate-data",
        b"data A { x: u8; }\ndata A { y: u8; }",
        DUPLICATE_DATA,
        b"A",
        2,
    )
    rejected(
        program,
        "duplicate-data-before-references",
        b"data A { x: Q; }\ndata B { y: u8; }\ndata B { z: u8; }",
        DUPLICATE_DATA,
        b"B",
        2,
    )
    rejected(
        program,
        "duplicate-field",
        b"data A { x: u8; x: u16; }",
        DUPLICATE_DATA_MEMBER,
        b"x",
        2,
    )
    rejected(
        program,
        "duplicate-case",
        b"data A { case B; case B; }",
        DUPLICATE_DATA_MEMBER,
        b"B",
        2,
    )
    rejected(
        program,
        "case-after-field",
        b"data A { x: u8; case x; }",
        DUPLICATE_DATA_MEMBER,
        b"x",
        2,
    )
    rejected(
        program,
        "field-after-case",
        b"data A { case x; x: u8; }",
        DUPLICATE_DATA_MEMBER,
        b"x",
        2,
    )
    rejected(
        program,
        "duplicate-payload-field",
        b"data A { case B(v: u8, v: u8); }",
        DUPLICATE_CASE_PAYLOAD_FIELD,
        b"v",
        2,
    )
    rejected(
        program,
        "payload-field-collides-with-common-field",
        b"data A { v: u8; case B(v: u8); }",
        PAYLOAD_FIELD_COLLIDES_WITH_COMMON_FIELD,
        b"v",
        2,
    )
    rejected(program, "unknown-field-type", b"data A { x: Q; }", UNKNOWN_DATA_TYPE, b"Q")
    rejected(program, "usize-is-not-builtin", b"data A { x: usize; }", UNKNOWN_DATA_TYPE, b"usize")
    rejected(program, "builtin-prefix", b"data A { x: boo; }", UNKNOWN_DATA_TYPE, b"boo")
    rejected(program, "builtin-extension", b"data A { x: u88; }", UNKNOWN_DATA_TYPE, b"u88")
    rejected(program, "builtin-case", b"data A { x: Bool; }", UNKNOWN_DATA_TYPE, b"Bool")
    rejected(program, "lifetime-is-not-a-type", b"data Z<'q> { x: q; }", UNKNOWN_DATA_TYPE, b"q", 2)
    rejected(
        program,
        "unknown-payload-type",
        b"data A { case B(v: Q); }",
        UNKNOWN_DATA_TYPE,
        b"Q",
    )
    rejected(
        program,
        "unknown-generic-annotation",
        b"data A<const N: Q> { x: u8; }",
        UNKNOWN_DATA_TYPE,
        b"Q",
    )
    rejected(
        program,
        "generic-scope-does-not-leak",
        b"data A<T> { x: u8; }\ndata B { y: T; }",
        UNKNOWN_DATA_TYPE,
        b"T",
        2,
    )
    rejected(program, "unknown-domain", b"data A { x: u64 in Foo; }", UNKNOWN_DOMAIN, b"Foo")
    rejected(
        program,
        "parameter-is-not-a-domain",
        b"data A<T> { x: u64 in T; }",
        UNKNOWN_DOMAIN,
        b"T",
        2,
    )
    rejected(
        program,
        "first-reference-diagnostic-domain",
        b"data A { x: u8 in Foo; y: Q; }",
        UNKNOWN_DOMAIN,
        b"Foo",
    )
    rejected(
        program,
        "first-reference-diagnostic-type",
        b"data A { y: Q; x: u8 in Foo; }",
        UNKNOWN_DATA_TYPE,
        b"Q",
    )

    # Machine signatures resolve in the empty scope; references bind their
    # referee row. Rows appear in parse order.
    accepted(
        program,
        "machine-signature-types",
        b"machine f(a: u8, b: &mut D) -> u64 { state s(x: D) {} }\ndata D { y: u8; }",
        (builtin("u8"), data(0), REFERENCE_ROW, builtin("u64"), data(0), builtin("u8")),
    )
    accepted(
        program,
        "data-field-reference",
        b"data View { target: &Record; }\ndata Record { x: u8; }",
        (data(1), REFERENCE_ROW, builtin("u8")),
    )
    accepted(
        program,
        "overloads-by-parameter-type",
        b"machine f(a: u8) {} machine f(a: u16) {}",
        (builtin("u8"), builtin("u16")),
    )
    accepted(
        program,
        "overloads-by-access-and-domain",
        b"machine f(a: &u8) {} machine f(a: &mut u8) {} machine g(a: u8 in Wrapping) {} machine g(a: u8) {}",
        (builtin("u8"), REFERENCE_ROW, builtin("u8"), REFERENCE_ROW, builtin("u8", WRAPPING), builtin("u8")),
    )
    accepted(
        program,
        "machine-owners-separate-names",
        b"machine A::f() {} machine B::f() {} machine f() {}",
        (),
    )
    accepted(
        program,
        "state-repeats-machine-and-parameter-names",
        b"machine f(a: u8) { state f(a: u8) {} }",
        (builtin("u8"), builtin("u8")),
    )
    rejected(program, "unknown-parameter-type", b"machine f(a: Q) {}", UNKNOWN_DATA_TYPE, b"Q")
    rejected(program, "unknown-return-type", b"machine f() -> Q {}", UNKNOWN_DATA_TYPE, b"Q")
    rejected(program, "unknown-state-parameter-type", b"machine f() { state s(x: Q) {} }", UNKNOWN_DATA_TYPE, b"Q")
    rejected(program, "unknown-referee", b"machine f(a: &Q) {}", UNKNOWN_DATA_TYPE, b"Q")
    rejected(
        program,
        "duplicate-parameter",
        b"machine f(a: u8, a: u16) {}",
        DUPLICATE_PARAMETER,
        b"a:",
        2,
        length=1,
    )
    rejected(
        program,
        "duplicate-state-parameter",
        b"machine f() { state s(x: u8, x: u8) {} }",
        DUPLICATE_PARAMETER,
        b"x",
        2,
    )
    rejected(
        program,
        "duplicate-state",
        b"machine f() { state s {} state s {} }",
        DUPLICATE_STATE,
        b"s {}",
        2,
        length=1,
    )
    rejected(
        program,
        "duplicate-overload",
        b"machine f(a: u8) {} machine f(b: u8) {}",
        DUPLICATE_MACHINE,
        b"f(",
        2,
        length=1,
    )
    rejected(
        program,
        "duplicate-owned-machine",
        b"machine A::f() {} machine A::f() {}",
        DUPLICATE_MACHINE,
        b"f(",
        2,
        length=1,
    )
    rejected(
        program,
        "types-before-machine-names",
        b"machine f(a: Q, a: u8) {}",
        UNKNOWN_DATA_TYPE,
        b"Q",
    )

    # Body types: `let` types and cast targets bind after every signature,
    # in the empty scope.
    accepted(
        program,
        "local-and-cast-types",
        b"machine f() { let x: u8 = 1 as u16 in Wrapping; let r: &D = g(); }\ndata D { y: u8; }",
        (builtin("u8"), builtin("u16", WRAPPING), data(0), REFERENCE_ROW, builtin("u8")),
    )
    rejected(program, "unknown-local-type", b"machine f() { let x: Q; }", UNKNOWN_DATA_TYPE, b"Q")
    rejected(program, "unknown-cast-type", b"machine f() { g(1 as Q); }", UNKNOWN_DATA_TYPE, b"Q")
    rejected(
        program,
        "signature-types-before-body-types",
        b"machine f(a: P) { let x: Q; }",
        UNKNOWN_DATA_TYPE,
        b"P",
    )

    phase_rejected(program, "parse-rejection-has-no-observation", b"data A { x: ; }", 250)
    phase_rejected(program, "lex-rejection-has-no-observation", b"\x80", 251)

    determinism = b"data A<T> { x: T; y: B in Wrapping; }\ndata B { case C(v: A); }"
    first = run(program, determinism)
    second = run(program, determinism)
    assert first == second, "determinism"
    assert first[0] == 0, first

    print(f"Psi resolver slices: {len(CASE_SOURCES)} cases passed")


if __name__ == "__main__":
    main()
