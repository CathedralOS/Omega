#!/usr/bin/env python3
"""Black-box checks for the live Omega-written Psi typing slice."""

from __future__ import annotations

import struct
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

# Tags are declaration ordinals, mirrored from source/psi/types/types.omg and
# source/psi/resolution/resolution.omg.
KINDS = (
    "Absent", "Builtin", "Data", "Reference", "Array", "Slice", "GenericParameter",
    "StaticInteger", "Unit", "AnonymousInteger", "AnonymousFloat",
)
BUILTIN_SPELLINGS = (
    "bool", "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "addr",
    "f32", "f64", "Slice", "Result", "SyscallResult", "Terminal", "Never",
    "UInt", "Int", "AtomicBool", "AtomicU32", "AtomicU64",
)
DOMAINS = ("", "Wrapping", "Saturating", "Trapping", "Finite")
DIAGNOSTICS = (
    "None", "TypeCapacityExceeded", "UnsupportedTypeForm", "GenericArgumentCount",
    "UnsupportedArrayLength", "UnsupportedDomain", "MismatchedTypes", "LiteralOutOfRange",
    "UnsupportedLiteral", "AnonymousValueOutOfRange", "AnonymousRemainder", "UnknownField",
    "ProjectionOfNonData", "IndexOfNonCollection", "NonIntegerIndex", "OperandMismatch",
    "NotBoolean", "UnsupportedEquality", "NoMatchingMachine", "AmbiguousMachine",
    "NotCallable", "NotAValue", "UnknownConstructionField",
    "DuplicateConstructionField", "NotConstructible", "CaseOfOtherData", "UnknownPatternField",
    "InvalidCast", "NotAPlace", "TerminalResultMismatch", "AnonymousFraction",
    "UnsupportedExpression", "TargetArgumentCount", "UninferredGenericArgument",
    "SatisfierMismatch", "SilentDiscard", "DiscardsNoCall", "PropositionArgumentCount",
)
SELECTIONS = ("none", "machine", "state", "field", "length")
ADAPTATIONS = ("", "landed", "lent")


@dataclass(frozen=True)
class TypeRow:
    kind: int
    builtin: int
    domain: int
    item: int
    access: int
    inner: int
    length: int
    first_argument: int
    argument_count: int


@dataclass(frozen=True)
class ExpressionRow:
    span: tuple[int, int, int]
    type: int
    selection: tuple[int, int]
    adaptation: int
    negative: bool
    magnitude: int


@dataclass(frozen=True)
class Observation:
    accepted: bool
    diagnostic: str
    diagnostic_span: tuple[int, int, int]
    rows: tuple[TypeRow, ...]
    arguments: tuple[int, ...]
    expressions: tuple[ExpressionRow, ...]
    binders: tuple[int, ...]

    def render(self, handle: int) -> str:
        """A type's spelling: builtins and domains as written, `&`/`&mut`,
        arrays, slices, data declarations as `D<handle>` with arguments,
        generic parameters as `G<handle>`, `()` for unit and `{integer}`
        for an anonymous integer."""
        row = self.rows[handle]
        kind = KINDS[row.kind]
        domain = f" in {DOMAINS[row.domain]}" if row.domain else ""
        if kind == "Builtin":
            return BUILTIN_SPELLINGS[row.builtin] + domain
        if kind == "Data":
            arguments = self.arguments[row.first_argument:row.first_argument + row.argument_count]
            applied = f"<{', '.join(self.render(argument) for argument in arguments)}>" if arguments else ""
            return f"D{row.item}{applied}"
        if kind == "Reference":
            return ("&mut " if row.access == 1 else "&") + self.render(row.inner)
        if kind == "Array":
            return f"[{self.render(row.inner)}; {row.length}]{domain}"
        if kind == "Slice":
            return f"[{self.render(row.inner)}]{domain}"
        if kind == "GenericParameter":
            return f"G{row.item}"
        if kind == "StaticInteger":
            return str(row.length)
        if kind == "Unit":
            return "()"
        if kind == "AnonymousInteger":
            return "{integer}"
        if kind == "AnonymousFloat":
            return "{float}"
        return "absent"


class Reader:
    def __init__(self, payload: bytes) -> None:
        self.payload = payload
        self.cursor = 0

    def bytes(self, count: int) -> bytes:
        end = self.cursor + count
        if end > len(self.payload):
            raise AssertionError("truncated typing observation")
        value = self.payload[self.cursor:end]
        self.cursor = end
        return value

    def byte(self) -> int:
        return self.bytes(1)[0]

    def u64(self) -> int:
        return struct.unpack("<Q", self.bytes(8))[0]

    def span(self) -> tuple[int, int, int]:
        return (self.u64(), self.u64(), self.u64())


