#!/usr/bin/env python3
"""Black-box checks for the live Omega-written Psi name resolution slice."""

from __future__ import annotations

import struct
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

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
DUPLICATE_GENERIC_PARAMETER = 12
UNKNOWN_NAME = 13
AMBIGUOUS_NAME = 14

# Value bindings: (tag, handle, case handle).
NOT_A_NAME = (0, 0, 0)


def local(statement: int) -> tuple[int, int, int]:
    return (2, statement, 0)


def parameter(handle: int) -> tuple[int, int, int]:
    return (3, handle, 0)


def binder(field: int) -> tuple[int, int, int]:
    return (4, field, 0)


def state(handle: int) -> tuple[int, int, int]:
    return (5, handle, 0)


def machine(handle: int) -> tuple[int, int, int]:
    return (6, handle, 0)


def data_case(item: int, handle: int) -> tuple[int, int, int]:
    return (8, item, handle)

BUILTIN = 1
DATA = 2
GENERIC = 3
REFERENCE = 4
ARRAY = 5
SLICE = 6
LITERAL = 7

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
SLICE_ROW = (SLICE, 0, UNQUALIFIED)
LITERAL_ROW = (LITERAL, 0, UNQUALIFIED)


def array(domain: int = UNQUALIFIED) -> tuple[int, int, int]:
    return (ARRAY, 0, domain)


@dataclass(frozen=True)
class Observation:
    accepted: bool
    diagnostic: int
    diagnostic_span: tuple[int, int, int]
    bindings: tuple[tuple[int, int, int], ...]
    value_bindings: tuple[tuple[int, int, int], ...] = ()


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
    value_bindings = tuple(
        (reader.byte(), reader.u64(), reader.u64())
        for _ in range(reader.u64())
    )
    assert reader.cursor == len(payload), "trailing resolution observation bytes"
    return Observation(accepted, diagnostic, diagnostic_span, bindings, value_bindings)


# Every distinct input the gate runs is one case; the determinism check runs
# its input twice and counts once.
CASE_SOURCES: set[bytes] = set()


def frame(units: tuple[tuple[bytes, bytes], ...]) -> bytes:
    """The harness input: a unit count, then each unit's module path and
    source, `u32`-framed. A unit's position is its source id."""
    payload = struct.pack("<I", len(units))
    for module_path, source in units:
        payload += struct.pack("<I", len(module_path)) + module_path
        payload += struct.pack("<I", len(source)) + source
    return payload


def run_units(program: str, units: tuple[tuple[bytes, bytes], ...]) -> tuple[int, bytes]:
    payload = frame(units)
    CASE_SOURCES.add(payload)
    completed = subprocess.run([program], input=payload, capture_output=True, check=False)
    assert completed.stderr == b"", completed.stderr.decode(errors="replace")
    return completed.returncode, completed.stdout


def run(program: str, source: bytes) -> tuple[int, bytes]:
    """One source unit, the root module."""
    return run_units(program, ((b"", source),))


def accepted(
    program: str,
    name: str,
    source: bytes,
    bindings: tuple[tuple[int, int, int], ...],
    value_bindings: tuple[tuple[int, int, int], ...] | None = None,
) -> None:
    """Value bindings are compared only when the case names them."""
    status, payload = run(program, source)
    assert status == 0, f"{name}: status {status}"
    actual = decode(payload)
    if value_bindings is None:
        actual = Observation(actual.accepted, actual.diagnostic, actual.diagnostic_span, actual.bindings)
    expected = Observation(True, 0, (0, 0, 0), bindings, value_bindings or ())
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
    expected = Observation(False, diagnostic, (0, start, end), (), ())
    assert actual == expected, f"{name}:\nexpected {expected}\nactual   {actual}"


