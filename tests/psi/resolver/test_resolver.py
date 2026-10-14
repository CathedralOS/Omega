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
SHADOWS_BUILTIN = 2
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
PARTIAL_STABLE_IDENTITIES = 15
DUPLICATE_STABLE_IDENTITY = 16
INVALID_STABLE_IDENTITY = 17
UNSUPPORTED_DECLARATION = 18
UNSUPPORTED_OPERATOR_TOKEN = 19
UNSUPPORTED_MACHINE_CONTRACT = 20
UNSUPPORTED_CLAUSE = 21
UNSUPPORTED_GENERIC_PARAMETER = 22
UNSUPPORTED_EXPRESSION = 23
DUPLICATE_DECLARATION = 24
NOT_A_TYPE = 25
NOT_A_TRAIT = 26
NOT_A_REQUIREMENT = 27
UNDECLARED_DEPENDENCY = 28
FOREIGN_OPERATOR_HOME = 29
PROOF_ONLY_APPLICATION = 30
CONTRACT_NAMES_NO_BINDER = 31
DUPLICATE_CONTRACT = 32
UNSUPPORTED_RANKING_VIEW = 33
UNDECLARED_LIFETIME = 34
LIFETIME_ARGUMENT_COUNT = 35
MEASURE_FORM = 36
UNKNOWN_MEASURE_COMPONENT = 37

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


def data_value(item: int) -> tuple[int, int, int]:
    """A construction's data declaration."""
    return (7, item, 0)


def binder_name(handle: int) -> tuple[int, int, int]:
    """A value or machine generic parameter of the enclosing machine."""
    return (10, handle, 0)


def constant(handle: int) -> tuple[int, int, int]:
    return (11, handle, 0)


# Path bindings: (tag, handle).
def path_machine(handle: int) -> tuple[int, int]:
    return (1, handle)


def path_domain(handle: int) -> tuple[int, int]:
    return (2, handle)


def field_name(handle: int) -> tuple[int, int, int]:
    """A field a data declaration's `where` fact names bare."""
    return (12, handle, 0)


def returned_value(machine: int) -> tuple[int, int, int]:
    """`result` in a machine's `ensures` fact."""
    return (13, machine, 0)


def proposition(handle: int) -> tuple[int, int, int]:
    """A proposition application's callee."""
    return (14, handle, 0)


def domain_subject(handle: int) -> tuple[int, int, int]:
    """A domain's subject binder in its predicates."""
    return (16, handle, 0)


def byte_predicate(tag: int) -> tuple[int, int, int]:
    """A primitive byte predicate: 1 `valid_utf8`, 2 `no_nul`, 4
    `ascii_only`, 8 `non_empty`."""
    return (17, tag, 0)


def payload_field_name(handle: int) -> tuple[int, int, int]:
    """A payload field a case's own `where` fact names bare."""
    return (15, handle, 0)

# Type bindings: (tag, handle, domain tag, declared domain handle).
BUILTIN = 1
DATA = 2
GENERIC = 3
REFERENCE = 4
ARRAY = 5
SLICE = 6
LITERAL = 7
TRAIT = 9
CONFORMANCE = 10

UNQUALIFIED = 0
WRAPPING = 1
SATURATING = 2
TRAPPING = 3
FINITE = 4
DECLARED = 5
CASE = 6

BUILTIN_SPELLINGS = (
    "bool", "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "addr",
    "f32", "f64", "Slice", "Result", "SyscallResult", "Terminal", "Never",
    "UInt", "Int", "AtomicBool", "AtomicU32", "AtomicU64",
)


TypeRow = tuple[int, int, int, int]


def builtin(spelling: str, domain: int = UNQUALIFIED) -> TypeRow:
    return (BUILTIN, BUILTIN_SPELLINGS.index(spelling), domain, 0)


def data(handle: int, domain: int = UNQUALIFIED) -> TypeRow:
    return (DATA, handle, domain, 0)


def generic(handle: int, domain: int = UNQUALIFIED) -> TypeRow:
    return (GENERIC, handle, domain, 0)


def machine_argument(handle: int) -> TypeRow:
    """A static argument naming a machine, for a machine binder."""
    return (11, handle, UNQUALIFIED, 0)


def trait(handle: int) -> TypeRow:
    return (TRAIT, handle, UNQUALIFIED, 0)


def conformance(handle: int) -> TypeRow:
    return (CONFORMANCE, handle, UNQUALIFIED, 0)


def in_domain(row: TypeRow, domain: int) -> TypeRow:
    """`row` qualified by declared domain `domain`."""
    return (row[0], row[1], DECLARED, domain)


def in_case(row: TypeRow, data_case: int) -> TypeRow:
    """`row` qualified by the implied domain of case `data_case`."""
    return (row[0], row[1], CASE, data_case)


REFERENCE_ROW = (REFERENCE, 0, UNQUALIFIED, 0)
SLICE_ROW = (SLICE, 0, UNQUALIFIED, 0)
LITERAL_ROW = (LITERAL, 0, UNQUALIFIED, 0)


def array(domain: int = UNQUALIFIED) -> TypeRow:
    return (ARRAY, 0, domain, 0)


@dataclass(frozen=True)
class Observation:
    accepted: bool
    diagnostic: int
    diagnostic_span: tuple[int, int, int]
    bindings: tuple[TypeRow, ...]
    value_bindings: tuple[tuple[int, int, int], ...] = ()
    path_bindings: tuple[tuple[int, int], ...] = ()
    requirements: tuple[tuple[int, int], ...] = ()


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
    assert reader.u64() == 3
    accepted = reader.byte() == 1
    diagnostic = reader.byte()
    diagnostic_span = (reader.u64(), reader.u64(), reader.u64())
    bindings = tuple(
        (reader.byte(), reader.u64(), reader.byte(), reader.u64())
        for _ in range(reader.u64())
    )
    value_bindings = tuple(
        (reader.byte(), reader.u64(), reader.u64())
        for _ in range(reader.u64())
    )
    path_bindings = tuple(
        (reader.byte(), reader.u64())
        for _ in range(reader.u64())
    )
    requirements = tuple(
        (reader.byte(), reader.u64())
        for _ in range(reader.u64())
    )
    assert reader.cursor == len(payload), "trailing resolution observation bytes"
    return Observation(accepted, diagnostic, diagnostic_span, bindings, value_bindings, path_bindings, requirements)


# Every distinct input the gate runs is one case; the determinism check runs
# its input twice and counts once.
CASE_SOURCES: set[bytes] = set()


# A source set's packages, as (name, toolchain) rows, and dependency edges,
# as (requester, scope, alias, target) rows of package indices. Most cases
# run one ordinary package, which every unit given as (module path, source)
# belongs to; a unit given as (package, module path, source) names its own.
GATE_PACKAGES = ((b"gate", False),)
PRODUCT = 1
BUILD = 2

Package = tuple[bytes, bool]
Dependency = tuple[int, int, bytes, int]


def frame(
    units: tuple[tuple, ...],
    packages: tuple[Package, ...] = GATE_PACKAGES,
    dependencies: tuple[Dependency, ...] = (),
) -> bytes:
    """The harness input, framed like the request's subject tables: the
    packages (a toolchain flag and a `u32`-framed name), the dependency edges
    (requester, scope tag, framed alias, target), then the units (package
    index, framed module path, framed source). A unit's position is its
    source id."""
    payload = struct.pack("<I", len(packages))
    for name, toolchain in packages:
        payload += struct.pack("<II", 1 if toolchain else 0, len(name)) + name
    payload += struct.pack("<I", len(dependencies))
    for requester, scope, alias, target in dependencies:
        payload += struct.pack("<III", requester, scope, len(alias)) + alias + struct.pack("<I", target)
    payload += struct.pack("<I", len(units))
    for unit in units:
        package, module_path, source = unit if len(unit) == 3 else (0, *unit)
        payload += struct.pack("<II", package, len(module_path)) + module_path
        payload += struct.pack("<I", len(source)) + source
    return payload


def run_units(
    program: str,
    units: tuple[tuple, ...],
    packages: tuple[Package, ...] = GATE_PACKAGES,
    dependencies: tuple[Dependency, ...] = (),
) -> tuple[int, bytes]:
    payload = frame(units, packages, dependencies)
    CASE_SOURCES.add(payload)
    completed = subprocess.run([program], input=payload, capture_output=True, check=False)
    assert completed.stderr == b"", completed.stderr.decode(errors="replace")
    return completed.returncode, completed.stdout


def run(program: str, source: bytes) -> tuple[int, bytes]:
    """One source unit, the root module."""
    return run_units(program, ((b"", source),))


def expected_observation(
    actual: Observation,
    bindings: tuple[TypeRow, ...],
    value_bindings: tuple[tuple[int, int, int], ...] | None,
    path_bindings: tuple[tuple[int, int], ...] | None,
    requirements: tuple[tuple[int, int], ...] | None = None,
) -> tuple[Observation, Observation]:
    """Value, path and requirement bindings are compared only when the case
    names them."""
    actual = Observation(
        actual.accepted, actual.diagnostic, actual.diagnostic_span, actual.bindings,
        () if value_bindings is None else actual.value_bindings,
        () if path_bindings is None else actual.path_bindings,
        () if requirements is None else actual.requirements,
    )
    expected = Observation(True, 0, (0, 0, 0), bindings, value_bindings or (), path_bindings or (), requirements or ())
    return actual, expected