def decode(payload: bytes) -> Observation:
    reader = Reader(payload)
    assert reader.bytes(8) == b"OMGTYP1\0"
    assert reader.u64() == 1
    accepted = reader.byte() == 1
    diagnostic = DIAGNOSTICS[reader.byte()]
    diagnostic_span = reader.span()
    rows = tuple(
        TypeRow(reader.byte(), reader.byte(), reader.byte(), reader.u64(), reader.byte(),
                reader.u64(), reader.u64(), reader.u64(), reader.u64())
        for _ in range(reader.u64())
    )
    arguments = tuple(reader.u64() for _ in range(reader.u64()))
    expressions = tuple(
        ExpressionRow(reader.span(), reader.u64(), (reader.byte(), reader.u64()), reader.byte(),
                      reader.byte() == 1, reader.u64())
        for _ in range(reader.u64())
    )
    binders = tuple(reader.u64() for _ in range(reader.u64()))
    assert reader.cursor == len(payload), "trailing typing observation bytes"
    return Observation(accepted, diagnostic, diagnostic_span, rows, arguments, expressions, binders)


# Every distinct input the gate runs is one case; the determinism check runs
# its input twice and counts once.
CASE_SOURCES: set[bytes] = set()


# The source set's packages, as (name, toolchain) rows. The typing cases
# run one ordinary package that every unit belongs to, and no dependency
# edges; the resolver gate owns package selection.
GATE_PACKAGES = ((b"gate", False),)


def frame(units: tuple[tuple[bytes, bytes], ...], packages: tuple[tuple[bytes, bool], ...] = GATE_PACKAGES) -> bytes:
    """The harness input, framed like the request's subject tables: the
    packages (a toolchain flag and a `u32`-framed name), no dependency
    edges, then the units, each in package 0 with its framed module path and
    source. A unit's position is its source id."""
    payload = struct.pack("<I", len(packages))
    for name, toolchain in packages:
        payload += struct.pack("<II", 1 if toolchain else 0, len(name)) + name
    payload += struct.pack("<II", 0, len(units))
    for module_path, source in units:
        payload += struct.pack("<II", 0, len(module_path)) + module_path
        payload += struct.pack("<I", len(source)) + source
    return payload


def run_units(
    program: str,
    units: tuple[tuple[bytes, bytes], ...],
    packages: tuple[tuple[bytes, bool], ...] = GATE_PACKAGES,
) -> tuple[int, bytes]:
    payload = frame(units, packages)
    CASE_SOURCES.add(payload)
    completed = subprocess.run([program], input=payload, capture_output=True, check=False)
    assert completed.stderr == b"", completed.stderr.decode(errors="replace")
    return completed.returncode, completed.stdout


def run(program: str, source: bytes) -> tuple[int, bytes]:
    """One source unit, the root module."""
    return run_units(program, ((b"", source),))


def span_of(source: bytes, spelling: bytes, occurrence: int = 1) -> tuple[int, int, int]:
    start = -1
    for _ in range(occurrence):
        start = source.index(spelling, start + 1)
    return (0, start, start + len(spelling))


def expression_at(observation: Observation, span: tuple[int, int, int]) -> ExpressionRow:
    """The outermost expression authored exactly at `span`."""
    matches = [row for row in observation.expressions if row.span == span]
    assert matches, f"no expression at {span}"
    return matches[-1]


def accepted(
    program: str,
    name: str,
    source: bytes,
    types: tuple[tuple[bytes, str], ...],
    selections: tuple[tuple[bytes, str, int], ...] = (),
    adaptations: tuple[tuple[bytes, str], ...] = (),
) -> Observation:
    """Each `(spelling, type)` names the first expression authored exactly as
    `spelling` and its rendered type; `(spelling!n, ...)` picks the n-th."""
    status, payload = run(program, source)
    assert status == 0, f"{name}: status {status}"
    observation = decode(payload)
    assert observation.accepted and observation.diagnostic == "None", (
        f"{name}: {observation.diagnostic} at {observation.diagnostic_span}"
    )
    for spelling, expected in types:
        row = expression_at(observation, located(source, spelling))
        actual = observation.render(row.type)
        assert actual == expected, f"{name}: {spelling!r} has {actual}, expected {expected}"
    for spelling, kind, handle in selections:
        row = expression_at(observation, located(source, spelling))
        actual = (SELECTIONS[row.selection[0]], row.selection[1])
        assert actual == (kind, handle), f"{name}: {spelling!r} selects {actual}, expected {(kind, handle)}"
    for spelling, expected in adaptations:
        row = expression_at(observation, located(source, spelling))
        assert ADAPTATIONS[row.adaptation] == expected, (
            f"{name}: {spelling!r} adapts {ADAPTATIONS[row.adaptation]!r}, expected {expected!r}"
        )
    return observation


def located(source: bytes, spelling: bytes) -> tuple[int, int, int]:
    text, _, occurrence = spelling.partition(b"!")
    return span_of(source, text, int(occurrence) if occurrence else 1)