def accepted_units(
    program: str,
    name: str,
    units: tuple[tuple[bytes, bytes], ...],
    bindings: tuple[tuple[int, int, int], ...],
    value_bindings: tuple[tuple[int, int, int], ...] | None = None,
) -> None:
    status, payload = run_units(program, units)
    assert status == 0, f"{name}: status {status}"
    actual = decode(payload)
    if value_bindings is None:
        actual = Observation(actual.accepted, actual.diagnostic, actual.diagnostic_span, actual.bindings)
    expected = Observation(True, 0, (0, 0, 0), bindings, value_bindings or ())
    assert actual == expected, f"{name}:\nexpected {expected}\nactual   {actual}"


def rejected_units(
    program: str,
    name: str,
    units: tuple[tuple[bytes, bytes], ...],
    diagnostic: int,
    unit: int,
    spelling: bytes,
    occurrence: int = 1,
) -> None:
    """The diagnostic names the `occurrence`-th `spelling` of unit `unit`."""
    source = units[unit][1]
    start = -1
    for _ in range(occurrence):
        start = source.index(spelling, start + 1)
    status, payload = run_units(program, units)
    assert status == 249, f"{name}: status {status}"
    actual = decode(payload)
    expected = Observation(False, diagnostic, (unit, start, start + len(spelling)), (), ())
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

    rejected(program, "use-of-an-unsupplied-module", b"use a;", UNRESOLVED_IMPORT, b"use a;")
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
        b"machine f() { let x: u8 = 1 as u16 in Wrapping; let r: &D = g(); }\ndata D { y: u8; }\nmachine g() {}",
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

    # Nested types: every row of a type's run binds in the owner's scope;
    # names keep their declarations and forms bind by form.
    accepted(
        program,
        "array-slice-and-application-rows",
        b"data A<T, const N: u64> { b: Box<T>; c: [u8; N] in Trapping; d: &mut [A<u8, 4>]; }\n"
        b"data Box<T> { v: T; }",
        (
            builtin("u64"),
            generic(0), data(1),
            builtin("u8"), generic(1), array(TRAPPING),
            builtin("u8"), LITERAL_ROW, data(0), SLICE_ROW, REFERENCE_ROW,
            generic(2),
        ),
    )
    rejected(program, "unknown-array-element", b"data A { x: [Missing; 4]; }", UNKNOWN_DATA_TYPE, b"Missing")
    rejected(
        program,
        "unknown-static-argument",
        b"data A { x: Box<Missing>; }\ndata Box<T> { v: T; }",
        UNKNOWN_DATA_TYPE,
        b"Missing",
    )
    accepted(
        program,
        "overloads-differ-inside-types",
        b"machine f(a: [u8; 4]) {} machine f(a: [u8; 5]) {} machine f(a: &[u8]) {} "
        b"machine f(a: &mut [u8]) {} machine f(a: Box<u8>) {} machine f(a: Box<u16>) {}\n"
        b"data Box<T> { v: T; }",
        (
            builtin("u8"), LITERAL_ROW, array(),
            builtin("u8"), LITERAL_ROW, array(),
            builtin("u8"), SLICE_ROW, REFERENCE_ROW,
            builtin("u8"), SLICE_ROW, REFERENCE_ROW,
            builtin("u8"), data(0),
            builtin("u16"), data(0),
            generic(0),
        ),
    )
    rejected(
        program,
        "duplicate-overload-with-nested-types",
        b"machine f(a: &[Box<u8>; 4]) {} machine f(b: &[Box<u8>; 4]) {}\ndata Box<T> { v: T; }",
        DUPLICATE_MACHINE,
        b"f(",
        2,
        length=1,
    )

    # Generic parameters of every kind share one namespace per declaration.
    rejected(program, "duplicate-generic-parameter", b"data Dup<T, T> {}", DUPLICATE_GENERIC_PARAMETER, b"T", 2)
    rejected(
        program,
        "duplicate-lifetime-and-type-parameter",
        b"data Dup<'a, a> {}",
        DUPLICATE_GENERIC_PARAMETER,
        b"a>",
        length=1,
    )

    # A static application's argument types bind like body types.
    accepted(
        program,
        "static-application-arguments",
        b"machine f() { g<u8, Box<D>, 4>(1); }\ndata Box<T> { v: T; }\ndata D { x: u8; }\nmachine g(x: u8) {}",
        (builtin("u8"), data(1), data(0), LITERAL_ROW, generic(0), builtin("u8"), builtin("u8")),
    )
    rejected(program, "unknown-static-argument-type", b"machine f() { g<Q>(1); }", UNKNOWN_DATA_TYPE, b"Q")

    # Expression names: locals, then pattern binders, then the body's own
    # parameters; callees name machines, targets a state before a machine,
    # constructions and paths a data declaration's case.
    accepted(
        program,
        "locals-and-parameters",
        b"machine f(a: u8) { let b: u8 = a; g(b); }\nmachine g(x: u8) {}",
        (builtin("u8"), builtin("u8"), builtin("u8")),
        (parameter(0), machine(1), local(0), NOT_A_NAME),
    )
    accepted(
        program,
        "state-scope",
        b"machine f(a: u8) { transition { _ -> s(a) } state s(b: u8) { g(b); } }\nmachine g(x: u8) {}",
        (builtin("u8"), builtin("u8"), builtin("u8")),
        (state(0), parameter(0), NOT_A_NAME, machine(1), parameter(1), NOT_A_NAME),
    )
    rejected(
        program,
        "state-cannot-see-entry-parameter",
        b"machine f(a: u8) { transition { _ -> s() } state s() { g(a); } }\nmachine g(x: u8) {}",
        UNKNOWN_NAME,
        b"a)",
        length=1,
    )
    accepted(
        program,
        "cases-constructions-and-patterns",
        b"data T { case A; case B(v: u8); }\n"
        b"machine f(t: T) { transition t { T::B { v } -> s(v) _ -> s(1) } "
        b"state s(v: u8) { let y: T = T::B { v: v }; let z: T = T::A; } }",
        (builtin("u8"), data(0), builtin("u8"), data(0), data(0)),
        (
            parameter(0), data_case(0, 1), state(0), binder(0), NOT_A_NAME,
            state(0), NOT_A_NAME, NOT_A_NAME, data_case(0, 1), parameter(1), NOT_A_NAME, data_case(0, 0),
        ),
    )
    accepted(
        program,
        "owned-machine-callee",
        b"machine A::f(x: u8) {}\nmachine g() { A::f(1); }",
        (builtin("u8"),),
        (machine(0), NOT_A_NAME, NOT_A_NAME),
    )
    accepted(
        program,
        "machine-target",
        b"machine f() { transition { _ -> g(1) } }\nmachine g(x: u8) {}",
        (builtin("u8"),),
        (machine(1), NOT_A_NAME, NOT_A_NAME),
    )
    rejected(program, "unknown-case", b"data T { case A; }\nmachine f() { let x: T = T::C; }", UNKNOWN_NAME, b"T::C")
    rejected(program, "unknown-target", b"machine f() { transition { _ -> nowhere() } }", UNKNOWN_NAME, b"nowhere")
    rejected(program, "unknown-callee", b"machine f() { nowhere(1); }", UNKNOWN_NAME, b"nowhere")

    # Source sets: a unit sees its own module, a sibling unit of the same
    # module, and each module it imports; imports are not transitive, and two
    # visible declarations of one spelling are ambiguous.
    shapes = (b"shapes", b"pub data D { x: u8; }\npub machine g(d: D) {}")
    accepted_units(
        program,
        "import-binds-types-and-callees",
        (shapes, (b"", b"use shapes;\nmachine f(d: D) { g(d); }")),
        (builtin("u8"), data(0), data(0)),
        (machine(0), parameter(1), NOT_A_NAME),
    )
    accepted_units(
        program,
        "module-path-import",
        ((b"geometry::shapes", b"data D { x: u8; }"), (b"", b"use geometry::shapes;\nmachine f(d: D) {}")),
        (builtin("u8"), data(0)),
    )
    rejected_units(
        program,
        "unimported-module-is-invisible",
        (shapes, (b"", b"machine f(d: D) {}")),
        UNKNOWN_DATA_TYPE,
        1,
        b"D",
    )
    rejected_units(
        program,
        "import-names-no-module",
        (shapes, (b"", b"use shape;\nmachine f() {}")),
        UNRESOLVED_IMPORT,
        1,
        b"use shape;",
    )
    rejected_units(
        program,
        "import-path-prefix-is-no-module",
        ((b"geometry::shapes", b"data D { x: u8; }"), (b"", b"use geometry;\nmachine f() {}")),
        UNRESOLVED_IMPORT,
        1,
        b"use geometry;",
    )
    rejected_units(
        program,
        "imports-are-not-transitive",
        ((b"base", b"data D { x: u8; }"), (b"middle", b"use base;\ndata E { d: D; }"),
         (b"", b"use middle;\nmachine f(d: D) {}")),
        UNKNOWN_DATA_TYPE,
        2,
        b"D",
    )
    accepted_units(
        program,
        "same-spelling-in-other-modules",
        ((b"left", b"data D { x: u8; }"), (b"right", b"data D { y: u16; }"), (b"", b"use right;\nmachine f(d: D) {}")),
        (builtin("u8"), builtin("u16"), data(1)),
    )
    rejected_units(
        program,
        "ambiguous-type",
        ((b"left", b"data D { x: u8; }"), (b"right", b"data D { y: u16; }"),
         (b"", b"use left;\nuse right;\nmachine f(d: D) {}")),
        AMBIGUOUS_NAME,
        2,
        b"D",
    )
    rejected_units(
        program,
        "ambiguous-callee",
        ((b"left", b"machine g() {}"), (b"right", b"machine g() {}"),
         (b"", b"use left;\nuse right;\nmachine f() { g(); }")),
        AMBIGUOUS_NAME,
        2,
        b"g",
        occurrence=2,
    )
    accepted_units(
        program,
        "overloads-of-one-module-are-one-family",
        ((b"left", b"machine g(x: u8) {}\nmachine g(x: u16) {}"), (b"", b"use left;\nmachine f() { g(1); }")),
        (builtin("u8"), builtin("u16")),
        (machine(0), NOT_A_NAME, NOT_A_NAME),
    )
    accepted_units(
        program,
        "sibling-units-share-their-module",
        ((b"m", b"data D { x: u8; }"), (b"m", b"machine f(d: D) {}")),
        (builtin("u8"), data(0)),
    )
    rejected_units(
        program,
        "duplicate-across-sibling-units",
        ((b"m", b"data D { x: u8; }"), (b"m", b"data D { y: u8; }")),
        DUPLICATE_DATA,
        1,
        b"D",
    )
    rejected_units(
        program,
        "duplicate-machine-across-sibling-units",
        ((b"m", b"machine g(x: u8) {}"), (b"m", b"machine g(y: u8) {}")),
        DUPLICATE_MACHINE,
        1,
        b"g",
    )

    # The product's own psi package resolves as one source set: each module
    # under source/psi as `psi::<directory>::<file>`. Its build file belongs
    # to the build context, not the package's product sources.
    psi_root = Path(__file__).resolve().parents[3] / "source" / "psi"
    psi_units = tuple(
        (("psi::" + "::".join(path.relative_to(psi_root).with_suffix("").parts)).encode(), path.read_bytes())
        for path in sorted(psi_root.rglob("*.omg"))
        if path.parent != psi_root
    )
    status, payload = run_units(program, psi_units)
    assert status == 0, f"psi-package: status {status}"
    package = decode(payload)
    assert package.accepted and package.diagnostic == 0, f"psi-package: {package.diagnostic} {package.diagnostic_span}"
    assert len(psi_units) >= 17 and len(package.bindings) > 0 and len(package.value_bindings) > 0, "psi-package"

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
