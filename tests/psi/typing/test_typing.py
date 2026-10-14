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
    "StaticInteger", "Unit", "AnonymousInteger", "AnonymousFloat", "AnonymousChoice",
    "AnonymousArray",
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
    "UncoveredDispatch", "ImmutablePlace", "ImplicitAttenuation",
)
SELECTIONS = ("none", "machine", "state", "field", "length", "binder")
ADAPTATIONS = ("", "landed", "lent", "qualified", "read")


@dataclass(frozen=True)
class TypeRow:
    kind: int
    builtin: int
    domain: int
    domain_item: int
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
        """A type's spelling: builtins and policies as written, a declared
        domain as `Domain<handle>`, `&`/`&mut`,
        arrays, slices, data declarations as `D<handle>` with arguments,
        generic parameters as `G<handle>`, `()` for unit and `{integer}`
        for an anonymous integer."""
        row = self.rows[handle]
        kind = KINDS[row.kind]
        if row.domain == 5:
            domain = f" in Domain{row.domain_item}"
        elif row.domain == 6:
            domain = f" in Case{row.domain_item}"
        else:
            domain = f" in {DOMAINS[row.domain]}" if row.domain else ""
        if kind == "Builtin":
            return BUILTIN_SPELLINGS[row.builtin] + domain
        if kind == "Data":
            arguments = self.arguments[row.first_argument:row.first_argument + row.argument_count]
            applied = f"<{', '.join(self.render(argument) for argument in arguments)}>" if arguments else ""
            return f"D{row.item}{applied}"
        if kind == "Reference":
            access = {0: "&", 1: "&mut ", 2: "&write "}[row.access]
            return access + self.render(row.inner)
        if kind == "Array":
            return f"[{self.render(row.inner)}; {self.render(row.length)}]{domain}"
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
        if kind == "AnonymousChoice":
            return "{choice}"
        if kind == "AnonymousArray":
            return f"{{[{self.render(row.inner)}; {self.render(row.length)}]}}"
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
        TypeRow(reader.byte(), reader.byte(), reader.byte(), reader.u64(), reader.u64(), reader.byte(),
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
    # A suffix chooses the carrier, not the policy: the literal lands at a
    # policy-qualified destination of that carrier, where a typed value of
    # another policy does not.
    accepted(
        program,
        "suffixed-literals-land-under-a-destination-policy",
        b"machine g(w: u32 in Wrapping) -> u32 in Wrapping { w + 1u32 }\n"
        b"machine f() -> u32 in Wrapping {\n    let mut w: u32 in Wrapping = 0u32 as u32 in Wrapping;\n"
        b"    w = 5u32;\n    g(5u32)\n}",
        ((b"w + 1u32", "u32 in Wrapping"), (b"1u32", "u32 in Wrapping"), (b"5u32!2", "u32 in Wrapping")),
        adaptations=((b"1u32", "landed"), (b"5u32!2", "landed")),
    )
    rejected(
        program,
        "a-suffixed-literal-keeps-its-carrier-under-a-policy",
        b"machine f() { let mut w: u32 in Wrapping = 0u32 as u32 in Wrapping; w = 5u8; }",
        "MismatchedTypes",
        b"5u8",
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
    # An element of a policy-qualified collection reads and stores under
    # that policy: a typed value keeps its own policy and must already carry
    # the collection's, while a literal lands there
    # (wiki/spec/language/numeric_values.md).
    accepted(
        program,
        "policy-qualified-collections-read-and-store",
        b"data B { counts: [u64; 4] in Trapping; }\n"
        b"machine f(b: &mut B, value: u64 in Trapping, plain: u64) -> u64 in Trapping {\n"
        b"    b.counts[0] = value;\n    b.counts[2] = plain as u64 in Trapping;\n    b.counts[3] = 7;\n"
        b"    b.counts[1] + 1\n"
        b"}",
        ((b"b.counts[0]", "u64 in Trapping"), (b"b.counts[1] + 1", "u64 in Trapping"), (b"plain as u64 in Trapping", "u64 in Trapping")),
    )
    rejected(
        program,
        "a-typed-value-keeps-its-policy-at-an-element-store",
        b"data B { counts: [u64; 4] in Trapping; }\n"
        b"machine f(b: &mut B, plain: u64) { b.counts[0] = plain; }",
        "MismatchedTypes",
        b"plain!2",
    )
    accepted(
        program,
        "wrapping-and-saturating-collections",
        b"data Holder { values: [u32; 1] in Wrapping; }\n"
        b"machine f(h: &Holder, v: &[u8] in Saturating) -> u32 in Wrapping\n"
        b"requires v.len > 0\n"
        b"{\n    let s: u8 in Saturating = v[0] + 10u8;\n    h.values[0] + 1u32\n}",
        ((b"h.values", "[u32; 1] in Wrapping"), (b"h.values[0]", "u32 in Wrapping"), (b"v[0]", "u8 in Saturating")),
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
    # A `where machine` contract types a call through its binder; a static
    # machine argument selects a machine whose signature refines the
    # contract, and a type binder the contract alone names is inferred from
    # the selected machine.
    accepted(
        program,
        "machine-binders-call-through-contracts",
        b"machine double(x: u8) -> u8 { x }\n"
        b"machine zero() -> u8 { 0 }\n"
        b"machine apply<machine F>(x: u8) -> u8\nwhere machine F(value: u8) -> u8;\n{ F(x) }\n"
        b"machine again<machine G>(x: u8) -> u8\nwhere machine G(value: u8) -> u8;\n{ apply<G>(x) }\n"
        b"machine pick<T, machine K>(x: T) -> T\nwhere machine K(value: T) -> T;\n{ K(x) }\n"
        b"machine make<U, machine M>() -> U\nwhere machine M() -> U;\n{ M() }\n"
        b"machine g() -> u8 {\n"
        b"    let a: u8 = apply<double>(3);\n"
        b"    let b: u8 = again<double>(a);\n"
        b"    let c: u8 = make<zero>();\n"
        b"    pick<double>(b)\n"
        b"}",
        (
            (b"F(x)", "u8"),
            (b"K(x)", "G2"),
            (b"M()!2", "G4"),
            (b"apply<double>(3)", "u8"),
            (b"again<double>(a)", "u8"),
            (b"make<zero>()", "u8"),
            (b"pick<double>(b)", "u8"),
        ),
        selections=(
            (b"F(x)", "binder", 0),
            (b"apply<G>(x)", "machine", 2),
            (b"K(x)", "binder", 3),
            (b"M()!2", "binder", 5),
            (b"apply<double>(3)", "machine", 2),
            (b"make<zero>()", "machine", 5),
        ),
    )
    # An inline array's extent may name a `const` binder: substitution and
    # unification carry it like an element type.
    accepted(
        program,
        "const-extents-substitute-and-unify",
        b"data Buffer<T, const N: u64> { items: [T; N]; }\n"
        b"machine take<const N: u64>(items: [u8; N]) -> [u8; N] { items }\n"
        b"machine f(b: Buffer<u16, 4>, a: [u8; 3]) -> [u8; 3] {\n"
        b"    let held: [u16; 4] = b.items;\n"
        b"    take(a)\n"
        b"}",
        ((b"items!3", "[u8; G2]"), (b"b.items", "[u16; 4]"), (b"take(a)", "[u8; 3]")),
    )
    # A declared domain qualifies its carrier: a value of the carrier meets a
    # qualified destination by a membership checking proves, a literal lands
    # there, and a predicate-only qualification disappears implicitly.
    accepted(
        program,
        "declared-domains-qualify-their-carrier",
        b"domain u64::Small(value)\nrequires value < 8;\ndomain [u8]::Text(bytes)\nrequires valid_utf8(bytes);\n"
        b"machine take(x: u64 in Small) -> u64 { x }\n"
        b"machine show(text: &[u8] in Text) {}\n"
        b"machine f(held: u64 in Small, bare: u64, raw: &[u8]) -> u64 {\n"
        b"    let a: u64 = take(held);\n"
        b"    let b: u64 = take(bare);\n"
        b"    let c: u64 = held;\n"
        b"    show(raw);\n"
        b"    take(3)\n"
        b"}",
        ((b"held!2", "u64 in Domain0"), (b"bare!2", "u64"), (b"take(3)", "u64")),
        adaptations=((b"held!2", ""), (b"bare!2", "qualified"), (b"raw!2", "qualified"), (b"3", "landed")),
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
    # Wholly anonymous arithmetic denotes an exact rational; landing needs an
    # integral value (wiki/spec/language/numeric_values.md).
    accepted(
        program,
        "anonymous-rationals-land-when-integral",
        b"machine f(x: i32) -> i32 { x * (7 / 2 * 2) }\n"
        b"machine g() -> u32 { let value: u32 = (4097 / 4096) * 4096; value }\n"
        b"machine h() -> i32 { (1 / 3 + 2 / 3) * 5 }",
        ((b"7 / 2 * 2", "i32"),),
    )
    rejected(
        program,
        "anonymous-fraction-rejects-at-landing",
        b"machine f() -> u8 { 7 / 2 + 1 }",
        "AnonymousFraction",
        b"7 / 2 + 1",
    )
    rejected(
        program,
        "anonymous-fraction-rejects-at-a-cast",
        b"machine f() -> u8 { (7 / 2) as u8 }",
        "AnonymousFraction",
        b"7 / 2) as u8",
    )
    rejected(
        program,
        "anonymous-division-by-zero-has-no-value",
        b"machine f() -> u8 { 7 / 0 * 0 }",
        "AnonymousFraction",
        b"7 / 0",
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
    # A reference target is a recast (wiki/spec/layouts/recasts.md): the
    # operand is a reference the target's access does not exceed, and both
    # referents are scalars of one width; checking judges representations.
    accepted(
        program,
        "recasts-view-scalars-of-one-width",
        b"data M { rate: f32; half: i16; flag: bool; wide: u64; }\n"
        b"machine f(m: &mut M, r: &u32) -> bool {\n"
        b"    let bits: &u32 = &m.rate as &u32;\n    let wide: &u16 = &m.half as &u16;\n"
        b"    let byte: &u8 = &m.flag as &u8;\n    let writable: &mut u32 = &mut m.rate as &mut u32;\n"
        b"    let signed: &i32 = r as &i32;\n    let narrowed: &f64 = &m.wide as &f64;\n    true\n}",
        ((b"&m.rate as &u32", "&u32"), (b"&mut m.rate as &mut u32", "&mut u32"), (b"r as &i32", "&i32"), (b"&m.wide as &f64", "&f64")),
    )
    rejected(
        program,
        "a-recast-keeps-the-width",
        b"data M { rate: f32; }\nmachine f(m: &M) { let bits: &u64 = &m.rate as &u64; }",
        "InvalidCast",
        b"&m.rate as &u64",
    )
    rejected(
        program,
        "a-recast-does-not-escalate-access",
        b"data M { rate: f32; }\nmachine f(m: &M) { let bits: &mut u32 = &m.rate as &mut u32; }",
        "InvalidCast",
        b"&m.rate as &mut u32",
    )
    rejected(
        program,
        "a-recast-keeps-its-polarity",
        b"data M { rate: f32; }\nmachine f(m: &mut M) { let bits: &u32 = &mut m.rate as &u32; }",
        "InvalidCast",
        b"&mut m.rate as &u32",
    )
    rejected(
        program,
        "a-recast-needs-a-reference",
        b"data M { rate: f32; }\nmachine f(m: &M) { let bits: &u32 = m.rate as &u32; }",
        "InvalidCast",
        b"m.rate as &u32",
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
    # A ranking view is vocabulary, not a value; `Nat::IncreasingTo`'s limit
    # is typed as an expression.
    accepted(
        program,
        "increasing-to-limit-is-typed",
        b"machine g(index: u64, limit: u64) terminates by index -> Nat::IncreasingTo(limit); {}",
        ((b"index!2", "u64"), (b"limit!2", "u64")),
    )
    rejected(
        program,
        "routed-domains-are-not-forgotten-implicitly",
        b"machine mint() -> u64;\ndomain u64::Issued\nestablished by mint;\n"
        b"machine f(token: u64 in Issued) -> u64 { token }",
        "MismatchedTypes",
        b"token!2",
    )
    rejected(
        program,
        "byte-predicates-apply-to-byte-views",
        b"domain u64::Odd(value)\nrequires valid_utf8(value);",
        "NoMatchingMachine",
        b"valid_utf8(value)",
    )
    rejected(
        program,
        "array-extents-are-static",
        b"machine g<N: u64>(items: [u8; N]) {}",
        "UnsupportedArrayLength",
        b"N!2",
    )
    rejected(
        program,
        "machine-binders-need-a-contract",
        b"machine apply<machine F>(x: u8) -> u8 { F(x) }",
        "NotCallable",
        b"F!2",
    )
    rejected(
        program,
        "binder-calls-match-their-contract",
        b"machine apply<machine F>(x: u8) -> u8\nwhere machine F(value: u8) -> u8;\n{ F(x, x) }",
        "NoMatchingMachine",
        b"F(x, x)",
    )
    rejected(
        program,
        "static-machine-arguments-refine-the-contract",
        b"machine widen(x: u8) -> u16 { x as u16 }\n"
        b"machine apply<machine F>(x: u8) -> u8\nwhere machine F(value: u8) -> u8;\n{ F(x) }\n"
        b"machine g() -> u8 { apply<widen>(3) }",
        "NoMatchingMachine",
        b"apply<widen>(3)",
    )
    rejected(
        program,
        "machine-arguments-need-machine-binders",
        b"machine double(x: u8) -> u8 { x }\nmachine id<T>(x: T) -> T { x }\nmachine g() -> u8 { id<double>(5) }",
        "NoMatchingMachine",
        b"id<double>(5)",
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

    # A value-position match (wiki/spec/language/patterns.md): its patterns
    # meet its subject, its arms' values meet one type and its arms cover the
    # subject. Arms of anonymous numbers make an anonymous choice that lands
    # arm by arm where its destination is known.
    accepted(
        program,
        "match-anonymous-arms-land-at-the-result",
        b"machine f(c: u8) -> u64 { match c { 1 -> 20, 7 -> 300, _ -> 4 } }",
        (
            (b"match c { 1 -> 20, 7 -> 300, _ -> 4 }", "u64"),
            (b"1", "u8"), (b"7", "u8"), (b"20", "u64"), (b"300", "u64"), (b"4!2", "u64"),
        ),
        adaptations=((b"1", "landed"), (b"20", "landed"), (b"4!2", "landed"), (b"match c { 1 -> 20, 7 -> 300, _ -> 4 }", "")),
    )
    accepted(
        program,
        "match-typed-arm-fixes-the-result",
        b"data K { case A; case B; }\nmachine f(k: &K, x: u8) -> u8 { match k { K::A -> x, K::B -> 9 } }",
        ((b"match k { K::A -> x, K::B -> 9 }", "u8"), (b"K::A", "D0"), (b"x!2", "u8"), (b"9", "u8")),
        adaptations=((b"9", "landed"), (b"x!2", "")),
    )
    accepted(
        program,
        "match-boolean-arms-cover",
        b"machine f(b: bool, x: u16) -> u16 { match b { true -> x, false -> 0 } }",
        ((b"match b { true -> x, false -> 0 }", "u16"), (b"true", "bool"), (b"0", "u16")),
    )
    accepted(
        program,
        "match-nested-choice-lands-through",
        b"machine f(a: u8, b: u8) -> u32 { match a { 0 -> match b { 0 -> 1 _ -> 2 } _ -> 3 } }",
        (
            (b"match a { 0 -> match b { 0 -> 1 _ -> 2 } _ -> 3 }", "u32"),
            (b"match b { 0 -> 1 _ -> 2 }", "u32"),
            (b"1", "u32"), (b"2!2", "u32"), (b"3!2", "u32"), (b"0!2", "u8"),
        ),
        adaptations=((b"1", "landed"), (b"3!2", "landed"), (b"match b { 0 -> 1 _ -> 2 }", "")),
    )
    accepted(
        program,
        "match-wildcard-has-no-pattern",
        b"machine g(v: u16) -> u16 { v }\nmachine f(c: u8, d: u16) -> u16 { match c { 0 -> d, _ -> 8 } }",
        ((b"v!2", "u16"), (b"8!2", "u16")),
    )
    accepted(
        program,
        "match-choice-lands-at-an-argument",
        b"machine g(v: u16) -> u16 { v }\nmachine f(c: u8) -> u16 { g(match c { 0 -> 5, _ -> 8 }) }",
        ((b"match c { 0 -> 5, _ -> 8 }", "u16"), (b"5", "u16"), (b"8!2", "u16")),
    )
    # An arm's `if` guard is Boolean and covers nothing
    # (wiki/spec/language/patterns.md#coverage-and-domain-tests).
    accepted(
        program,
        "guarded-arms",
        b"data T { case A; case B(v: u8); }\n"
        b"machine f(t: T, x: u8) -> u8 { transition t { T::B { v } if v == x -> s(v) _ if x > 1 -> s(x) _ -> s(1) } state s(v: u8) -> u8 { v } }",
        ((b"v == x", "bool"), (b"x > 1", "bool")),
    )
    rejected(
        program,
        "arm-guard-must-be-boolean",
        b"data T { case A; case B(v: u8); }\nmachine f(t: T) { transition t { T::B { v } if v -> s(v) _ -> s(1) } state s(v: u8) {} }",
        "NotBoolean",
        b"v!3",
    )
    rejected(
        program,
        "guarded-arms-cover-nothing",
        b"data T { case A; case B(v: u8); }\nmachine f(t: T) { transition t { T::A -> s(0) T::B { v } if v == 1 -> s(v) } state s(v: u8) {} }",
        "UncoveredDispatch",
        b"transition t { T::A -> s(0) T::B { v } if v == 1 -> s(v) }",
    )
    rejected(
        program,
        "match-literal-ladder-needs-a-wildcard",
        b"machine f(c: u8) -> u8 { match c { 1 -> 2, 3 -> 4 } }",
        "UncoveredDispatch",
        b"match c { 1 -> 2, 3 -> 4 }",
    )
    rejected(
        program,
        "match-misses-a-case",
        b"data K { case A; case B; }\nmachine f(k: K) -> u8 { match k { K::A -> 1 } }",
        "UncoveredDispatch",
        b"match k { K::A -> 1 }",
    )
    rejected(
        program,
        "match-misses-a-boolean",
        b"machine f(b: bool) -> u8 { match b { true -> 1 } }",
        "UncoveredDispatch",
        b"match b { true -> 1 }",
    )
    rejected(
        program,
        "match-arms-of-two-types",
        b"machine f(c: u8, x: u8, y: u16) -> u8 { match c { 0 -> x, _ -> y } }",
        "MismatchedTypes",
        b"y!2",
    )
    rejected(
        program,
        "match-arms-of-two-domains",
        b"domain i64::Km(value) requires value >= 0;\ndomain i64::Miles(value) requires value >= 0;\n"
        b"machine f(b: bool, near: i64 in Km, far: i64 in Miles) -> i64 in Km { match b { true -> near, false -> far } }",
        "MismatchedTypes",
        b"far!2",
    )
    rejected(
        program,
        "match-arm-outside-the-result",
        b"machine f(c: u8, x: u8) -> u8 { match c { 0 -> x, _ -> 300 } }",
        "LiteralOutOfRange",
        b"300",
    )
    rejected(
        program,
        "match-choice-arm-outside-the-destination",
        b"machine f(c: u8) -> u8 { match c { 0 -> 1, _ -> 300 } }",
        "LiteralOutOfRange",
        b"300",
    )
    rejected(
        program,
        "match-pattern-of-other-data",
        b"data K { case A; }\ndata L { case B; }\nmachine f(k: K) -> u8 { match k { L::B -> 1, _ -> 2 } }",
        "CaseOfOtherData",
        b"L::B",
    )
    rejected(
        program,
        "match-choice-is-no-subject",
        b"machine f(c: u8) -> u8 { match match c { _ -> 1 } { _ -> 2 } }",
        "NotAValue",
        b"match c { _ -> 1 }",
    )

    # A fixed-array literal joins its elements into one type, its extent its
    # element count; unlanded elements make an anonymous array that lands at
    # a destination of its shape, level by level.
    accepted(
        program,
        "array-literal-lands-at-its-local",
        b"machine f() -> u8 { let a: [u8; 3] = [1, 2, 3]; a[0] }",
        ((b"[1, 2, 3]", "[u8; 3]"), (b"1", "u8"), (b"3!2", "u8")),
        adaptations=((b"1", "landed"), (b"[1, 2, 3]", "")),
    )
    accepted(
        program,
        "array-literal-of-typed-elements",
        b"machine f(x: u16) -> u16 { let a: [u16; 2] = [x, 7]; a[1] }",
        ((b"[x, 7]", "[u16; 2]"), (b"7", "u16")),
    )
    accepted(
        program,
        "nested-array-literals-land",
        b"machine f() -> u8 { let m: [[u8; 2]; 2] = [[1, 5], [3, 4]]; m[1][0] }",
        ((b"[[1, 5], [3, 4]]", "[[u8; 2]; 2]"), (b"[1, 5]", "[u8; 2]"), (b"5", "u8"), (b"4", "u8")),
    )
    accepted(
        program,
        "array-of-matches-lands",
        b"machine f(c: u8) -> u32 { let a: [u32; 2] = [match c { 0 -> 1, _ -> 7 }, 9]; a[0] }",
        ((b"match c { 0 -> 1, _ -> 7 }", "u32"), (b"7", "u32"), (b"9", "u32")),
    )
    # An array literal's tree is its elements: names before the machine, a
    # domain's subject among them, stay out of its scope.
    accepted(
        program,
        "array-literal-tree-is-its-elements",
        b"domain u8::Small(value)\nrequires value < 4;\nmachine f() -> u8 { let a: [u8; 2] = [1, 9]; a[0] }",
        ((b"[1, 9]", "[u8; 2]"),),
    )
    accepted(
        program,
        "empty-array-literal-lands",
        b"machine f() -> u64 { let none: [u8; 0] = []; let nested: [[u8; 0]; 0] = []; 0 }",
        ((b"[]", "[u8; 0]"), (b"[]!2", "[[u8; 0]; 0]")),
    )
    rejected(
        program,
        "array-literal-extent-mismatch",
        b"machine f() -> u8 { let a: [u8; 2] = [1, 5, 7]; a[0] }",
        "MismatchedTypes",
        b"[1, 5, 7]",
    )
    rejected(
        program,
        "array-literal-element-out-of-range",
        b"machine f() -> u8 { let a: [u8; 2] = [1, 300]; a[0] }",
        "LiteralOutOfRange",
        b"300",
    )
    rejected(
        program,
        "array-literal-elements-of-two-types",
        b"machine f(x: u8, y: u16) -> u8 { let a: [u8; 2] = [x, y]; a[0] }",
        "MismatchedTypes",
        b"y!2",
    )
    rejected(
        program,
        "array-literal-mixes-arrays-and-numbers",
        b"machine f() -> u8 { let a: [u8; 2] = [[1], 2]; a[0] }",
        "MismatchedTypes",
        b"2!2",
    )

    # An assignment writes a `let mut` local, a `mut` parameter, or a place
    # reached through an exclusive reference.
    accepted(
        program,
        "assignments-to-writable-places",
        b"data P { x: u8; }\nmachine f(mut count: u8, p: &mut P, a: &mut [u8; 2]) -> u8 {\n"
        b"    let mut local: P = P { x: 1 };\n    count = 2;\n    local.x = 3;\n    p.x = 4;\n    a[0] = 5;\n    count\n}",
        ((b"count!3", "u8"),),
    )
    rejected(
        program,
        "assignment-to-immutable-local",
        b"machine f() -> u8 { let x: u8 = 1; x = 2; x }",
        "ImmutablePlace",
        b"x!2",
    )
    rejected(
        program,
        "assignment-to-immutable-parameter",
        b"machine f(input: u8) -> u8 { input = 2; input }",
        "ImmutablePlace",
        b"input!2",
    )
    rejected(
        program,
        "assignment-through-shared-reference",
        b"data P { x: u8; }\nmachine f(p: &P) -> u8 { p.x = 2; 0 }",
        "ImmutablePlace",
        b"p.x",
    )

    # `&write place` lends exclusive mutation without observation
    # (wiki/spec/language/ownership.md#borrows-and-aliases): it forms from a
    # place an assignment could write, a `&write` destination is met only by
    # a `&write` borrow (attenuation and forwarding are explicit), and a
    # write-only reference reads as nothing.
    accepted(
        program,
        "write-only-borrows-form-from-writable-places",
        b"data P { x: u8; }\n"
        b"machine replace(d: &write u8, v: u8) { d = v; }\n"
        b"machine f(mut count: u8, p: &mut P, w: &write P) -> u8 {\n"
        b"    let mut local: P = P { x: 1 };\n"
        b"    replace(&write count, 2);\n    replace(&write local.x, 3);\n    replace(&write p.x, 4);\n"
        b"    replace(&write w.x, 5);\n    w.x = 6;\n    let whole: &write P = &write local;\n    count\n}",
        ((b"&write count", "&write u8"), (b"&write p.x", "&write u8"), (b"&write w.x", "&write u8"), (b"&write local!2", "&write D0")),
    )
    rejected(
        program,
        "write-only-borrow-through-shared-reference",
        b"data P { x: u8; }\nmachine replace(d: &write u8, v: u8) { d = v; }\nmachine f(p: &P) { replace(&write p.x, 1); }",
        "ImmutablePlace",
        b"p.x",
    )
    rejected(
        program,
        "write-only-borrow-of-immutable-local",
        b"machine replace(d: &write u8, v: u8) { d = v; }\nmachine f() { let x: u8 = 0; replace(&write x, 1); }",
        "ImmutablePlace",
        b"x!2",
    )
    rejected(
        program,
        "a-write-only-destination-needs-a-write-only-borrow",
        b"machine replace(d: &write u8, v: u8) { d = v; }\nmachine f(m: &mut u8) { replace(&mut m, 1); }",
        "ImplicitAttenuation",
        b"&mut m",
    )
    rejected(
        program,
        "a-write-only-reference-forwards-only-by-borrow",
        b"machine replace(d: &write u8, v: u8) { d = v; }\nmachine f(sink: &write u8) { replace(sink, 1); }",
        "ImplicitAttenuation",
        b"sink!2",
    )
    rejected(
        program,
        "a-write-only-reference-does-not-read",
        b"machine f(sink: &write u8) -> u8 { sink }",
        "MismatchedTypes",
        b"sink!2",
    )
    # A rank range's endpoints are integers (wiki/spec/language/termination.md#ranking).
    accepted(
        program,
        "rank-range-endpoints-are-integers",
        b"machine f(n: u64, limit: u64) -> u64 terminates by n in 0..=limit; { 0 }",
        ((b"0", "u64"), (b"limit!2", "u64")),
    )
    rejected(
        program,
        "a-rank-range-endpoint-of-another-type",
        b"machine f(n: u64, flag: bool) -> u64 terminates by n in 0..=flag; { 0 }",
        "NonIntegerIndex",
        b"flag!2",
    )
    # A declared measure's subject meets the measure's parameter, or is the
    # data a lexicographic measure's owner names
    # (wiki/spec/language/termination.md#ranking).
    accepted(
        program,
        "measure-subjects-meet-their-measures",
        b"data Card { power: u64; }\ndata Progress { outer: u64; inner: u64; }\n"
        b"measure Card::PowerOrder(card: Card) -> u64 { card.power }\n"
        b"measure Progress::Steps lexicographic { outer, inner }\n"
        b"machine f(card: Card, progress: Progress) -> u64\n"
        b"terminates by card -> Card::PowerOrder;\nterminates by progress -> Progress::Steps;\n{ 0 }",
        ((b"card.power", "u64"),),
    )
    rejected(
        program,
        "a-measure-subject-of-another-type",
        b"data Card { power: u64; }\nmeasure Card::PowerOrder(card: Card) -> u64 { card.power }\n"
        b"machine f(value: u64) -> u64 terminates by value -> Card::PowerOrder; { 0 }",
        "MismatchedTypes",
        b"value!2",
    )
    rejected(
        program,
        "a-lexicographic-subject-of-another-data",
        b"data Progress { outer: u64; inner: u64; }\ndata Other { outer: u64; inner: u64; }\n"
        b"measure Progress::Steps lexicographic { outer, inner }\n"
        b"machine f(other: Other) -> u64 terminates by other -> Progress::Steps; { 0 }",
        "MismatchedTypes",
        b"other!2",
    )
    rejected(
        program,
        "a-write-only-reference-is-not-shared",
        b"machine take(v: &u8) {}\nmachine f(sink: &write u8) { take(sink); }",
        "NoMatchingMachine",
        b"take(sink)",
    )

    # A value pattern, a literal or a path or name denoting no case, meets
    # the subject's type and selects by equality, so that type compares
    # intrinsically; it covers nothing.
    accepted(
        program,
        "match-constant-pattern",
        b"const LIMIT: u8 = 9;\nmachine f(c: u8) -> u8 { match c { LIMIT -> 1, _ -> 2 } }",
        ((b"LIMIT!2", "u8"),),
    )
    accepted(
        program,
        "match-parameter-pattern",
        b"machine f(c: u8, d: u8) -> u8 { match c { d -> 1, _ -> 2 } }",
        ((b"d!2", "u8"),),
    )
    accepted(
        program,
        "transition-constant-path-pattern",
        b"data Size { value: u8; }\nconst Size::MAX: u8 = 9;\n"
        b"machine f(c: u8) { transition c { Size::MAX -> g() _ -> g() } state g() {} }",
        ((b"Size::MAX!2", "u8"),),
    )
    rejected(
        program,
        "value-pattern-needs-intrinsic-equality",
        b"data P { x: u8; }\nconst P::ZERO: P = P { x: 0 };\nmachine f(p: P) -> u8 { match p { P::ZERO -> 1, _ -> 2 } }",
        "UnsupportedEquality",
        b"P::ZERO!2",
    )
    rejected(
        program,
        "value-pattern-is-not-coverage",
        b"const LIMIT: u8 = 9;\nmachine f(c: u8) -> u8 { match c { LIMIT -> 1 } }",
        "UncoveredDispatch",
        b"match c { LIMIT -> 1 }",
    )

    # A transition's arms cover its subject, as a match's do; without one,
    # complementary guards (`x == true` and `x == false`, `l == r` and
    # `l != r`) or a `true` guard close the dispatch.
    accepted(
        program,
        "transition-cases-cover",
        b"data K { case A; case B; }\nmachine f(k: K) { transition k { K::A -> g() K::B -> g() } state g() {} }",
        (),
    )
    accepted(
        program,
        "transition-booleans-cover",
        b"machine f(b: bool) { transition b { true -> g() false -> g() } state g() {} }",
        (),
    )
    accepted(
        program,
        "transition-complementary-equalities-cover",
        b"machine f(x: u8, y: u8) { transition { x == y -> g() x != y -> g() } state g() {} }",
        (),
    )
    accepted(
        program,
        "transition-opposed-boolean-comparisons-cover",
        b"machine f(a: u8) { transition { (a < 3) == true -> g() false == (a < 3) -> g() } state g() {} }",
        (),
    )
    accepted(
        program,
        "transition-true-guard-closes",
        b"machine f(x: u8) { transition { x < 3 -> g() true -> g() } state g() {} }",
        (),
    )
    rejected(
        program,
        "transition-literal-ladder-needs-a-wildcard",
        b"machine f(c: u8) { transition c { 1 -> g() 2 -> g() } state g() {} }",
        "UncoveredDispatch",
        b"transition c { 1 -> g() 2 -> g() }",
    )
    rejected(
        program,
        "transition-misses-a-case",
        b"data K { case A; case B; }\nmachine f(k: K) { transition k { K::A -> g() } state g() {} }",
        "UncoveredDispatch",
        b"transition k { K::A -> g() }",
    )
    rejected(
        program,
        "transition-order-ladder-needs-a-wildcard",
        b"machine f(x: u8) { transition { x < 3 -> g() x >= 3 -> g() } state g() {} }",
        "UncoveredDispatch",
        b"transition { x < 3 -> g() x >= 3 -> g() }",
    )
    # The rejection spells `!=`, which `rejected` reads as an occurrence.
    source = b"machine f(x: u8, y: u8) { transition { x == y -> g() x != 1 -> g() } state g() {} }"
    status, payload = run(program, source)
    observation = decode(payload)
    expected = ("UncoveredDispatch", span_of(source, b"transition { x == y -> g() x != 1 -> g() }"))
    assert status == 249 and (observation.diagnostic, observation.diagnostic_span) == expected, (
        f"transition-equalities-of-other-sides-do-not-cover: {status} {observation.diagnostic} at {observation.diagnostic_span}"
    )

    # A membership is a Boolean: a case of the value's data, or a declared
    # domain of its carrier. A renamed pattern field types as its field.
    accepted(
        program,
        "case-membership",
        b"data O { case None; case Some(value: u8); }\nmachine f(o: &O) -> bool { o in O::None }",
        ((b"o in O::None", "bool"),),
    )
    accepted(
        program,
        "domain-membership-fact",
        b"domain u64::Small(value)\nrequires value < 4;\nmachine f(x: u64) -> u64\nrequires x in Small\n{ x }",
        ((b"x in Small", "bool"),),
    )
    rejected(
        program,
        "membership-of-another-carrier",
        b"domain u64::Small(value)\nrequires value < 4;\nmachine f(x: u8) -> bool\nrequires x in Small\n{ true }",
        "MismatchedTypes",
        b"x!2",
    )
    rejected(
        program,
        "case-membership-of-other-data",
        b"data K { case A; }\ndata L { case B; }\nmachine f(k: K) -> bool { k in L::B }",
        "CaseOfOtherData",
        b"k!2",
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
    accepted(
        program,
        "declared-domain",
        b"domain u64::Small(value) requires value < 4;\nmachine f(x: u64 in Small) -> u64 in Small { x }",
        ((b"x!2", "u64 in Domain0"),),
    )
    accepted(
        program,
        "path-qualified-declared-domain",
        b"domain u64::Small(value) requires value < 4;\nmachine f(x: u64 in u64::Small) -> u64 in Small { x }",
        ((b"x!2", "u64 in Domain0"),),
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
        "ImmutablePlace",
        b"n!3",
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

    # A range slice is the slice place of its collection's elements under
    # the collection's qualifier: a shared lending or a borrow makes it a
    # view. Its endpoints are indexes; the range itself is no value.
    accepted(
        program,
        "range-slices-are-slice-places",
        b"machine see(v: &[u8]) {}\nmachine fill(v: &mut [u8]) {}\n"
        b"machine f(a: &[u8; 8], b: &mut [u8; 8], c: &[u16], i: u32) {\n"
        b"    see(a[1..3]);\n    fill(&mut b[..2]);\n    let d: &[u16] = &c[i..];\n    see(a[..=5]);\n}",
        ((b"a[1..3]", "[u8]"), (b"b[..2]", "[u8]"), (b"&mut b[..2]", "&mut [u8]"), (b"c[i..]", "[u16]"),
         (b"1..3", "()"), (b"3!2", "u64"), (b"a[..=5]", "[u8]")),
        adaptations=((b"a[1..3]", "lent"),),
    )
    # A range slice is a view wherever a shared-reference destination
    # stands, not only at a call's argument; no other place lends implicitly.
    accepted(
        program,
        "range-slice-views-at-any-shared-destination",
        b"data Sample { bytes: &[u8]; }\n"
        b"machine f(a: &[u8; 8]) -> u64 {\n"
        b"    let view: &[u8] = a[1..3];\n    let sample: Sample = Sample { bytes: a[..2] };\n    view.len\n}",
        ((b"a[1..3]", "[u8]"), (b"a[..2]", "[u8]")),
        adaptations=((b"a[1..3]", "lent"), (b"a[..2]", "lent")),
    )
    rejected(
        program,
        "only-a-range-slice-lends-implicitly",
        b"machine f(owned: [u8; 8]) -> u64 { let view: &[u8; 8] = owned; 0 }",
        "MismatchedTypes",
        b"owned!2",
    )
    # A quoted literal copies its bytes into an unqualified fixed byte array
    # of exactly its length; each escape is one byte.
    accepted(
        program,
        "quoted-literal-copies-into-a-byte-array",
        b"data Record { bytes: [u8; 3]; }\n"
        b"machine f() -> u8 {\n"
        b"    let none: [u8; 0] = \"\";\n    let escaped: [u8; 2] = \"\\x41\\n\";\n"
        b"    let record: Record = Record { bytes: \"abc\" };\n    escaped[0]\n}",
        ((b"\"\"", "[u8; 0]"), (b"\"\\x41\\n\"", "[u8; 2]"), (b"\"abc\"", "[u8; 3]")),
        adaptations=((b"\"abc\"", "landed"),),
    )
    rejected(
        program,
        "quoted-literal-length-must-match",
        b"machine f() -> u8 { let bytes: [u8; 2] = \"abc\"; bytes[0] }",
        "MismatchedTypes",
        b"\"abc\"",
    )
    # A reference to a builtin scalar reads its referent wherever the scalar
    # is needed: an admitted value, an operand, a cast or an index.
    accepted(
        program,
        "reference-reads-its-scalar",
        b"machine read(value: &u8) -> u8 { value }\n"
        b"machine bump(count: &mut u64) { count = ((count as u64 in Wrapping) + 1) as u64; }\n"
        b"machine above(count: &u64) -> bool { count > 3 }\n"
        b"machine flip(flag: &bool) -> bool { !flag }\n"
        b"machine pick(items: &[u8; 4], at: &u64) -> u8 { items[at] }",
        ((b"value!2", "&u8"), (b"count as u64 in Wrapping", "u64 in Wrapping"), (b"count > 3", "bool"),
         (b"items[at]", "u8")),
        adaptations=((b"value!2", "read"),),
    )
    rejected(
        program,
        "only-a-scalar-reads-through-a-reference",
        b"data Token { x: u64; }\nmachine f(token: &Token) -> Token { token }",
        "MismatchedTypes",
        b"token!2",
    )
    # A reference to a whole array views it as a slice of its elements. A
    # mutable subrange is written `&mut items[start..end]`; an expected
    # `&mut [T]` never inserts the borrow
    # (wiki/spec/language/expressions.md#borrowed-range-access).
    accepted(
        program,
        "array-references-view-as-slices",
        b"machine see(v: &[u8]) {}\nmachine fill(v: &mut [u8]) {}\n"
        b"machine f(a: [u8; 4], b: &mut [u8; 4]) {\n"
        b"    see(&a);\n    fill(b);\n    see(b);\n    let held: &mut [u8] = &mut b[1..3];\n    fill(held);\n}",
        ((b"&a", "&[u8; 4]"), (b"b[1..3]", "[u8]"), (b"&mut b[1..3]", "&mut [u8]")),
    )
    rejected(
        program,
        "a-bare-range-never-lends-exclusively",
        b"machine f(b: &mut [u8; 4]) { let held: &mut [u8] = b[1..3]; }",
        "MismatchedTypes",
        b"b[1..3]",
    )
    rejected(
        program,
        "a-bare-range-argument-never-lends-exclusively",
        b"machine fill(v: &mut [u8]) {}\nmachine f(b: &mut [u8; 4]) { fill(b[1..3]); }",
        "NoMatchingMachine",
        b"fill(b[1..3])",
    )
    rejected(
        program,
        "an-exclusive-slice-needs-an-exclusive-reference",
        b"machine f(a: &[u8; 4]) { let held: &mut [u8] = &mut a[1..3]; }",
        "ImmutablePlace",
        b"a[1..3]",
    )
    rejected(
        program,
        "a-shared-field-is-not-lent-exclusively",
        b"data H { items: [u8; 4]; }\nmachine fill(v: &mut [u8]) {}\nmachine f(h: &H) { fill(&mut h.items[1..3]); }",
        "ImmutablePlace",
        b"h.items[1..3]",
    )
    rejected(
        program,
        "range-endpoint-is-an-index",
        b"machine see(v: &[u8]) {}\nmachine f(a: &[u8; 8]) { see(a[true..]); }",
        "NonIntegerIndex",
        b"true",
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