def accepted(
    program: str,
    name: str,
    source: bytes,
    bindings: tuple[TypeRow, ...],
    value_bindings: tuple[tuple[int, int, int], ...] | None = None,
    path_bindings: tuple[tuple[int, int], ...] | None = None,
    requirements: tuple[tuple[int, int], ...] | None = None,
) -> None:
    status, payload = run(program, source)
    assert status == 0, f"{name}: status {status}"
    actual, expected = expected_observation(decode(payload), bindings, value_bindings, path_bindings, requirements)
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
    units: tuple[tuple, ...],
    bindings: tuple[TypeRow, ...],
    value_bindings: tuple[tuple[int, int, int], ...] | None = None,
    path_bindings: tuple[tuple[int, int], ...] | None = None,
    packages: tuple[Package, ...] = GATE_PACKAGES,
    dependencies: tuple[Dependency, ...] = (),
) -> None:
    status, payload = run_units(program, units, packages, dependencies)
    assert status == 0, f"{name}: status {status}"
    actual, expected = expected_observation(decode(payload), bindings, value_bindings, path_bindings)
    assert actual == expected, f"{name}:\nexpected {expected}\nactual   {actual}"


def rejected_units(
    program: str,
    name: str,
    units: tuple[tuple, ...],
    diagnostic: int,
    unit: int,
    spelling: bytes,
    occurrence: int = 1,
    packages: tuple[Package, ...] = GATE_PACKAGES,
    dependencies: tuple[Dependency, ...] = (),
) -> None:
    """The diagnostic names the `occurrence`-th `spelling` of unit `unit`."""
    source = units[unit][-1]
    start = -1
    for _ in range(occurrence):
        start = source.index(spelling, start + 1)
    status, payload = run_units(program, units, packages, dependencies)
    assert status == 249, f"{name}: status {status}"
    actual = decode(payload)
    expected = Observation(False, diagnostic, (unit, start, start + len(spelling)), (), ())
    assert actual == expected, f"{name}:\nexpected {expected}\nactual   {actual}"


def phase_rejected(program: str, name: str, source: bytes, status: int) -> None:
    phase_rejected_units(program, name, ((b"", source),), status)