def rejected(
    program: str,
    name: str,
    source: bytes,
    diagnostic: str,
    spelling: bytes,
) -> None:
    """The diagnostic names the authored `spelling` (`text!n` for the n-th)."""
    status, payload = run(program, source)
    assert status == 249, f"{name}: status {status}"
    observation = decode(payload)
    actual = (observation.accepted, observation.diagnostic, observation.diagnostic_span)
    expected = (False, diagnostic, located(source, spelling))
    assert actual == expected, f"{name}:\nexpected {expected}\nactual   {actual}"


def phase_rejected(program: str, name: str, source: bytes, status: int) -> None:
    actual_status, payload = run(program, source)
    assert actual_status == status, f"{name}: status {actual_status}"
    assert payload == b"", name


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: test_typing.py <omega-typing-harness>")
    program = sys.argv[1]

    accepted(program, "empty", b"", ())
    accepted(
        program,
        "declared-types-and-landed-literals",
        b"machine f() {\n"
        b"    let small: u8 = 200;\n"
        b"    let count: u64 in Trapping = small as u64 in Trapping + 1;\n"
        b"    let negative: i8 = -128;\n"
        b"    let folded: u16 = 250 * 4 + 24;\n"
        b"}",
        (
            (b"200", "u8"),
            (b"small as u64 in Trapping", "u64 in Trapping"),
            (b"1", "u64 in Trapping"),
            (b"small as u64 in Trapping + 1", "u64 in Trapping"),
            (b"-128", "i8"),
            (b"250 * 4 + 24", "u16"),
            (b"250 * 4", "{integer}"),
        ),
        adaptations=((b"200", "landed"), (b"1", "landed"), (b"250 * 4 + 24", "landed")),
    )
    accepted(
        program,
        "suffixed-literals-keep-their-carrier",
        b"machine f() { let a: u8 = 7u8; let b: bool = 300u16 > 299; }",
        ((b"7u8", "u8"), (b"300u16", "u16"), (b"299", "u16"), (b"300u16 > 299", "bool")),
    )
    accepted(
        program,
        "fields-through-references",
        b"data P { x: u32; inner: Q; }\ndata Q { flag: bool; }\n"
        b"machine f(p: &P, q: &mut Q) -> bool {\n"
        b"    q.flag = p.inner.flag;\n"
        b"    p.x > 3\n"
        b"}",
        (
            (b"p!2", "&D0"),
            (b"p.inner", "D1"),
            (b"p.inner.flag", "bool"),
            (b"q.flag", "bool"),
            (b"p.x", "u32"),
            (b"p.x > 3", "bool"),
        ),
        selections=((b"p.inner", "field", 1), (b"p.x", "field", 0), (b"q.flag", "field", 2)),
    )
    accepted(
        program,
        "arrays-and-indexing",
        b"data B { bytes: [u8; 4] in Trapping; count: u64; }\n"
        b"machine f(b: &B, i: u64) -> u8 in Trapping { b.bytes[i] }\n"
        b"machine g(b: &B) -> u8 in Trapping { b.bytes[2] }",
        (
            (b"b.bytes", "[u8; 4] in Trapping"),
            (b"b.bytes[i]", "u8 in Trapping"),
            (b"2", "u64"),
        ),
    )
    accepted(
        program,
        "policy-qualified-collections-read-and-store",
        b"data B { counts: [u64; 4] in Trapping; }\n"
        b"machine f(b: &mut B, value: u64 in Trapping) -> u64 in Trapping {\n"
        b"    b.counts[0] = value as u64;\n"
        b"    b.counts[1] + 1\n"
        b"}",
        ((b"b.counts[0]", "u64 in Trapping"), (b"b.counts[1] + 1", "u64 in Trapping"), (b"value as u64", "u64")),
    )
    accepted(
        program,
        "collection-lengths",
        b"machine f(view: &[u8], fixed: [u16; 3]) -> bool { view.len == fixed.len }",
        ((b"view.len", "u64"), (b"fixed.len", "u64"), (b"view.len == fixed.len", "bool")),
        selections=((b"view.len", "length", 0), (b"fixed.len", "length", 0)),
    )
    accepted(
        program,
        "overloads-select-by-argument-types",
        b"machine A::pick(x: u8) -> u8 { x }\n"
        b"machine A::pick(x: bool) -> bool { x }\n"
        b"machine f() -> bool { A::pick(true) }\n"
        b"machine g(v: u8) -> u8 { A::pick(v) }",
        ((b"A::pick(true)", "bool"), (b"A::pick(v)", "u8")),
        selections=((b"A::pick(true)", "machine", 1), (b"A::pick(v)", "machine", 0)),
    )
    accepted(
        program,
        "shared-parameters-lend-implicitly",
        b"data P { x: u32; }\n"
        b"machine read(p: &P) -> u32 { p.x }\n"
        b"machine f(p: P, q: &mut P) -> u32 {\n"
        b"    let a: u32 = read(p);\n"
        b"    read(q)\n"
        b"}",
        ((b"read(p)", "u32"), (b"read(q)", "u32")),
        adaptations=((b"p!4", "lent"), (b"q!2", "")),
    )
    accepted(
        program,
        "borrows-through-references-reborrow-the-referent",
        b"data P { x: u32; }\n"
        b"machine bump(p: &mut P) { p.x = 1; }\n"
        b"machine f(q: &mut P) { bump(&mut q); bump(q); }",
        ((b"&mut q", "&mut D0"),),
    )
    accepted(
        program,
        "constructions-and-case-values",
        b"data P { x: u32; y: bool; }\n"
        b"data S { case Empty; case Full(v: u8); }\n"
        b"machine f() -> S {\n"
        b"    let p: P = P { x: 5, y: true };\n"
        b"    let e: S = S::Empty;\n"
        b"    S::Full { v: 9 }\n"
        b"}",
        (
            (b"P { x: 5, y: true }", "D0"),
            (b"5", "u32"),
            (b"S::Empty", "D1"),
            (b"S::Full { v: 9 }", "D1"),
            (b"9", "u8"),
        ),
    )
    observation = accepted(
        program,
        "case-patterns-bind-payload-types",
        b"data S { case Empty; case Full(v: u8, w: bool); }\n"
        b"machine f(s: &S) -> u8 {\n"
        b"    transition s {\n"
        b"        S::Full { v, w: true } -> got(v)\n"
        b"        _ -> got(0)\n"
        b"    }\n"
        b"\n"
        b"    state got(value: u8) -> u8 {\n"
        b"        value\n"
        b"    }\n"
        b"}",
        ((b"got(v)", "()"), (b"v!3", "u8"), (b"true", "bool"), (b"0", "u8"), (b"value!2", "u8")),
        selections=((b"got(v)", "state", 0), (b"got(0)", "state", 0)),
    )
    assert observation.render(observation.binders[0]) == "u8", "case-patterns-bind-payload-types: binder"
    accepted(
        program,
        "guards-and-equality",
        b"data L { case On; case Off; }\n"
        b"machine f(a: u8, b: i64, l: L) -> bool {\n"
        b"    transition {\n"
        b"        a < 3 && b == -1 -> yes()\n"
        b"        l == L::On -> yes()\n"
        b"        _ -> no()\n"
        b"    }\n"
        b"\n"
        b"    state yes() -> bool { true }\n"
        b"    state no() -> bool { false }\n"
        b"}",
        ((b"a < 3", "bool"), (b"3", "u8"), (b"-1", "i64"), (b"l == L::On", "bool")),
    )
    accepted(
        program,
        "terminal-calls-complete-the-machine",
        b"machine g(x: u8) -> u8 { x }\n"
        b"machine f(x: u8) -> u8 {\n"
        b"    transition { _ -> g(x) }\n"
        b"}",
        ((b"g(x)", "u8"),),
        selections=((b"g(x)", "machine", 0),),
    )

    accepted(
        program,
        "generic-machines-infer-and-apply",
        b"data Box<T> { value: T; }\n"
        b"machine id<T>(x: T) -> T { x }\n"
        b"machine Box::get<T>(b: &Box<T>) -> T { b.value }\n"
        b"machine f(b: Box<u8>) -> u8 {\n"
        b"    let a: u8 = id(5u8);\n"
        b"    let c: u16 = id<u16>(7);\n"
        b"    Box::get(&b)\n"
        b"}",
        (
            (b"b.value", "G2"),
            (b"id(5u8)", "u8"),
            (b"id<u16>(7)", "u16"),
            (b"7", "u16"),
            (b"Box::get(&b)", "u8"),
        ),
        selections=((b"id(5u8)", "machine", 0), (b"id<u16>(7)", "machine", 0), (b"Box::get(&b)", "machine", 1)),
    )
    accepted(
        program,
        "generic-data-constructions-and-patterns",
        b"data Opt<T> { case None; case Some(v: T); }\n"
        b"data Pair<A, B> { first: A; second: B; }\n"
        b"machine make(x: u16) -> Pair<u16, bool> { Pair { first: x, second: true } }\n"
        b"machine h(o: &Opt<u16>) -> u16 {\n"
        b"    transition o {\n"
        b"        Opt::Some { v } -> got(v)\n"
        b"        _ -> got(0)\n"
        b"    }\n"
        b"\n"
        b"    state got(x: u16) -> u16 { x }\n"
        b"}\n"
        b"machine wrap(x: u16) -> Opt<u16> { Opt::Some { v: x } }",
        (
            (b"Pair { first: x, second: true }", "D1<u16, bool>"),
            (b"got(v)", "()"),
            (b"Opt::Some { v: x }", "D0<u16>"),
        ),
    )
    accepted(
        program,
        "floats-land-at-their-format",
        b"machine k(x: f64) -> bool {\n"
        b"    let y: f32 = 1.5;\n"
        b"    let z: f64 = 2.0 * 3;\n"
        b"    let w: f64 = -0.5;\n"
        b"    x < 2.5\n"
        b"}",
        (
            (b"1.5", "f32"),
            (b"2.0 * 3", "f64"),
            (b"2.0", "{float}"),
            (b"-0.5", "f64"),
            (b"2.5", "f64"),
            (b"x < 2.5", "bool"),
        ),
        adaptations=((b"1.5", "landed"), (b"2.0 * 3", "landed")),
    )

    rejected(program, "let-mismatch", b"machine f() { let a: u8 = true; }", "MismatchedTypes", b"true")
    rejected(program, "literal-out-of-range", b"machine f() { let a: u8 = 256; }", "LiteralOutOfRange", b"256")
    rejected(
        program,
        "folded-out-of-range",
        b"machine f() { let a: u8 = 250 + 6; }",
        "LiteralOutOfRange",
        b"250 + 6",
    )
    rejected(
        program,
        "anonymous-fraction",
        b"machine f() { let a: u8 = 7 / 2; }",
        "AnonymousFraction",
        b"7 / 2",
    )
    rejected(
        program,
        "anonymous-remainder-needs-a-typed-operand",
        b"machine f() { let a: u8 = 7 % 2; }",
        "AnonymousRemainder",
        b"7 % 2",
    )
    rejected(
        program,
        "no-implicit-widening",
        b"machine f(small: u8) { let wide: u16 = small; }",
        "MismatchedTypes",
        b"small!2",
    )
    rejected(
        program,
        "policies-do-not-mix",
        b"machine f(wrapped: u64 in Wrapping, plain: u64) { let sum: u64 = wrapped + plain; }",
        "MismatchedTypes",
        b"plain!2",
    )
    rejected(
        program,
        "unknown-field",
        b"data P { x: u8; }\nmachine f(p: P) -> u8 { p.y }",
        "UnknownField",
        b"y",
    )
    rejected(
        program,
        "projection-of-a-scalar",
        b"machine f(p: u8) -> u8 { p.y }",
        "ProjectionOfNonData",
        b"p.y",
    )
    rejected(
        program,
        "no-matching-overload",
        b"machine A::pick(x: u8) -> u8 { x }\nmachine f() -> u8 { A::pick(true) }",
        "NoMatchingMachine",
        b"A::pick(true)",
    )
    rejected(
        program,
        "ambiguous-overload",
        b"machine A::pick(x: u8) -> u8 { x }\nmachine A::pick(x: u16) -> u8 { 0 }\n"
        b"machine f() -> u8 { A::pick(1) }",
        "AmbiguousMachine",
        b"A::pick(1)",
    )
    rejected(
        program,
        "mutable-loans-are-explicit",
        b"data P { x: u32; }\nmachine bump(p: &mut P) { p.x = 1; }\nmachine f(p: P) { bump(p); }",
        "NoMatchingMachine",
        b"bump(p)",
    )
    rejected(
        program,
        "guard-is-boolean",
        b"machine f(level: u8) { transition { level -> g() } state g() {} }",
        "NotBoolean",
        b"level!2",
    )
    rejected(
        program,
        "case-of-other-data",
        b"data S { case A; }\ndata T { case B; }\n"
        b"machine f(s: S) { transition s { T::B -> g() } state g() {} }",
        "CaseOfOtherData",
        b"T::B",
    )
    rejected(
        program,
        "payload-case-needs-fields",
        b"data S { case Full(v: u8); }\nmachine f() -> S { S::Full }",
        "NotAValue",
        b"S::Full",
    )
    rejected(
        program,
        "record-equality-needs-a-declaration",
        b"data P { x: u8; }\nmachine f(a: P, b: P) -> bool { a == b }",
        "UnsupportedEquality",
        b"a == b",
    )
    rejected(
        program,
        "invalid-cast",
        b"data P { x: u8; }\nmachine f(p: P) -> u8 { p as u8 }",
        "InvalidCast",
        b"p as u8",
    )
    rejected(
        program,
        "assignment-needs-a-place",
        b"machine g() -> u8 { 1 }\nmachine f() { g() = 2; }",
        "NotAPlace",
        b"g()!2",
    )
    rejected(
        program,
        "terminal-result-mismatch",
        b"machine g() -> bool { true }\nmachine f() -> u8 { transition { _ -> g() } }",
        "MismatchedTypes",
        b"g()!2",
    )
    rejected(
        program,
        "unit-completion-needs-a-unit-body",
        b"machine f() -> u8 { transition { _ -> {} } }",
        "TerminalResultMismatch",
        b"_ -> {}",
    )
    rejected(
        program,
        "generic-arguments-must-be-inferable",
        b"machine id<T>(x: T) -> T { x }\nmachine f() -> u8 { id(5) }",
        "NoMatchingMachine",
        b"id(5)",
    )
    rejected(
        program,
        "explicit-arguments-fit-the-machine",
        b"machine id<T>(x: T) -> T { x }\nmachine f() -> u8 { id<u8, u8>(5) }",
        "NoMatchingMachine",
        b"id<u8, u8>(5)",
    )
    rejected(
        program,
        "construction-arguments-must-be-inferable",
        b"data Box<T> { value: T; }\nmachine f() -> Box<u8> { Box { value: 9 } }",
        "UninferredGenericArgument",
        b"Box { value: 9 }",
    )
    rejected(
        program,
        "floats-do-not-cast-to-integers",
        b"machine f() -> u8 { 1.5 as u8 }",
        "InvalidCast",
        b"1.5 as u8",
    )
    rejected(
        program,
        "float-formats-do-not-mix",
        b"machine f(a: f32, b: f64) -> f64 { a + b }",
        "MismatchedTypes",
        b"b!2",
    )
    rejected(
        program,
        "target-argument-count",
        b"machine f() { transition { _ -> g(1) } state g() {} }",
        "TargetArgumentCount",
        b"g(1)",
    )

    # Expression forms typing does not judge yet reject at the expression
    # rather than leaving it untyped: a value-position match and a case
    # membership both resolve. A renamed pattern field types as its field.
    rejected(
        program,
        "match-is-unsupported",
        b"machine f(c: u8) -> u8 { match c { 1 -> 2, _ -> 3 } }",
        "UnsupportedExpression",
        b"match c { 1 -> 2, _ -> 3 }",
    )
    rejected(
        program,
        "membership-is-unsupported",
        b"data O { case None; case Some(value: u8); }\nmachine f(o: O) -> bool { o in O::None }",
        "UnsupportedExpression",
        b"o in O::None",
    )
    accepted(
        program,
        "renamed-pattern-binder",
        b"data N { case Zero; case Succ(prev: u8); }\nmachine f(n: N) -> u8 { transition n { N::Succ { prev as w } -> w _ -> 0 } }",
        ((b"w!2", "u8"),),
    )

    # Constants are values of their declared types, and each constant's
    # value types against that type like a `let` initializer.
    accepted(
        program,
        "constants",
        b"data Pair { x: u8; }\nconst LIMIT: u64 = 4096;\nconst Pair::ZERO: Pair = Pair { x: 0 };\n"
        b"machine f() -> u64 { LIMIT + 1 }\nmachine g() -> u8 { let zero: Pair = Pair::ZERO; zero.x }",
        ((b"4096", "u64"), (b"LIMIT!2", "u64"), (b"LIMIT + 1", "u64"), (b"Pair::ZERO!2", "D0"), (b"0!2", "u8")),
        adaptations=((b"4096", "landed"), (b"0!2", "landed")),
    )
    rejected(program, "constant-value-mismatch", b"const SMALL: u8 = true;", "MismatchedTypes", b"true")
    rejected(program, "constant-value-out-of-range", b"const SMALL: u8 = 256;", "LiteralOutOfRange", b"256")

    # A conformance member of a non-generic conformance is an ordinary
    # machine; a trait requirement needs a selected conformance, a generic
    # conformance's member its arguments, a target-scoped family a
    # realization, and a declared domain's theory checking: each rejects as
    # unsupported rather than typing partially.
    accepted(
        program,
        "conformance-member-call",
        b"trait Ranked<T> { machine rank(value: T) -> u64; }\n"
        b"ByValue satisfies Ranked<u64> { machine rank(value: u64) -> u64 { value } }\n"
        b"machine f() -> u64 { ByValue::rank(7) }",
        ((b"ByValue::rank(7)", "u64"), (b"7", "u64")),
        selections=((b"ByValue::rank(7)", "machine", 1),),
    )
    rejected(
        program,
        "trait-requirement-call",
        b"trait Order<T> { machine before(left: T, right: T) -> bool; }\nmachine f() -> bool { Order::before(1, 2) }",
        "UnsupportedExpression",
        b"Order::before(1, 2)",
    )
    rejected(
        program,
        "generic-conformance-member-call",
        b"trait Wrap<T> { machine wrap(value: T) -> T; }\n"
        b"Same<T> satisfies Wrap<T> { machine wrap(value: T) -> T { value } }\nmachine f() -> u8 { Same::wrap(7) }",
        "UnsupportedExpression",
        b"Same::wrap(7)",
    )
    rejected(
        program,
        "target-scoped-family-call",
        b"linux_arm64 machine Clock::now() -> u64 { 1 }\nmacos_arm64 machine Clock::now() -> u64 { 2 }\n"
        b"machine read() -> u64 { Clock::now() }",
        "UnsupportedExpression",
        b"Clock::now()!3",
    )
    rejected(
        program,
        "declared-domain",
        b"domain u64::Small(value) requires value < 4;\nmachine f(x: u64 in Small) {}",
        "UnsupportedDomain",
        b"u64 in Small",
    )
    rejected(
        program,
        "path-qualified-declared-domain",
        b"domain u64::Small(value) requires value < 4;\nmachine f(x: u64 in u64::Small) {}",
        "UnsupportedDomain",
        b"u64 in u64::Small",
    )
    rejected(
        program,
        "case-domain-qualifier",
        b"data O { case None; case Some(value: u8); }\nmachine f(o: O in O::Some) {}",
        "UnsupportedDomain",
        b"O in O::Some",
    )
    # An arithmetic or order operator over data selects, by its operands,
    # the machine binding its token at an operand's data.
    vector = b"data V { x: u64; }\nmachine + V::add(a: V, b: V) -> V { a }\nmachine + V::shift(a: V, b: u64) -> V { a }\n"
    accepted(
        program,
        "operator-selects-its-home-machine",
        vector + b"machine f(a: V, b: V) -> V { a + b }",
        ((b"a + b", "D0"),),
        selections=((b"a + b", "machine", 0),),
    )
    accepted(
        program,
        "operator-selects-by-operand",
        vector + b"machine f(a: V) -> V { a + 3 }",
        ((b"a + 3", "D0"), (b"3", "u64")),
        selections=((b"a + 3", "machine", 1),),
    )
    accepted(
        program,
        "order-operator-over-data",
        b"data V { x: u64; }\nmachine < V::before(a: V, b: V) -> bool { true }\nmachine f(a: V, b: V) -> bool { a < b }",
        ((b"a < b", "bool"),),
        selections=((b"a < b", "machine", 0),),
    )
    rejected(
        program,
        "operator-without-a-machine",
        b"data V { x: u64; }\nmachine f(a: V, b: V) -> V { a * b }",
        "OperandMismatch",
        b"a * b",
    )

    # A proposition application is a Boolean statement whose arguments meet
    # the proposition's parameters by ordinary admission; a generic
    # proposition binds its parameters like a generic machine call.
    accepted(
        program,
        "proposition-application-is-boolean",
        b"proposition Close(a: u64, b: u8) = a > 0;\nmachine f(x: u64)\nrequires Close(x, 3)\n{}",
        ((b"Close(x, 3)", "bool"), (b"x!2", "u64"), (b"3", "u8")),
        adaptations=((b"3", "landed"),),
    )
    accepted(
        program,
        "generic-proposition-application",
        b"proposition Same<T>(a: T, b: T);\nmachine f(x: u16, y: u16)\nrequires Same(x, y), Same<u16>(x, 7)\n{}",
        ((b"Same(x, y)", "bool"), (b"Same<u16>(x, 7)", "bool"), (b"7", "u16")),
    )
    accepted(
        program,
        "data-fact-proposition-application",
        b"proposition Ordered(lo: u64, hi: u64) = lo <= hi;\n"
        b"data Range\nwhere\n    Ordered(lo, hi),\n{ lo: u64; hi: u64; }",
        ((b"Ordered(lo, hi)", "bool"), (b"lo!3", "u64")),
    )
    rejected(
        program,
        "proposition-argument-count",
        b"proposition P(a: u64);\nmachine f(x: u64)\nrequires P(x, x)\n{}",
        "PropositionArgumentCount",
        b"P(x, x)",
    )
    rejected(
        program,
        "proposition-argument-mismatch",
        b"proposition P(a: u64);\nmachine f(flag: bool)\nrequires P(flag)\n{}",
        "MismatchedTypes",
        b"flag!2",
    )
    rejected(
        program,
        "uninferred-proposition-binder",
        b"proposition P<T>(a: u8);\nmachine f(x: u8)\nrequires P(x)\n{}",
        "UninferredGenericArgument",
        b"P(x)",
    )
    rejected(
        program,
        "proposition-static-argument-count",
        b"proposition P(a: u8);\nmachine f(x: u8)\nrequires P<u8>(x)\n{}",
        "GenericArgumentCount",
        b"P<u8>(x)",
    )

    # Assigning a name that holds an exclusive loan writes its referent
    # (a borrowed argument denotes the original storage); a shared loan is
    # not writable storage.
    accepted(
        program,
        "assignment-through-an-exclusive-loan",
        b"data P { x: u8; }\nmachine set(n: &mut u64, p: &mut P) { n = 99; p = P { x: 1 }; }",
        ((b"99", "u64"), (b"P { x: 1 }", "D0")),
        adaptations=((b"99", "landed"),),
    )
    rejected(
        program,
        "assignment-through-a-shared-loan",
        b"machine set(n: &u64) { n = 99; }",
        "MismatchedTypes",
        b"99",
    )

    # A case's own `where` fact types over its payload fields and the
    # declaration's fields, and is a Boolean.
    accepted(
        program,
        "case-fact-over-its-payload",
        b"data Interval { tag: u64; case Empty; case Range(lo: u64, hi: u32) where lo <= 9 && hi > 0 && tag > 0; }",
        ((b"lo!2", "u64"), (b"hi!2", "u32"), (b"tag!2", "u64"), (b"lo <= 9", "bool")),
    )
    rejected(
        program,
        "case-fact-is-a-boolean",
        b"data I { case A(x: u64) where x + 1; }",
        "NotBoolean",
        b"x + 1",
    )

    # A fact whose root is the set's first expression types.
    accepted(
        program,
        "fact-rooted-at-the-first-expression",
        b"machine f(b: bool)\nrequires b\n{}",
        ((b"b!3", "bool"),),
    )

    # A non-Unit result is dropped only through `_ = call();`.
    accepted(
        program,
        "intentional-discard",
        b"machine g() -> u8 { 1 }\nmachine f() { _ = g(); }",
        ((b"g()!2", "u8"),),
    )
    rejected(
        program,
        "silent-discard",
        b"machine g() -> u8 { 1 }\nmachine f() { g(); }",
        "SilentDiscard",
        b"g()!2",
    )
    rejected(
        program,
        "discard-of-a-value",
        b"machine f(x: u8) { _ = x; }",
        "DiscardsNoCall",
        b"x!2",
    )

    # A `satisfies` machine's signature is its requirement's under the
    # clause's trait arguments.
    order = b"trait Order<T> { machine before(left: T, right: T) -> bool; }\n"
    accepted(
        program,
        "satisfier-matches-its-requirement",
        order + b"machine ascending(left: u8, right: u8) -> bool satisfies Order<u8>::before { left < right }",
        ((b"left < right", "bool"),),
    )
    rejected(
        program,
        "satisfier-parameter-differs",
        order + b"machine ascending(left: u16, right: u8) -> bool satisfies Order<u8>::before { true }",
        "SatisfierMismatch",
        b"satisfies Order<u8>::before",
    )
    rejected(
        program,
        "satisfier-result-differs",
        order + b"machine ascending(left: u8, right: u8) -> u8 satisfies Order<u8>::before { 0 }",
        "SatisfierMismatch",
        b"satisfies Order<u8>::before",
    )
    rejected(
        program,
        "satisfies-argument-count",
        order + b"machine ascending(left: u8, right: u8) -> bool satisfies Order::before { true }",
        "GenericArgumentCount",
        b"satisfies Order::before",
    )
    rejected(
        program,
        "conformance-argument",
        b"trait Order<T> { machine before(left: T, right: T) -> bool; }\nAscending satisfies Order<u8> { }\n"
        b"machine sort<T, O>(value: T) {}\nmachine f() { sort<u8, Ascending>(1); }",
        "UnsupportedTypeForm",
        b"Ascending!2",
    )

    # The product's own psi package types as one source set: each module
    # under source/psi as `<directory>::<file>` of package `psi`.
    psi_root = Path(__file__).resolve().parents[3] / "source" / "psi"
    psi_units = tuple(
        ("::".join(path.relative_to(psi_root).with_suffix("").parts).encode(), path.read_bytes())
        for path in sorted(psi_root.rglob("*.omg"))
        if path.parent != psi_root
    )
    status, payload = run_units(program, psi_units, ((b"psi", False),))
    package = decode(payload)
    assert status == 0, f"psi-package: status {status}: {package.diagnostic} at {package.diagnostic_span}"
    assert package.accepted and len(package.expressions) > 0, "psi-package"
    untyped = [row for row in package.expressions if row.type == 0]
    print(f"psi-package: {len(package.rows)} types, {len(package.expressions)} expressions, "
          f"{len(untyped)} denote no value")

    # The expression table's capacity, 131,072 rows across units, types
    # whole: a sum of n anonymous literals `1+...+1` fills 2n - 1 rows and
    # lands at its constant's type.
    def sum_constant(name: str, literals: int) -> bytes:
        return f"const {name}: u64 = ".encode() + b"+".join(b"1" for _ in range(literals)) + b";\n"

    status, payload = run_units(program, (
        (b"e0", sum_constant("A", 32000)),
        (b"e1", sum_constant("B", 32000)),
        (b"e2", sum_constant("C", 769) + sum_constant("D", 769)),
    ))
    capacity = decode(payload)
    assert status == 0 and capacity.accepted, f"expression-capacity: status {status}: {capacity.diagnostic}"
    assert len(capacity.expressions) == 131072, "expression-capacity"

    phase_rejected(program, "resolution-rejection-has-no-observation", b"data A { x: Q; }", 248)
    phase_rejected(program, "parse-rejection-has-no-observation", b"data A { x: ; }", 250)
    phase_rejected(program, "lex-rejection-has-no-observation", b"\x80", 251)

    determinism = b"data P { x: u8; }\nmachine f(p: &P) -> u16 { p.x as u16 + 1 }"
    first = run(program, determinism)
    second = run(program, determinism)
    assert first == second, "determinism"
    assert first[0] == 0, first

    print(f"Psi typing slices: {len(CASE_SOURCES)} cases passed")


if __name__ == "__main__":
    main()