def phase_rejected_units(program: str, name: str, units: tuple[tuple, ...], status: int) -> None:
    actual_status, payload = run_units(program, units)
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
    rejected(program, "data-shadows-u8", b"data u8 { x: bool; }", SHADOWS_BUILTIN, b"u8")
    rejected(
        program,
        "data-shadows-last-builtin",
        b"data AtomicU64 { x: bool; }",
        SHADOWS_BUILTIN,
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

    # Facts: a `requires` fact names its body's parameters, a data `where`
    # fact its fields.
    accepted(
        program,
        "fact-names",
        b"data Range\nwhere\n    lo <= hi,\n{ lo: u64; hi: u64; }\n"
        b"machine f(x: u64) -> u64\nrequires x > 0\n{\n"
        b"    transition { _ -> g(x) }\n\n"
        b"    state g(y: u64) -> u64\n    requires y > 1\n    { y }\n}",
        (builtin("u64"), builtin("u64"), builtin("u64"), builtin("u64"), builtin("u64"), builtin("u64")),
        (
            field_name(0), field_name(1), NOT_A_NAME,
            parameter(0), NOT_A_NAME, NOT_A_NAME,
            state(0), parameter(0), NOT_A_NAME,
            parameter(1), NOT_A_NAME, NOT_A_NAME,
            parameter(1),
        ),
    )
    # A case's own `where` fact names its payload fields, then the
    # declaration's fields; another case's payload is not in scope.
    accepted(
        program,
        "case-fact-names-its-payload",
        b"data Interval { tag: u64; case Empty; case Range(lo: u64, hi: u64) where lo <= hi && tag > 0; }",
        (builtin("u64"), builtin("u64"), builtin("u64")),
        (
            payload_field_name(0), payload_field_name(1), NOT_A_NAME,
            field_name(0), NOT_A_NAME, NOT_A_NAME, NOT_A_NAME,
        ),
    )
    rejected(
        program,
        "case-fact-names-only-its-own-payload",
        b"data I { case A(x: u64); case B(y: u64) where x > 0; }",
        UNKNOWN_NAME,
        b"x > 0",
        length=1,
    )
    rejected(program, "fact-unknown-name", b"machine f(x: u64)\nrequires y > 0\n{}", UNKNOWN_NAME, b"y")
    # A fact whose root is the set's first expression (handle zero) binds:
    # a clause's fact is present by its flag, not by a nonzero handle.
    accepted(
        program,
        "fact-rooted-at-the-first-expression",
        b"machine f(b: bool)\nrequires b\n{}",
        (builtin("bool"),),
        (parameter(0),),
    )
    # `result` names the returned value in `ensures` facts only, and an
    # entry parameter spelled `result` shadows it.
    accepted(
        program,
        "ensures-names-result",
        b"machine f(x: u64) -> u64\nensures result >= x\n{ x }",
        (builtin("u64"), builtin("u64")),
        (returned_value(0), parameter(0), NOT_A_NAME, parameter(0)),
    )
    accepted(
        program,
        "parameter-shadows-result",
        b"machine f(result: u64) -> u64\nensures result > 0\n{ result }",
        (builtin("u64"), builtin("u64")),
        (parameter(0), NOT_A_NAME, NOT_A_NAME, parameter(0)),
    )
    rejected(program, "result-only-in-ensures", b"machine f(x: u64)\nrequires result > 0\n{}", UNKNOWN_NAME, b"result")
    rejected(
        program,
        "data-fact-names-only-its-fields",
        b"data A { a: u64; }\ndata B\nwhere\n    a > 0,\n{ b: u64; }",
        UNKNOWN_NAME,
        b"a > 0",
        length=1,
    )

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
    # An arm's `if` guard sees the pattern's binders
    # (wiki/spec/language/patterns.md#coverage-and-domain-tests).
    accepted(
        program,
        "arm-guards-see-pattern-binders",
        b"data T { case A; case B(v: u8); }\n"
        b"machine f(t: T, x: u8) { transition t { T::B { v } if v == x -> s(v) _ if x > 1 -> s(x) _ -> s(1) } "
        b"state s(v: u8) {} }",
        (builtin("u8"), data(0), builtin("u8"), builtin("u8")),
        (
            parameter(0), data_case(0, 1), binder(0), parameter(1), NOT_A_NAME, state(0), binder(0), NOT_A_NAME,
            parameter(1), NOT_A_NAME, NOT_A_NAME, state(0), parameter(1), NOT_A_NAME,
            state(0), NOT_A_NAME, NOT_A_NAME,
        ),
    )
    rejected(
        program,
        "arm-guard-cannot-see-another-arm's-binder",
        b"data T { case A; case B(v: u8); }\n"
        b"machine f(t: T) { transition t { T::B { v } -> s(v) _ if v == 1 -> s(1) } state s(v: u8) {} }",
        UNKNOWN_NAME,
        b"v == 1",
        length=1,
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

    # Stable schema identities: numbering is all-or-nothing within the
    # direct fields, the cases, and each case's payload fields, and a
    # numbered scope's values are distinct `u64`s, compared by value.
    accepted(
        program,
        "case-identities",
        b"data Optional<T> { case #0 None; case #1 Some(value: T); }",
        (generic(0),),
    )
    accepted(
        program,
        "identity-scopes-are-separate",
        b"data Disk { #1 counter: i32; case Done(#0 code: u8, #1 more: u8); case Empty; }",
        (builtin("i32"), builtin("u8"), builtin("u8")),
    )
    accepted(
        program,
        "identity-bases-and-separators",
        b"data H { #0x10 a: u8; #1_7 b: u8; #0b11 c: u8; #0o4 d: u8; #18446744073709551615 e: u8; }",
        (builtin("u8"),) * 5,
    )
    rejected(program, "partial-field-identities", b"data P { #1 a: u8; b: u8; }", PARTIAL_STABLE_IDENTITIES, b"b")
    rejected(
        program,
        "partial-identities-first-member",
        b"data P { a: u8; #1 b: u8; }",
        PARTIAL_STABLE_IDENTITIES,
        b"a:",
        length=1,
    )
    rejected(program, "partial-case-identities", b"data P { case #0 A; case B; }", PARTIAL_STABLE_IDENTITIES, b"B")
    rejected(
        program,
        "partial-payload-identities",
        b"data P { case A(#0 x: u8, y: u8); }",
        PARTIAL_STABLE_IDENTITIES,
        b"y",
    )
    rejected(
        program,
        "duplicate-identity-by-value",
        b"data P { #1 a: u8; #0x1 b: u8; }",
        DUPLICATE_STABLE_IDENTITY,
        b"0x1",
    )
    rejected(program, "duplicate-case-identity", b"data P { case #2 A; case #2 B; }", DUPLICATE_STABLE_IDENTITY, b"2", occurrence=2)
    rejected(program, "suffixed-identity", b"data P { #1u8 a: u8; }", INVALID_STABLE_IDENTITY, b"1u8")
    rejected(
        program,
        "identity-past-u64",
        b"data P { #18446744073709551616 a: u8; }",
        INVALID_STABLE_IDENTITY,
        b"18446744073709551616",
    )

    # Machine generic parameters bind in the machine's own scope: its
    # signature, its states' signatures, `let` types, cast targets and static
    # arguments. Value and machine binders also bind as names.
    accepted(
        program,
        "generic-machine-signature",
        b"machine Vec::as_slice<T>(items: &Vec<T>) -> &[T] { state s(x: T) {} }\ndata Vec<T> { v: T; }",
        (generic(0), data(0), REFERENCE_ROW, generic(0), SLICE_ROW, REFERENCE_ROW, generic(0), generic(1)),
    )
    accepted(
        program,
        "generic-machine-body-types",
        b"machine f<T>(x: T) { let y: T = x; g<T>(y as T); }\nmachine g<U>(u: U) {}",
        (generic(0), generic(0), generic(0), generic(0), generic(1)),
    )
    accepted(
        program,
        "generic-value-and-machine-binders",
        b"machine apply<machine F, const N: u64>(x: u64) -> u64 { transition { _ -> F(x, N) } }\n"
        b"machine call<machine G>(x: u64) { G(x); }",
        (builtin("u64"), builtin("u64"), builtin("u64"), builtin("u64")),
        (binder_name(0), parameter(0), binder_name(1), NOT_A_NAME, binder_name(2), parameter(1), NOT_A_NAME),
    )
    # A `where machine` contract gives its `machine` binder a callable
    # signature in the machine's scope; its `requires` facts name its own
    # parameters. A static argument may name a machine for that binder, or
    # forward a binder.
    accepted(
        program,
        "machine-contract-binds",
        b"machine apply<machine F>(x: u8) -> u8\nwhere machine F(value: u8) -> u8 requires value > 0;\n{ F(x) }",
        (builtin("u8"), builtin("u8"), builtin("u8"), builtin("u8")),
        (parameter(1), NOT_A_NAME, NOT_A_NAME, binder_name(0), parameter(0), NOT_A_NAME),
    )
    accepted(
        program,
        "static-machine-arguments",
        b"machine double(x: u8) -> u8 { x }\n"
        b"machine apply<machine F>(x: u8) -> u8\nwhere machine F(value: u8) -> u8;\n{ F(x) }\n"
        b"machine again<machine G>(x: u8) -> u8\nwhere machine G(value: u8) -> u8;\n{ apply<G>(x) }\n"
        b"machine g() -> u8 { apply<double>(3) }",
        (builtin("u8"), builtin("u8"), builtin("u8"), builtin("u8"), builtin("u8"), builtin("u8"),
         builtin("u8"), builtin("u8"), builtin("u8"), builtin("u8"), generic(1), builtin("u8"), machine_argument(0)),
    )
    rejected(
        program,
        "contract-names-no-binder",
        b"machine apply<machine F>(x: u8) where machine G(value: u8); {}",
        CONTRACT_NAMES_NO_BINDER,
        b"G",
    )
    rejected(
        program,
        "second-contract-for-a-binder",
        b"machine apply<machine F>(x: u8) where machine F(a: u8); where machine F(b: u8); {}",
        DUPLICATE_CONTRACT,
        b"F",
        occurrence=3,
    )
    accepted(
        program,
        "generic-overloads",
        b"machine f<T>(a: T) {} machine f(a: u8) {} machine g<T>(a: u8) {} machine g<const N: u64>(a: u8) {}",
        (generic(0), builtin("u8"), builtin("u8"), builtin("u64"), builtin("u8")),
    )
    rejected(
        program,
        "duplicate-generic-machine",
        b"machine f<T>(a: T) {} machine f<U>(b: U) {}",
        DUPLICATE_MACHINE,
        b"f",
        occurrence=2,
    )
    rejected(program, "duplicate-machine-generic-name", b"machine f<T, T>() {}", DUPLICATE_GENERIC_PARAMETER, b"T", occurrence=2)
    rejected(program, "type-binder-is-not-a-value", b"machine f<T>() { T; }", UNKNOWN_NAME, b"T", occurrence=2)
    rejected(program, "generic-scope-per-machine", b"machine f<T>() {} machine g(x: T) {}", UNKNOWN_DATA_TYPE, b"T", occurrence=2)
    rejected(program, "unknown-generic-annotation", b"machine f<const N: Q>() {}", UNKNOWN_DATA_TYPE, b"Q")

    # A bodyless declaration binds its signature; a terminal value target
    # binds as any value.
    accepted(
        program,
        "bodyless-machine-and-float-argument",
        b"machine Float::meaning(value: f64) -> u8;\nmachine g() { Float::meaning(1.0); }",
        (builtin("f64"), builtin("u8")),
        (machine(0), NOT_A_NAME, NOT_A_NAME),
    )
    accepted(
        program,
        "value-targets",
        b"data V { x: u8; }\nmachine f(a: u8) -> V { transition { a == 0 -> V { x: a } _ -> (g(a)) } }\n"
        b"machine g(b: u8) -> V { transition { _ -> V { x: b } } }",
        (builtin("u8"), builtin("u8"), data(0), builtin("u8"), data(0)),
        (
            parameter(0), NOT_A_NAME, NOT_A_NAME, data_value(0), parameter(0), NOT_A_NAME,
            machine(1), parameter(0), NOT_A_NAME, data_value(0), parameter(1), NOT_A_NAME,
        ),
    )
    rejected(program, "unknown-value-target", b"machine f() -> u8 { transition { _ -> nowhere } }", UNKNOWN_NAME, b"nowhere")

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

    # Mathematical definitions and assumptions (`let` roots) are declaration
    # kinds resolution does not bind yet: they reject at their whole root, in
    # source order and before any binding, instead of resolving as if their
    # names were absent.
    refinement = b"trait Local = Logger { machine * reaches; }"
    for name, source, spelling in (
        ("unsupported-definition", b"let zero: u8 = 0;", b"let zero: u8 = 0;"),
        ("unsupported-assumption", b"boundary let choose(witness: u8): u8;", b"boundary let choose(witness: u8): u8;"),
        ("unsupported-refinement", b"trait Logger { machine log(); }\n" + refinement, refinement),
    ):
        rejected(program, name, source, UNSUPPORTED_DECLARATION, spelling)
    rejected(
        program,
        "unsupported-trait-contract",
        b"trait Ordered<C> where proposition R(left: C, right: C); { }",
        UNSUPPORTED_MACHINE_CONTRACT,
        b"where proposition R(left: C, right: C);",
    )
    rejected(
        program,
        "unsupported-trait-evidence-binder",
        b"trait Calling<Policy satisfies CallingPolicy> {}",
        UNSUPPORTED_GENERIC_PARAMETER,
        b"Policy satisfies CallingPolicy",
    )

    # Machine forms whose bindings resolution does not record yet reject at
    # the form, before any binding, whatever contains the machine.
    for name, source, diagnostic, spelling in (
        ("unsupported-operator-token", b"machine + add(a: u8, b: u8) -> u8 { a }",
         UNSUPPORTED_OPERATOR_TOKEN, b"+"),
        ("unsupported-index-token", b"machine [] at(items: u8, index: u8) -> u8 { items }",
         UNSUPPORTED_OPERATOR_TOKEN, b"[]"),
        ("unsupported-nominal-machine-contract", b"machine apply<machine F>(x: u8) where machine F satisfies T::r; {}",
         UNSUPPORTED_MACHINE_CONTRACT, b"where machine F satisfies T::r;"),
        ("unsupported-contract-guarantee", b"machine apply<machine F>(x: u8) where machine F(value: u8) -> u8 ensures result > 0; {}",
         UNSUPPORTED_MACHINE_CONTRACT, b"where machine F(value: u8) -> u8 ensures result > 0;"),
        ("unsupported-evidence-binder", b"machine m<T, O satisfies Order<T>>() {}",
         UNSUPPORTED_GENERIC_PARAMETER, b"O satisfies Order<T>"),
        ("unsupported-proposition-binder", b"machine m<proposition R>() {}",
         UNSUPPORTED_GENERIC_PARAMETER, b"proposition R"),
        ("unsupported-requirement-token", b"trait Ranked { machine < compare(left: u8, right: u8) -> bool; }",
         UNSUPPORTED_OPERATOR_TOKEN, b"<"),
    ):
        rejected(program, name, source, diagnostic, spelling)
    source = b"data A { x: Q; }\nmachine + f(a: u8) -> u8 { a }"
    rejected(program, "unsupported-form-before-binding", source, UNSUPPORTED_OPERATOR_TOKEN, b"+")

    # An arithmetic or order token binds on an ordinary owned machine whose
    # owner is a data declaration of its own package: the token's semantic
    # home. A builtin or foreign home, equality's token and one token/shape
    # twice at one home reject.
    accepted(
        program,
        "operator-homed-at-its-data",
        b"data V { x: u64; }\nmachine + V::add(a: V, b: V) -> V { a }\nmachine + V::shift(a: V, b: u64) -> V { a }",
        (builtin("u64"), data(0), data(0), data(0), data(0), builtin("u64"), data(0)),
    )
    rejected(
        program,
        "operator-home-is-a-builtin",
        b"machine + u8::wrap(left: u8, right: u8) -> u8 { left }",
        FOREIGN_OPERATOR_HOME,
        b"u8",
    )
    rejected_units(
        program,
        "operator-home-in-another-package",
        ((1, b"shapes", b"pub data D { x: u8; }"), (0, b"", b"use geo::shapes;\nmachine + D::add(a: D, b: D) -> D { a }")),
        FOREIGN_OPERATOR_HOME,
        1,
        b"D",
        packages=((b"app", False), (b"geometry", False)),
        dependencies=((0, PRODUCT, b"geo", 1),),
    )
    rejected(
        program,
        "duplicate-operator-shape",
        b"data V { x: u64; }\nmachine + V::add(a: V, b: V) -> V { a }\nmachine + V::plus(a: V, b: V) -> V { b }",
        DUPLICATE_MACHINE,
        b"plus",
    )
    rejected(
        program,
        "equality-token-is-unsupported",
        b"data V { x: u64; }\nmachine == V::same(a: V, b: V) -> bool { true }",
        UNSUPPORTED_OPERATOR_TOKEN,
        b"==",
    )

    # A `satisfies` clause binds its trait, the trait's static arguments and
    # the named requirement; its `via` payload sees the machine's static
    # scope, never its parameters. A `terminates by` witness ranks the
    # body's parameters.
    order = b"trait Order<T> { machine before(left: T, right: T) -> bool; }\n"
    accepted(
        program,
        "satisfies-binds-the-requirement",
        order + b"machine ascending(left: u8, right: u8) -> bool satisfies Order<u8>::before { left < right }",
        (generic(0), generic(0), builtin("bool"), builtin("u8"), builtin("u8"), builtin("bool"), builtin("u8")),
        requirements=((1, 0),),
    )
    accepted(
        program,
        "satisfies-payload-binds-statically",
        b"data Binding { case Intrinsic; }\ntrait Clock { machine now() -> u64; }\n"
        b"machine C::now() -> u64 satisfies Clock::now via Binding::Intrinsic;",
        (builtin("u64"), builtin("u64")),
        (data_case(0, 0),),
        requirements=((1, 0),),
    )
    rejected(
        program,
        "satisfies-payload-sees-no-parameter",
        b"trait Clock { machine now() -> u64; }\nmachine C::now(x: u64) -> u64 satisfies Clock::now via x;",
        UNKNOWN_NAME,
        b"x",
        2,
    )
    rejected(program, "satisfies-unknown-trait", b"machine f() satisfies Order::before {}", UNKNOWN_DATA_TYPE, b"Order")
    rejected(
        program,
        "satisfies-unknown-requirement",
        order + b"machine f() satisfies Order<u8>::after {}",
        NOT_A_REQUIREMENT,
        b"after",
    )
    rejected(program, "satisfies-data-is-not-a-trait", b"data D { }\nmachine f() satisfies D::g {}", NOT_A_TRAIT, b"D", 2)
    rejected(
        program,
        "satisfies-through-a-module-path",
        b"machine f() satisfies shapes::Order::before {}",
        UNSUPPORTED_CLAUSE,
        b"satisfies shapes::Order::before",
    )
    accepted(
        program,
        "terminates-ranks-parameters",
        b"machine f(n: u64) requires n > 0 terminates by n; {}",
        (builtin("u64"),),
        (parameter(0), NOT_A_NAME, NOT_A_NAME, parameter(0)),
    )
    rejected(program, "terminates-unknown-subject", b"machine f(n: u64) terminates by m; {}", UNKNOWN_NAME, b"m;", length=1)
    # A view names the compiler's ranking vocabulary by spelling, not a
    # declaration; only `Nat::IncreasingTo`'s limit binds a name.
    accepted(
        program,
        "ranking-views-name-the-vocabulary",
        b"machine f(i: u64, n: u64) terminates by (i, n) -> Nat::BoundedDistance; {}\n"
        b"machine g(i: u64, n: u64) terminates by i -> Nat::IncreasingTo(n); {}",
        (builtin("u64"), builtin("u64"), builtin("u64"), builtin("u64")),
        (parameter(0), parameter(1), NOT_A_NAME, parameter(2), NOT_A_NAME, parameter(3), NOT_A_NAME),
    )
    rejected(
        program,
        "ranking-view-outside-the-vocabulary",
        b"machine f(n: u64) terminates by n -> Nat::Sideways; {}",
        UNSUPPORTED_RANKING_VIEW,
        b"Nat::Sideways",
    )
    # A rank range's endpoints bind as the subjects do
    # (wiki/spec/language/termination.md#ranking).
    accepted(
        program,
        "rank-range-endpoints-bind",
        b"machine f(n: u64, limit: u64) terminates by n in 0..=limit; {}",
        (builtin("u64"), builtin("u64")),
        (parameter(0), NOT_A_NAME, parameter(1), NOT_A_NAME),
    )
    rejected(program, "rank-range-unknown-endpoint", b"machine f(n: u64) terminates by n in 0..=limit; {}", UNKNOWN_NAME, b"limit")
    # A declared measure (wiki/spec/language/termination.md#ranking) is the
    # view its path names: a computed one takes one subject parameter and a
    # body, a lexicographic one names fields of its owner. A measure is never
    # a callee.
    measures = (
        b"data Card { power: u64; }\ndata Progress { outer: u64; inner: u64; }\n"
        b"measure Card::PowerOrder(card: Card) -> u64 { card.power }\n"
        b"measure Progress::Steps lexicographic { outer, inner }\n"
        b"machine f(card: Card, progress: Progress)\n"
        b"terminates by card -> Card::PowerOrder;\n"
        b"terminates by progress -> Progress::Steps;\n{}"
    )
    accepted(
        program,
        "measure-views-bind-their-declarations",
        measures,
        (builtin("u64"), builtin("u64"), builtin("u64"), data(0), builtin("u64"), data(0), data(1)),
    )
    rejected(
        program,
        "a-measure-is-not-a-callee",
        b"data Card { power: u64; }\nmeasure Card::PowerOrder(card: Card) -> u64 { card.power }\n"
        b"machine f(card: Card) -> u64 { Card::PowerOrder(card) }",
        UNKNOWN_NAME,
        b"Card::PowerOrder",
        occurrence=2,
    )
    rejected(
        program,
        "a-computed-measure-takes-one-subject",
        b"measure Card::PowerOrder(card: u64, other: u64) -> u64 { card }",
        MEASURE_FORM,
        b"measure Card::PowerOrder(card: u64, other: u64) -> u64 { card }",
    )
    rejected(
        program,
        "a-lexicographic-measure-names-its-owner-fields",
        b"data Progress { outer: u64; }\nmeasure Progress::Steps lexicographic { outer, depth }",
        UNKNOWN_MEASURE_COMPONENT,
        b"depth",
    )

    # A membership in `Type::Case` binds the case whose implied domain it
    # names; a `Carrier::Name` path or a bare name binds a declared domain;
    # several domains at once, or a bare name several domains declare, need
    # what resolution does not bind yet. A value-position
    # match binds its subject, value patterns and values as values, and a
    # renamed pattern field binds under its rename while a waived one binds
    # nothing.
    accepted(
        program,
        "membership-binds-its-case",
        b"data O { case None; case Some(value: u8); }\nmachine f(o: O) -> bool { o in O::None }",
        (builtin("u8"), data(0), builtin("bool")),
        (parameter(0), data_case(0, 0)),
    )
    rejected(program, "case-domain-qualifier-is-unknown", b"data X { field: u8 in Event::Completion; }",
             UNKNOWN_DOMAIN, b"Event::Completion")
    # A path qualifier `T in Owner::Name` names what a membership's path
    # names, and that domain must qualify `T`.
    accepted(
        program,
        "path-qualifier-binds-a-declared-domain",
        b"boundary data Event;\ndomain Event::Completion;\n"
        b"data Attempt { case Done(event: Event in Event::Completion); case Pending; }",
        (data(0), in_domain(data(0), 0)),
    )
    accepted(
        program,
        "path-qualifier-binds-a-case-domain",
        b"data O { case None; case Some(value: u8); }\nmachine f(o: O in O::Some) {}",
        (builtin("u8"), in_case(data(0), 1)),
    )
    rejected(
        program,
        "path-qualifier-of-another-carrier",
        b"domain u64::Small;\nmachine f(x: u32 in u64::Small) {}",
        UNKNOWN_DOMAIN,
        b"u64::Small",
        2,
    )
    rejected(
        program,
        "case-qualifier-of-other-data",
        b"data O { case None; }\ndata P { case None; }\nmachine f(p: P in O::None) {}",
        UNKNOWN_DOMAIN,
        b"O::None",
    )
    rejected(
        program,
        "case-and-domain-compete-in-a-qualifier",
        b"data K { case A; }\ndomain K::A;\nmachine f(k: K in K::A) {}",
        AMBIGUOUS_NAME,
        b"K::A",
        2,
    )
    rejected(program, "membership-in-a-declared-domain", b"machine f(o: u8) -> bool { o in Positive }",
             UNKNOWN_DOMAIN, b"Positive")
    rejected(program, "membership-in-a-machine", b"machine O::check() {}\nmachine f(o: u8) -> bool { o in O::check }",
             UNKNOWN_DOMAIN, b"O::check", occurrence=2)
    rejected(
        program,
        "membership-in-several-domains",
        b"data O { case None; case Some(value: u8); }\nmachine f(o: O) -> bool { o in O::None | O::Some }",
        UNSUPPORTED_EXPRESSION,
        b"o in O::None | O::Some",
    )
    accepted(
        program,
        "match-binds-its-names",
        b"data K { case A; case B; }\nmachine f(k: K, x: u8) -> u8 { match k { K::A -> x, _ -> 0 } }",
        (data(0), builtin("u8"), builtin("u8")),
        (parameter(0), data_case(0, 0), parameter(1), NOT_A_NAME, NOT_A_NAME),
    )
    accepted(
        program,
        "renamed-pattern-binder",
        b"data N { case Zero; case Succ(prev: u8); }\nmachine f(n: N) -> u8 { transition n { N::Succ { prev as w } -> w _ -> 0 } }",
        (builtin("u8"), data(0), builtin("u8")),
        (parameter(0), data_case(0, 1), binder(0), NOT_A_NAME),
    )
    rejected(
        program,
        "waived-pattern-field-binds-nothing",
        b"data H { case P(ok: u8); }\nmachine f(h: H) -> u8 { transition h { H::P { ok as _ } -> ok } }",
        UNKNOWN_NAME,
        b"ok",
        occurrence=3,
    )
    accepted(
        program,
        "membership-in-a-declared-domain-binds",
        b"domain u8::Small(v) requires v < 4;\nmachine f(o: u8) -> bool { o in Small }",
        (builtin("u8"), builtin("u8"), builtin("bool")),
        None,
        (path_domain(0),),
    )
    # A domain's predicates name its subject binder, and a primitive byte
    # predicate binds by its spelling.
    accepted(
        program,
        "domain-predicates-name-their-subject",
        b"domain u64::Small(value)\nrequires value < 8;\ndomain [u8]::Text(bytes)\nrequires valid_utf8(bytes);",
        (builtin("u64"), builtin("u8"), SLICE_ROW),
        (domain_subject(0), NOT_A_NAME, NOT_A_NAME, byte_predicate(1), domain_subject(1), NOT_A_NAME),
    )
    rejected(
        program,
        "domain-predicates-see-only-their-subject",
        b"domain u64::Small(value)\nrequires other < 8;",
        UNKNOWN_NAME,
        b"other",
    )
    small = b"domain u8::Small;\ndomain u16::Small;\n"
    accepted(
        program,
        "membership-selects-by-carrier-path",
        small + b"machine f(o: u16) -> bool { o in u16::Small }",
        (builtin("u8"), builtin("u16"), builtin("u16"), builtin("bool")),
        None,
        (path_domain(1),),
    )
    rejected(
        program,
        "bare-membership-needs-its-carrier",
        small + b"machine f(o: u16) -> bool { o in Small }",
        UNSUPPORTED_EXPRESSION,
        b"Small",
        3,
    )
    rejected(
        program,
        "case-and-domain-compete",
        b"data K { case A; }\ndomain K::A;\nmachine f(k: K) -> bool { k in K::A }",
        AMBIGUOUS_NAME,
        b"K::A",
        2,
    )
    source = b"data A { x: u8; }\nlet zero: u8 = 0;\ndata B { y: Q; }"
    rejected(program, "unsupported-declaration-before-binding", source, UNSUPPORTED_DECLARATION, b"let zero: u8 = 0;")
    # `boundary data` declares an opaque type name and binds like any other
    # data declaration.
    accepted(program, "boundary-data-binds", b"boundary data Handle;\ndata A { h: Handle; }", (data(0),))
    accepted(
        program,
        "generic-boundary-data-binds",
        b"pub boundary data Ptr<T> [linear];\ndata A { p: Ptr<u8>; }",
        (builtin("u8"), data(0)),
    )

    # Traits: requirements and defaults are machines named through their
    # trait; the trait's generic parameters cover its members, whose own
    # shadow them. A callee `Trait::requirement`, or a bare fellow member
    # inside the trait, names the requirement.
    accepted(
        program,
        "trait-requirements-and-defaults",
        b"trait Order<T> { machine before(left: T, right: T) -> bool; "
        b"machine after(left: T, right: T) -> bool { Order::before(right, left) } "
        b"machine within(left: T, right: T) -> bool { before(left, right) } }",
        (generic(0), generic(0), builtin("bool"), generic(0), generic(0), builtin("bool"),
         generic(0), generic(0), builtin("bool")),
        (machine(0), parameter(3), parameter(2), NOT_A_NAME, machine(0), parameter(4), parameter(5), NOT_A_NAME),
    )
    accepted(
        program,
        "member-generics-shadow-the-container",
        b"trait Map<T> { machine map<T>(value: T) -> T; machine keep(value: T) -> T; }",
        (generic(1), generic(1), generic(0), generic(0)),
    )
    accepted(
        program,
        "container-value-binder",
        b"trait Sized<const N: u64> { machine size() -> u64 { N } }",
        (builtin("u64"), builtin("u64")),
        (binder_name(0),),
    )
    accepted(
        program,
        "trait-parents",
        b"trait Base<T> { }\ntrait Order<T>: Base<T> { requires Base<T>; }",
        (generic(1), trait(0), generic(1), trait(0)),
    )
    rejected(program, "parent-is-not-a-trait", b"data D { }\ntrait T: D { }", NOT_A_TRAIT, b"D", 2)
    rejected(program, "generic-parent-is-not-a-trait", b"trait T<P>: P { }", NOT_A_TRAIT, b"P", 2)
    rejected(program, "unknown-parent", b"trait T: Missing { }", UNKNOWN_DATA_TYPE, b"Missing")
    rejected(program, "trait-is-not-a-type", b"trait T { }\ndata D { x: T; }", NOT_A_TYPE, b"T", 2)
    rejected(
        program,
        "duplicate-requirement",
        b"trait T { machine f(x: u8); machine f(y: u8); }",
        DUPLICATE_MACHINE,
        b"f",
        2,
    )
    rejected(
        program,
        "requirement-and-owned-machine-share-a-path",
        b"trait T { machine f(x: u8); }\nmachine T::f(y: u8) {}",
        DUPLICATE_MACHINE,
        b"f",
        2,
    )

    # Named conformances: the satisfied trait binds as a trait application,
    # members bind like machines in the conformance's scope, and a reference
    # row binds its requirement and its machine. `Conformance::member` names
    # the member, else the machine its row or its trait supplies.
    counter = b"data Counter<T> { value: T; }\ntrait Increment<C> { machine increment(counter: &mut C); machine reset(counter: &mut C); }\n"
    accepted(
        program,
        "conformance-members-and-rows",
        counter
        + b"machine Counter::clear<T>(counter: &mut Counter<T>) {}\n"
        b"Saturating<T> satisfies Increment<Counter<T>> { machine increment(counter: &mut Counter<T>) {} "
        b"Increment::reset = Counter::clear; }\n"
        b"machine use_it(c: &mut Counter<u8>) { Saturating::increment(c); Saturating::reset(c); }",
        (
            generic(0),
            generic(1), REFERENCE_ROW, generic(1), REFERENCE_ROW,
            generic(2), data(0), REFERENCE_ROW,
            generic(3), data(0), trait(0),
            generic(3), data(0), REFERENCE_ROW,
            builtin("u8"), data(0), REFERENCE_ROW,
        ),
        (machine(3), parameter(4), NOT_A_NAME, machine(2), parameter(4), NOT_A_NAME),
        (path_machine(1), path_machine(2)),
    )
    ranked = b"trait Ranked<T> { machine rank(value: T) -> u64; machine twice(value: T) -> u64 { 2 } }\n"
    accepted(
        program,
        "conformance-default-callee",
        ranked + b"ByValue satisfies Ranked<u64> { machine rank(value: u64) -> u64 { value } }\n"
        b"machine f() -> u64 { ByValue::twice(1) }",
        (generic(0), builtin("u64"), generic(0), builtin("u64"), builtin("u64"), trait(0), builtin("u64"),
         builtin("u64"), builtin("u64")),
        (NOT_A_NAME, parameter(2), machine(1), NOT_A_NAME, NOT_A_NAME),
    )
    rejected(
        program,
        "row-names-no-requirement",
        b"machine D::helper() {}\ntrait T { machine f(); }\nC satisfies T { D::helper = D::helper; }",
        NOT_A_REQUIREMENT,
        b"D::helper",
        2,
    )
    rejected(
        program,
        "row-names-unknown-requirement",
        b"trait T { machine f(); }\nmachine g() {}\nC satisfies T { T::missing = g; }",
        UNKNOWN_NAME,
        b"T::missing",
    )
    rejected(
        program,
        "row-names-unknown-machine",
        b"trait T { machine f(); }\nC satisfies T { T::f = missing; }",
        UNKNOWN_NAME,
        b"missing",
    )
    rejected(program, "satisfied-data-is-not-a-trait", b"data D { }\nC satisfies D { }", NOT_A_TRAIT, b"D", 2)
    order = b"trait Order<T> { machine before(left: T, right: T) -> bool; }\n"
    accepted(
        program,
        "conformance-as-static-argument",
        order + b"Ascending satisfies Order<u8> { machine before(left: u8, right: u8) -> bool { left < right } }\n"
        b"machine sort<T, O>(value: T) {}\nmachine f() { sort<u8, Ascending>(1); }",
        (generic(0), generic(0), builtin("bool"), builtin("u8"), trait(0), builtin("u8"), builtin("u8"),
         builtin("bool"), generic(1), builtin("u8"), conformance(0)),
    )
    rejected(
        program,
        "conformance-is-not-a-type",
        order + b"Ascending satisfies Order<u8> { }\nmachine f(x: Ascending) {}",
        NOT_A_TYPE,
        b"Ascending",
        2,
    )
    rejected(
        program,
        "conformance-is-not-a-referent",
        order + b"Ascending satisfies Order<u8> { }\nmachine f(x: &Ascending) {}",
        NOT_A_TYPE,
        b"Ascending",
        2,
    )
    accepted_units(
        program,
        "imported-trait-requirement-callee",
        ((b"shapes", b"pub trait Shape { machine area() -> u64; }"), (b"", b"use shapes;\nmachine f() -> u64 { Shape::area() }")),
        (builtin("u64"), builtin("u64")),
        (machine(0), NOT_A_NAME),
    )

    # Type names share one namespace per module across data, traits and
    # conformances; constants and propositions have their own.
    for name, source, spelling, occurrence in (
        ("trait-after-data", b"data T { }\ntrait T { }", b"T", 2),
        ("data-after-trait", b"trait T { }\ndata T { }", b"T", 2),
        ("duplicate-conformance", b"trait R { }\nC satisfies R { }\nC satisfies R { }", b"C", 2),
        ("duplicate-constant", b"const N: u64 = 1;\nconst N: u64 = 2;", b"N", 2),
        ("duplicate-owned-constant", b"const A::N: u64 = 1;\nconst A::N: u64 = 2;", b"N", 2),
        ("duplicate-proposition", b"proposition P;\nproposition P;", b"P", 2),
        ("duplicate-domain", b"domain u64::Small(v) requires v < 4;\ndomain u64::Small;", b"Small", 2),
    ):
        rejected(program, name, source, DUPLICATE_DECLARATION, spelling, occurrence)
    rejected_units(
        program,
        "duplicate-trait-across-sibling-units",
        ((b"m", b"trait T { }"), (b"m", b"trait T { }")),
        DUPLICATE_DECLARATION,
        1,
        b"T",
    )
    rejected(program, "trait-shadows-builtin", b"trait u8 { }", SHADOWS_BUILTIN, b"u8")
    rejected(program, "domain-shadows-policy", b"domain u64::Wrapping;", SHADOWS_BUILTIN, b"Wrapping")
    accepted(
        program,
        "same-names-in-other-namespaces",
        b"const A::N: u64 = 1;\nconst B::N: u64 = 1;\nconst T: u64 = 1;\ndata T { }\nproposition T;\n"
        b"domain u32::Small;\ndomain u64::Small;",
        (builtin("u64"), builtin("u64"), builtin("u64"), builtin("u32"), builtin("u64")),
    )

    # Target-scoped machines: bodies for distinct targets form one family
    # under one path; a second body for one target, or an untargeted body
    # beside them, repeats the family's declaration.
    accepted(
        program,
        "target-scoped-family",
        b"linux_arm64 machine Clock::now() -> u64 { 1 }\nmacos_arm64 machine Clock::now() -> u64 { 2 }\n"
        b"machine read() -> u64 { Clock::now() }\nboundary machine entry(code: u8);",
        (builtin("u64"), builtin("u64"), builtin("u64"), builtin("u8")),
        (NOT_A_NAME, NOT_A_NAME, machine(0), NOT_A_NAME),
    )
    rejected(
        program,
        "second-body-for-one-target",
        b"linux_arm64 machine Clock::now() -> u64 { 1 }\nlinux_arm64 machine Clock::now() -> u64 { 2 }",
        DUPLICATE_MACHINE,
        b"now",
        2,
    )
    rejected(
        program,
        "untargeted-body-beside-a-family",
        b"machine Clock::now() -> u64 { 1 }\nlinux_arm64 machine Clock::now() -> u64 { 2 }",
        DUPLICATE_MACHINE,
        b"now",
        2,
    )

    # Domains: an `in Name` qualifier selects the visible domain of that name
    # whose carrier is the qualified type, a domain's own generic parameters
    # standing for any type; carriers and index arguments bind in the
    # domain's scope; alias constituents name domains and routes machines.
    accepted(
        program,
        "declared-domain-qualifier",
        b"domain u64::Small(value) requires value < 4;\nmachine f(x: u64 in Small) {}",
        (builtin("u64"), in_domain(builtin("u64"), 0)),
    )
    accepted(
        program,
        "generic-carrier-domain",
        b"domain<T> T::NonZero;\nmachine f<T>(x: u32 in NonZero, y: T in NonZero) {}",
        (generic(0), in_domain(builtin("u32"), 0), in_domain(generic(1), 0)),
    )
    accepted(
        program,
        "slice-carrier-domain",
        b"domain [u8]::Path(path);\nmachine open(path: &[u8] in Path) {}",
        (builtin("u8"), SLICE_ROW, builtin("u8"), in_domain(SLICE_ROW, 0), REFERENCE_ROW),
    )
    accepted(
        program,
        "domain-selected-by-carrier",
        b"domain u32::Small;\ndomain u64::Small;\nmachine f(x: u64 in Small, y: u32 in Small) {}",
        (builtin("u32"), builtin("u64"), in_domain(builtin("u64"), 1), in_domain(builtin("u32"), 0)),
    )
    accepted(
        program,
        "indexed-domain",
        b"domain<const Capacity: u64> u64::AtMost<Capacity>(value) requires value <= Capacity;\n"
        b"data Buffer { length: u64 in AtMost; }",
        (builtin("u64"), builtin("u64"), generic(0), in_domain(builtin("u64"), 0)),
    )
    rejected(
        program,
        "carrier-mismatch",
        b"domain u64::Small;\nmachine f(x: u32 in Small) {}",
        UNKNOWN_DOMAIN,
        b"Small",
        2,
    )
    rejected(
        program,
        "ambiguous-domain",
        b"domain u32::Small;\ndomain<T> T::Small;\nmachine f(x: u32 in Small) {}",
        AMBIGUOUS_NAME,
        b"Small",
        3,
    )
    rejected(program, "unknown-carrier", b"domain Missing::Small;", UNKNOWN_DATA_TYPE, b"Missing")
    accepted(
        program,
        "alias-constituents-and-routes",
        b"domain i32::Positive(v) requires v > 0;\ndomain i32::Small(v) requires v < 10;\n"
        b"domain i32::Both = Positive & i32::Small;\n"
        b"trait Issues { machine issue() -> u64; }\nmachine mint() -> u64 { 1 }\n"
        b"domain u64::Issued established by Issues::issue, mint;",
        (builtin("i32"), builtin("i32"), builtin("i32"), builtin("u64"), builtin("u64"), builtin("u64")),
        None,
        (path_domain(0), path_domain(1), path_machine(0), path_machine(1)),
    )
    rejected(
        program,
        "unknown-constituent",
        b"domain i32::Positive;\ndomain i32::Both = Positive & Missing;",
        UNKNOWN_DOMAIN,
        b"Missing",
    )
    rejected(
        program,
        "constituent-of-another-carrier",
        b"domain u64::Positive;\ndomain i32::Both = Positive;",
        UNKNOWN_DOMAIN,
        b"Positive",
        2,
    )
    rejected(program, "unknown-route", b"domain u64::Issued established by missing;", UNKNOWN_NAME, b"missing")

    # Constants: a constant is a value, after locals, parameters and value
    # binders; `Owner::NAME` names an owned constant; a constant's value
    # binds with no locals or parameters in scope.
    accepted(
        program,
        "constants-are-values",
        b"const LIMIT: u64 = 4096;\nconst TWICE: u64 = LIMIT;\nmachine f() -> u64 { LIMIT }\n"
        b"machine g(LIMIT: u64) -> u64 { LIMIT }",
        (builtin("u64"), builtin("u64"), builtin("u64"), builtin("u64"), builtin("u64")),
        (NOT_A_NAME, constant(0), constant(0), parameter(0)),
    )
    accepted(
        program,
        "owned-constant",
        b"data Duration { seconds: u64; }\nconst Duration::ZERO: Duration = Duration { seconds: 0 };\n"
        b"machine f() -> Duration { Duration::ZERO }",
        (builtin("u64"), data(0), data(0)),
        (data_value(0), NOT_A_NAME, NOT_A_NAME, constant(0)),
    )
    rejected(
        program,
        "constant-and-case-share-a-path",
        b"data E { case A; }\nconst E::A: E = E::A;",
        DUPLICATE_DECLARATION,
        b"A",
        2,
    )
    rejected(program, "constant-and-machine-share-a-path", b"machine N() {}\nconst N: u64 = 1;", DUPLICATE_DECLARATION, b"N", 2)
    rejected_units(
        program,
        "case-and-constant-compete-across-modules",
        ((b"left", b"pub data E { case A; }"), (b"right", b"pub const E::A: u64 = 1;"),
         (b"", b"use left;\nuse right;\nmachine f() { let x: u64 = E::A; }")),
        AMBIGUOUS_NAME,
        2,
        b"E::A",
    )
    rejected(program, "unknown-name-in-constant", b"const A: u64 = missing;", UNKNOWN_NAME, b"missing")
    rejected(program, "unknown-constant-type", b"const A: Q = 1;", UNKNOWN_DATA_TYPE, b"Q")
    rejected(program, "constant-is-not-a-callee", b"const A: u64 = 1;\nmachine f() { A(); }", UNKNOWN_NAME, b"A", 2)

    # Propositions: generic annotations, parameter and evidence types bind
    # in the proposition's scope; its fact is checking's.
    accepted(
        program,
        "propositions",
        b"proposition Even(value: u64) = value % 2 == 0;\nproposition Witness<T>(value: T) evidence T;",
        (builtin("u64"), generic(0), generic(0)),
    )
    rejected(program, "duplicate-proposition-parameter", b"proposition P(a: u8, a: u8);", DUPLICATE_PARAMETER, b"a", 2)
    rejected(program, "unknown-evidence-type", b"proposition P evidence Q;", UNKNOWN_DATA_TYPE, b"Q")

    # A proposition applies as a whole fact: its callee binds the
    # proposition, its arguments like any fact names. Machines and
    # propositions are one callee tier. Inside a larger fact the application
    # is unsupported, and where a value is computed it is no value at all.
    accepted(
        program,
        "proposition-applications-in-facts",
        b"proposition Close(a: u64, b: u64) = a == b;\n"
        b"machine f(x: u64, y: u64)\nrequires Close(x, y)\nensures Close(x, x)\n{}",
        (builtin("u64"), builtin("u64"), builtin("u64"), builtin("u64")),
        (
            NOT_A_NAME, NOT_A_NAME, NOT_A_NAME,
            proposition(0), parameter(2), parameter(3), NOT_A_NAME,
            proposition(0), parameter(2), parameter(2), NOT_A_NAME,
        ),
    )
    accepted(
        program,
        "proposition-application-in-a-data-fact",
        b"proposition Ordered(lo: u64, hi: u64) = lo <= hi;\n"
        b"data Range\nwhere\n    Ordered(lo, hi),\n{ lo: u64; hi: u64; }",
        (builtin("u64"), builtin("u64"), builtin("u64"), builtin("u64")),
        (NOT_A_NAME, NOT_A_NAME, NOT_A_NAME, proposition(0), field_name(0), field_name(1), NOT_A_NAME),
    )
    accepted(
        program,
        "applied-generic-proposition",
        b"proposition Same<T>(a: T, b: T);\nmachine f(x: u8)\nrequires Same<u8>(x, x)\n{}",
        (generic(0), generic(0), builtin("u8"), builtin("u8")),
        (proposition(0), NOT_A_NAME, parameter(2), parameter(2), NOT_A_NAME),
    )
    # Lifetime tags name a declared binder of the declaration or its
    # container; an applied name spells as many lifetime arguments as the
    # data declares, or elides them (wiki/spec/language/lifetimes.md).
    accepted(
        program,
        "declared-lifetimes-bind",
        b"data Message<'buffer> { body: &'buffer [u8]; }\n"
        b"machine f<'s>(m: &Message<'s>, bare: &Message) -> u64 { m.body.len }",
        (builtin("u8"), SLICE_ROW, REFERENCE_ROW, data(0), REFERENCE_ROW, data(0), REFERENCE_ROW, builtin("u64")),
    )
    rejected(
        program,
        "undeclared-lifetime",
        b"data D { v: &'a u8; }",
        UNDECLARED_LIFETIME,
        b"&'a u8",
    )
    rejected(
        program,
        "lifetime-argument-count",
        b"data P { v: u8; }\nmachine f<'a>(p: &P<'a>) {}",
        LIFETIME_ARGUMENT_COUNT,
        b"P",
        2,
    )
    rejected(
        program,
        "fewer-lifetime-arguments",
        b"data Pair<'a, 'b> { x: &'a u8; y: &'b u8; }\nmachine f<'s>(p: &Pair<'s>) {}",
        LIFETIME_ARGUMENT_COUNT,
        b"Pair",
        2,
    )
    rejected(
        program,
        "proposition-inside-a-larger-fact",
        b"proposition P(a: u64);\nmachine f(x: u64)\nrequires P(x) && x > 0\n{}",
        UNSUPPORTED_EXPRESSION,
        b"P",
        2,
    )
    rejected(
        program,
        "proposition-inside-a-larger-data-fact",
        b"proposition P(a: u64);\ndata D\nwhere\n    !P(v),\n{ v: u64; }",
        UNSUPPORTED_EXPRESSION,
        b"P",
        2,
    )
    rejected(
        program,
        "proposition-applied-in-a-body",
        b"proposition P(a: u64);\nmachine f(x: u64) -> bool { P(x) }",
        PROOF_ONLY_APPLICATION,
        b"P",
        2,
    )
    rejected(
        program,
        "proposition-applied-in-a-constant",
        b"proposition P(a: u64);\nconst C: bool = P(1);",
        PROOF_ONLY_APPLICATION,
        b"P",
        2,
    )
    rejected(
        program,
        "machine-and-proposition-share-a-callee",
        b"proposition f(a: u64);\nmachine f(x: u64) {}\nmachine g(x: u64)\nrequires f(x)\n{}",
        AMBIGUOUS_NAME,
        b"f",
        3,
    )

    # Packages: a `use` path's first member selects a requester-local product
    # dependency alias's package, or the requester's own by its name, and the
    # toolchain mount `omega::language::core` the toolchain package; any
    # other path is a module of the requester's own package. Across packages
    # only `pub` declarations are visible, a member by its container's `pub`.
    app_geometry = ((b"app", False), (b"geometry", False))
    geo = ((0, PRODUCT, b"geo", 1),)
    accepted_units(
        program,
        "aliased-import-resolves",
        ((1, b"shapes", b"pub data D { x: u8; }\npub machine g(d: D) {}"),
         (0, b"", b"use geo::shapes;\nmachine f(d: D) { g(d); }")),
        (builtin("u8"), data(0), data(0)),
        (machine(0), parameter(1), NOT_A_NAME),
        packages=app_geometry,
        dependencies=geo,
    )
    accepted_units(
        program,
        "own-package-name-selects-its-module",
        ((b"shapes", b"data D { x: u8; }"), (b"", b"use gate::shapes;\nmachine f(d: D) {}")),
        (builtin("u8"), data(0)),
    )
    rejected_units(
        program,
        "package-name-without-alias-rejects",
        ((1, b"shapes", b"pub data D { x: u8; }"), (0, b"", b"use geometry::shapes;\nmachine f() {}")),
        UNDECLARED_DEPENDENCY,
        1,
        b"use geometry::shapes;",
        packages=app_geometry,
        dependencies=geo,
    )
    rejected_units(
        program,
        "build-alias-authorizes-no-product-import",
        ((1, b"shapes", b"pub data D { x: u8; }"), (0, b"", b"use geo::shapes;\nmachine f() {}")),
        UNDECLARED_DEPENDENCY,
        1,
        b"use geo::shapes;",
        packages=app_geometry,
        dependencies=((0, BUILD, b"geo", 1),),
    )
    rejected_units(
        program,
        "another-requesters-alias-authorizes-nothing",
        ((2, b"m", b"pub data D { x: u8; }"), (0, b"", b"use lib::m;\nmachine f() {}")),
        UNRESOLVED_IMPORT,
        1,
        b"use lib::m;",
        packages=app_geometry + ((b"base", False),),
        dependencies=((1, PRODUCT, b"lib", 2),),
    )
    rejected_units(
        program,
        "dependency-lacks-the-module",
        ((1, b"shapes", b"pub data D { x: u8; }"), (0, b"", b"use geo::circles;\nmachine f() {}")),
        UNRESOLVED_IMPORT,
        1,
        b"use geo::circles;",
        packages=app_geometry,
        dependencies=geo,
    )
    rejected_units(
        program,
        "alias-alone-names-no-module",
        ((1, b"shapes", b"pub data D { x: u8; }"), (0, b"", b"use geo;\nmachine f() {}")),
        UNRESOLVED_IMPORT,
        1,
        b"use geo;",
        packages=app_geometry,
        dependencies=geo,
    )
    rejected_units(
        program,
        "private-data-across-packages",
        ((1, b"shapes", b"data D { x: u8; }"), (0, b"", b"use geo::shapes;\nmachine f(d: D) {}")),
        UNKNOWN_DATA_TYPE,
        1,
        b"D",
        packages=app_geometry,
        dependencies=geo,
    )
    rejected_units(
        program,
        "private-machine-across-packages",
        ((1, b"shapes", b"pub data D { x: u8; }\nmachine g(d: D) {}"),
         (0, b"", b"use geo::shapes;\nmachine f(d: D) { g(d); }")),
        UNKNOWN_NAME,
        1,
        b"g",
        occurrence=2,
        packages=app_geometry,
        dependencies=geo,
    )
    accepted_units(
        program,
        "public-trait-requirement-across-packages",
        ((1, b"shapes", b"pub trait Shape { machine area(x: u8) -> u8; }"),
         (0, b"", b"use geo::shapes;\nmachine f() { Shape::area(1); }")),
        (builtin("u8"), builtin("u8")),
        (machine(0), NOT_A_NAME, NOT_A_NAME),
        packages=app_geometry,
        dependencies=geo,
    )
    rejected_units(
        program,
        "private-trait-requirement-across-packages",
        ((1, b"shapes", b"trait Shape { machine area(x: u8) -> u8; }"),
         (0, b"", b"use geo::shapes;\nmachine f() { Shape::area(1); }")),
        UNKNOWN_NAME,
        1,
        b"Shape::area",
        packages=app_geometry,
        dependencies=geo,
    )
    accepted_units(
        program,
        "public-domain-across-packages",
        ((1, b"limits", b"pub domain u64::Small(value) requires value < 4;"),
         (0, b"", b"use geo::limits;\nmachine f(x: u64 in Small) {}")),
        (builtin("u64"), in_domain(builtin("u64"), 0)),
        packages=app_geometry,
        dependencies=geo,
    )
    rejected_units(
        program,
        "private-domain-across-packages",
        ((1, b"limits", b"domain u64::Small(value) requires value < 4;"),
         (0, b"", b"use geo::limits;\nmachine f(x: u64 in Small) {}")),
        UNKNOWN_DOMAIN,
        1,
        b"Small",
        packages=app_geometry,
        dependencies=geo,
    )
    same_spelled = ((0, b"shapes", b"data D { x: u8; }"), (1, b"shapes", b"pub data D { y: u16; }"))
    accepted_units(
        program,
        "same-spelled-modules-of-two-packages",
        same_spelled + ((0, b"", b"use geo::shapes;\nmachine f(d: D) {}"), (0, b"local", b"use shapes;\nmachine g(d: D) {}")),
        (builtin("u8"), builtin("u16"), data(1), data(0)),
        packages=app_geometry,
        dependencies=geo,
    )
    rejected_units(
        program,
        "same-spelled-modules-imported-together",
        same_spelled + ((0, b"", b"use shapes;\nuse geo::shapes;\nmachine f(d: D) {}"),),
        AMBIGUOUS_NAME,
        2,
        b"D",
        packages=app_geometry,
        dependencies=geo,
    )
    accepted_units(
        program,
        "alias-precedes-a-local-module",
        ((0, b"geo::shapes", b"data D { x: u8; }"), (1, b"shapes", b"pub data D { y: u16; }"),
         (0, b"", b"use geo::shapes;\nmachine f(d: D) {}")),
        (builtin("u8"), builtin("u16"), data(1)),
        packages=app_geometry,
        dependencies=geo,
    )
    toolchain = ((b"app", False), (b"", True))
    accepted_units(
        program,
        "toolchain-mount",
        ((1, b"nat", b"pub data Nat { x: u8; }"),
         (1, b"rat", b"use omega::language::core::nat;\npub data Rat { n: Nat; }"),
         (0, b"", b"use omega::language::core::rat;\nmachine f(r: Rat) {}")),
        (builtin("u8"), data(0), data(1)),
        packages=toolchain,
    )
    rejected_units(
        program,
        "toolchain-mount-claims-its-paths",
        ((b"omega::language::core::nat", b"data Nat { x: u8; }"),
         (b"", b"use omega::language::core::nat;\nmachine f(n: Nat) {}")),
        UNRESOLVED_IMPORT,
        1,
        b"use omega::language::core::nat;",
    )

    # The product's own psi package resolves as one source set: each module
    # under source/psi as `<directory>::<file>` of package `psi`, whose own
    # `use psi::...` rows select its modules by its name. Its build file
    # belongs to the build context, not the package's product sources.
    psi_root = Path(__file__).resolve().parents[3] / "source" / "psi"
    psi_units = tuple(
        ("::".join(path.relative_to(psi_root).with_suffix("").parts).encode(), path.read_bytes())
        for path in sorted(psi_root.rglob("*.omg"))
        if path.parent != psi_root
    )
    status, payload = run_units(program, psi_units, ((b"psi", False),))
    package = decode(payload) if status in (0, 249) else None
    assert status == 0, f"psi-package: status {status}" + (
        f": {package.diagnostic} at {package.diagnostic_span} in {psi_units[package.diagnostic_span[0]][0]!r}"
        if package else ""
    )
    assert package.accepted and package.diagnostic == 0, f"psi-package: {package.diagnostic} {package.diagnostic_span}"
    assert len(psi_units) >= 17 and len(package.bindings) > 0 and len(package.value_bindings) > 0, "psi-package"
    print(f"psi-package: {len(psi_units)} units, {len(package.bindings)} type references, "
          f"{len(package.value_bindings)} expressions, {len(package.path_bindings)} name paths")

    # Whole-program tables one unit's 65,536 tokens cannot fill reach their
    # capacity edges across units; one more row in a further unit rejects
    # while parsing. A sum of n literals `1+...+1` fills 2n - 1 expression
    # rows.
    def sum_constant(name: str, literals: int) -> bytes:
        return f"const {name}: u64 = ".encode() + b"+".join(b"1" for _ in range(literals)) + b";\n"

    exact_expressions = (
        (b"e0", sum_constant("A", 32000)),
        (b"e1", sum_constant("B", 32000)),
        (b"e2", sum_constant("C", 769) + sum_constant("D", 769)),
    )
    status, payload = run_units(program, exact_expressions)
    assert status == 0, f"expression-capacity: status {status}"
    assert len(decode(payload).value_bindings) == 131072, "expression-capacity"
    phase_rejected_units(program, "expression-capacity-plus-one",
                         exact_expressions + ((b"e3", sum_constant("E", 1)),), 250)

    # 32,768 parameters, 128 to a machine, each typed `u8` or `&u8`: the
    # second fills the type-reference table's 65,536 rows as well.
    def parameter_units(parameter_type: bytes) -> tuple[tuple[bytes, bytes], ...]:
        signature = b", ".join(f"p{index}: ".encode() + parameter_type for index in range(128))
        unit = b"".join(f"machine f{index}(".encode() + signature + b") {}\n" for index in range(64))
        return tuple((f"p{index}".encode(), unit) for index in range(4))

    exact_parameters = parameter_units(b"u8")
    status, payload = run_units(program, exact_parameters)
    assert status == 0, f"parameter-capacity: status {status}"
    assert len(decode(payload).bindings) == 32768, "parameter-capacity"
    phase_rejected_units(program, "parameter-capacity-plus-one",
                         exact_parameters + ((b"p4", b"machine g(q: u8) {}"),), 250)
    exact_type_references = parameter_units(b"&u8")
    status, payload = run_units(program, exact_type_references)
    assert status == 0, f"type-reference-capacity: status {status}"
    assert len(decode(payload).bindings) == 65536, "type-reference-capacity"
    phase_rejected_units(program, "type-reference-capacity-plus-one",
                         exact_type_references + ((b"p4", b"const X: u8 = 1;"),), 250)

    # 65,536 call arguments: a call stages at most 256, and 64 such calls
    # fill one unit.
    call = b"f(" + b",".join(b"1" for _ in range(256)) + b");\n"
    argument_unit = b"machine f() {\n" + call * 64 + b"}\n"
    exact_arguments = tuple((f"a{index}".encode(), argument_unit) for index in range(4))
    status, payload = run_units(program, exact_arguments)
    assert status == 0, f"argument-capacity: status {status}"
    argument_bindings = decode(payload).value_bindings
    assert len(argument_bindings) == 4 * 64 * 258, "argument-capacity"
    assert sum(1 for row in argument_bindings if row[0] == 6) == 256, "argument-capacity callees"
    phase_rejected_units(program, "argument-capacity-plus-one",
                         exact_arguments + ((b"a4", b"machine g() { g(1); }"),), 250)

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
