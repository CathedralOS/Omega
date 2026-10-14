#!/usr/bin/env python3
"""Black-box checks for the live Omega-written Psi checking slice."""

from __future__ import annotations

import struct
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

# Tags are declaration ordinals, mirrored from source/psi/checks/checks.omg.
DIAGNOSTICS = (
    "None", "CheckCapacityExceeded", "UnsupportedFact", "UnsupportedCheck",
    "IndexOutOfBounds", "ArithmeticOverflow", "DivisorMayBeZero",
    "ConversionOutOfRange", "ShiftCountOutOfRange", "RequirementUnproven",
    "DomainUnproven", "CrashNotDeclared", "GuaranteeUnproven", "RankUnproven",
    "ProgressUnproven", "UseAfterMove", "MoveOutOfBorrow", "LinearUnconsumed",
    "LinearOverwritten", "OverlappingArguments", "LoanConflict", "LoanEscapes",
    "RepresentationUnproven", "WriteOnlyObservation", "RankRangeUnproven",
)


@dataclass(frozen=True)
class Routes:
    declares_trap: bool
    declares_abort: bool
    inferred: bool
    trap: bool
    abort: bool


@dataclass(frozen=True)
class Observation:
    accepted: bool
    diagnostic: str
    diagnostic_span: tuple[int, int, int]
    routes: tuple[Routes, ...]
    # Every rejection in the order checking found it; `diagnostic` is the
    # first, or None.
    rejections: tuple[tuple[str, tuple[int, int, int]], ...] = ()


class Reader:
    def __init__(self, payload: bytes) -> None:
        self.payload = payload
        self.cursor = 0

    def bytes(self, count: int) -> bytes:
        end = self.cursor + count
        if end > len(self.payload):
            raise AssertionError("truncated checking observation")
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
    assert reader.bytes(8) == b"OMGCHK1\0"
    assert reader.u64() == 2
    accepted = reader.byte() == 1
    rejections = tuple((DIAGNOSTICS[reader.byte()], reader.span()) for _ in range(reader.u64()))
    diagnostic, diagnostic_span = rejections[0] if rejections else ("None", (0, 0, 0))
    routes = tuple(
        Routes(*(reader.byte() == 1 for _ in range(5)))
        for _ in range(reader.u64())
    )
    assert reader.cursor == len(payload), "trailing checking observation bytes"
    assert accepted == (not rejections), "acceptance disagrees with the rejections"
    return Observation(accepted, diagnostic, diagnostic_span, routes, rejections)


# Every distinct input the gate runs is one case; the determinism check runs
# its input twice and counts once.
CASE_SOURCES: set[bytes] = set()


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


def located(source: bytes, spelling: bytes) -> tuple[int, int, int]:
    """`spelling` or `spelling!n` (its n-th occurrence) in the root unit."""
    occurrence = 1
    if b"!" in spelling:
        spelling, count = spelling.rsplit(b"!", 1)
        occurrence = int(count)
    start = -1
    for _ in range(occurrence):
        start = source.index(spelling, start + 1)
    return (0, start, start + len(spelling))


def accepted(program: str, name: str, source: bytes, routes: dict[int, str] | None = None) -> Observation:
    """`routes` maps a machine handle to its expected crash routes, spelled
    as the declared causes, `/`, then the derived ones: `T/T` declares and
    derives Trap, `?/T` infers Trap, `/` neither."""
    status, payload = run(program, source)
    observation = decode(payload)
    assert status == 0 and observation.accepted, (
        f"{name}: status {status}, {observation.diagnostic} at {observation.diagnostic_span}"
    )
    for machine, expected in (routes or {}).items():
        actual = spelled(observation.routes[machine])
        assert actual == expected, f"{name}: machine {machine} routes {actual}, expected {expected}"
    return observation


def spelled(routes: Routes) -> str:
    declared = "?" if routes.inferred else ("T" if routes.declares_trap else "") + ("A" if routes.declares_abort else "")
    derived = ("T" if routes.trap else "") + ("A" if routes.abort else "")
    return f"{declared}/{derived}"


def rejected(program: str, name: str, source: bytes, diagnostic: str, spelling: bytes) -> None:
    status, payload = run(program, source)
    assert status == 247, f"{name}: status {status}"
    observation = decode(payload)
    assert not observation.accepted, f"{name}: accepted"
    assert observation.diagnostic == diagnostic, f"{name}: {observation.diagnostic}, expected {diagnostic}"
    expected = located(source, spelling)
    assert observation.diagnostic_span == expected, (
        f"{name}: span {observation.diagnostic_span}, expected {expected}"
    )


def phase_rejected(program: str, name: str, source: bytes, status: int) -> None:
    actual, payload = run(program, source)
    assert actual == status and payload == b"", f"{name}: status {actual}, {len(payload)} bytes"


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: test_checking.py <checking-harness-executable>")
    program = sys.argv[1]

    # Indexing: a guard's fact reaches the state it selects.
    accepted(
        program,
        "guard-reaches-the-selected-state",
        b"machine read(a: &[u8; 4], i: u64) -> u8 {\n"
        b"    transition i < 4 {\n"
        b"        true -> found(a, i)\n"
        b"        _ -> missing()\n"
        b"    }\n"
        b"\n"
        b"    state found(a: &[u8; 4], i: u64) -> u8 { a[i] }\n"
        b"    state missing() -> u8 { 0 }\n"
        b"}",
        {0: "?/"},
    )
    rejected(
        program,
        "unguarded-index",
        b"machine read(a: &[u8; 4], i: u64) -> u8 { a[i] }",
        "IndexOutOfBounds",
        b"a[i]",
    )
    rejected(
        program,
        "passed-over-guard-bounds-nothing",
        b"machine read(a: &[u8; 4], i: u64) -> u8 {\n"
        b"    transition i < 4 {\n"
        b"        true -> missing()\n"
        b"        _ -> found(a, i)\n"
        b"    }\n"
        b"\n"
        b"    state found(a: &[u8; 4], i: u64) -> u8 { a[i] }\n"
        b"    state missing() -> u8 { 0 }\n"
        b"}",
        "IndexOutOfBounds",
        b"a[i]",
    )
    # A slice's length is a subject of its place.
    accepted(
        program,
        "slice-length-bounds-an-index",
        b"machine first(a: &[u8], i: u64) -> u8 {\n"
        b"    transition i < a.len {\n"
        b"        true -> found(a, i)\n"
        b"        _ -> missing()\n"
        b"    }\n"
        b"\n"
        b"    state found(a: &[u8], i: u64) -> u8 { a[i] }\n"
        b"    state missing() -> u8 { 0 }\n"
        b"}",
    )

    # A range slice stays within its collection: its start at most its
    # exclusive end, that end at most the length; an inclusive end is a
    # valid index.
    accepted(
        program,
        "range-slices-within-a-guarded-length",
        b"machine see(v: &[u8]) {}\n"
        b"machine f(a: &[u8], i: u64) {\n"
        b"    transition i < a.len {\n"
        b"        true -> sliced(a, i)\n"
        b"        _ -> {}\n"
        b"    }\n"
        b"\n"
        b"    state sliced(a: &[u8], i: u64) { see(a[i..]); see(a[..i]); see(a[..=i]); see(a[0..i]); see(a[..]); }\n"
        b"}",
    )
    accepted(
        program,
        "range-slices-of-a-fixed-array",
        b"machine see(v: &[u8]) {}\nmachine f(a: &[u8; 4]) { see(a[1..4]); see(a[..=3]); see(a[4..]); see(a[2..=1]); }",
    )
    rejected(
        program,
        "range-end-past-the-length",
        b"machine see(v: &[u8]) {}\nmachine f(a: &[u8; 4]) { see(a[1..5]); }",
        "IndexOutOfBounds",
        b"a[1..5]",
    )
    rejected(
        program,
        "inclusive-range-end-at-the-length",
        b"machine see(v: &[u8]) {}\nmachine f(a: &[u8; 4]) { see(a[..=4]); }",
        "IndexOutOfBounds",
        b"a[..=4]",
    )
    rejected(
        program,
        "range-start-past-its-end",
        b"machine see(v: &[u8]) {}\nmachine f(a: &[u8; 4], i: u64) { see(a[i..2]); }",
        "IndexOutOfBounds",
        b"a[i..2]",
    )

    # Short-circuit operands see their left operand's outcome.
    accepted(
        program,
        "conjunction-guards-its-right-operand",
        b"machine zero_at(a: &[u8; 4], i: u64) -> bool { i < 4 && a[i] == 0 }",
    )
    rejected(
        program,
        "disjunction-does-not-guard-on-success",
        b"machine zero_at(a: &[u8; 4], i: u64) -> bool { i < 4 || a[i] == 0 }",
        "IndexOutOfBounds",
        b"a[i]",
    )
    accepted(
        program,
        "disjunction-guards-on-failure",
        b"machine zero_at(a: &[u8; 4], i: u64) -> bool { i >= 4 || a[i] == 0 }",
    )

    # Exact arithmetic must fit its carrier.
    rejected(
        program,
        "exact-sum-may-overflow",
        b"machine next(x: u64) -> u64 { x + 1 }",
        "ArithmeticOverflow",
        b"x + 1",
    )
    accepted(
        program,
        "exact-sum-under-a-guard",
        b"machine next(x: u64) -> u64 {\n"
        b"    transition x < 100 {\n"
        b"        true -> more(x)\n"
        b"        _ -> same(x)\n"
        b"    }\n"
        b"\n"
        b"    state more(x: u64) -> u64 { x + 1 }\n"
        b"    state same(x: u64) -> u64 { x }\n"
        b"}",
    )
    accepted(
        program,
        "narrow-operands-fit-a-wide-sum",
        b"machine sum(a: u8, b: u8) -> u16 { (a as u16) + (b as u16) }",
    )
    rejected(
        program,
        "difference-may-underflow",
        b"machine gap(i: u64, n: u64) -> u64 { n - i }",
        "ArithmeticOverflow",
        b"n - i",
    )
    accepted(
        program,
        "difference-under-an-order",
        b"machine gap(i: u64, n: u64) -> u64 {\n"
        b"    transition i <= n {\n"
        b"        true -> difference(i, n)\n"
        b"        _ -> none()\n"
        b"    }\n"
        b"\n"
        b"    state difference(i: u64, n: u64) -> u64 { n - i }\n"
        b"    state none() -> u64 { 0 }\n"
        b"}",
    )
    accepted(
        program,
        "local-carries-its-initializer",
        b"machine twice(x: u8) -> u16 {\n"
        b"    let wide: u16 = x as u16;\n"
        b"    let doubled: u16 = wide * 2;\n"
        b"    doubled + 1\n"
        b"}",
    )

    accepted(
        program,
        "float-arithmetic-owes-no-integer-range",
        b"machine mix(x: f64, y: f64) -> f64 { -(x * y + 1.5) }",
    )

    # Divisors are nonzero; conversions fit their target unless its policy
    # says otherwise.
    rejected(
        program,
        "divisor-may-be-zero",
        b"machine half(x: u64, d: u64) -> u64 { x / d }",
        "DivisorMayBeZero",
        b"x / d",
    )
    accepted(
        program,
        "guarded-divisor",
        b"machine half(x: u64, d: u64) -> u64 {\n"
        b"    transition d > 0 {\n"
        b"        true -> divided(x, d)\n"
        b"        _ -> none()\n"
        b"    }\n"
        b"\n"
        b"    state divided(x: u64, d: u64) -> u64 { x / d }\n"
        b"    state none() -> u64 { 0 }\n"
        b"}",
    )
    rejected(
        program,
        "narrowing-may-not-fit",
        b"machine low(x: u64) -> u8 { x as u8 }",
        "ConversionOutOfRange",
        b"x as u8",
    )
    rejected(
        program,
        "a-policy-target-does-not-change-a-conversion",
        b"machine low(x: u64) -> u8 in Wrapping { x as u8 in Wrapping }",
        "ConversionOutOfRange",
        b"x as u8 in Wrapping",
    )
    accepted(
        program,
        "narrowing-a-proven-value",
        b"machine low(x: u64) -> u8 { (x % 256) as u8 }",
    )

    # A parameter enters with both ends of its carrier's range, and a
    # terminal value may be the set's first expression.
    accepted(
        program,
        "parameter-widens-within-its-range",
        b"machine f(a: u8) -> u16 { a as u16 }",
    )
    accepted(
        program,
        "terminal-value-as-the-first-expression",
        b"machine f() -> u8 { transition { _ -> (1) } }",
    )
    # A store whose value reads the overwritten place keeps only the value's
    # range: it never relates the place to itself.
    rejected(
        program,
        "self-increment-keeps-its-bound",
        b"machine f(a: &[u8; 4], i: u64) -> u8 {\n"
        b"    transition i < 3 {\n"
        b"        true -> g(a, i)\n"
        b"        _ -> (0)\n"
        b"    }\n"
        b"\n"
        b"    state g(a: &[u8; 4], i: u64) -> u8 {\n"
        b"        let mut j: u64 = i;\n"
        b"        j = j + 1;\n"
        b"        a[j + 1]\n"
        b"    }\n"
        b"}",
        "IndexOutOfBounds",
        b"a[j + 1]",
    )

    # A failed equality excludes one value: beside a proven order an
    # unsigned divisor known unequal to zero is at least one.
    accepted(
        program,
        "unequal-divisor-from-a-passed-literal-arm",
        b"machine f(a: u64, b: u64) -> u64 { transition b == 0 { true -> (0) _ -> (a / b) } }",
    )
    accepted(
        program,
        "unequal-divisor-from-a-failed-disjunction",
        b"machine f(a: u64, b: u64) -> u64 { transition { a == 0 || b == 0 -> (0) _ -> (a / b) } }",
    )

    accepted(
        program,
        "unequal-field-divisor-from-a-failed-disjunction",
        b"data E [copy] { negative: bool; magnitude: u64 in Wrapping; }\n"
        b"machine m(left: E, right: E) -> u64 in Wrapping {\n"
        b"    transition left.magnitude == 0 || right.magnitude == 0 {\n"
        b"        true -> (0)\n"
        b"        _ -> (left.magnitude / right.magnitude)\n"
        b"    }\n"
        b"}",
    )

    # A loop's facts settle over every predecessor of its head.
    accepted(
        program,
        "loop-index-stays-in-bounds",
        b"machine sum(a: &[u8; 8]) -> u64 in Wrapping {\n"
        b"    transition { _ -> step(a, 0, 0) }\n"
        b"\n"
        b"    state step(a: &[u8; 8], i: u64, total: u64 in Wrapping) -> u64 in Wrapping {\n"
        b"        transition i < 8 {\n"
        b"            true -> add(a, i, total)\n"
        b"            _ -> done(total)\n"
        b"        }\n"
        b"    }\n"
        b"\n"
        b"    state add(a: &[u8; 8], i: u64, total: u64 in Wrapping) -> u64 in Wrapping {\n"
        b"        let element: u64 in Wrapping = a[i] as u64 in Wrapping;\n"
        b"        transition { _ -> step(a, i + 1, total + element) }\n"
        b"    }\n"
        b"\n"
        b"    state done(total: u64 in Wrapping) -> u64 in Wrapping { total }\n"
        b"}",
    )
    rejected(
        program,
        "loop-counter-past-the-guard",
        b"machine fill(a: &mut [u8; 8]) {\n"
        b"    transition { _ -> step(a, 0) }\n"
        b"\n"
        b"    state step(a: &mut [u8; 8], i: u64) {\n"
        b"        transition i <= 8 {\n"
        b"            true -> store(a, i)\n"
        b"            _ -> {}\n"
        b"        }\n"
        b"    }\n"
        b"\n"
        b"    state store(a: &mut [u8; 8], i: u64) {\n"
        b"        a[i] = 0;\n"
        b"        transition { _ -> step(a, i + 1) }\n"
        b"    }\n"
        b"}",
        "IndexOutOfBounds",
        b"a[i]",
    )
    # A widened arrival may drop a bound only a carrier implied; the state's
    # places keep their carrier ranges all the same.
    accepted(
        program,
        "widened-field-keeps-its-carrier-range",
        b"data U { length: u64; }\n"
        b"data L { source: U; cursor: u64 in Trapping; index: u64 in Trapping; depth: u64 in Trapping; }\n"
        b"machine m(l: &mut L) -> u64 in Trapping {\n"
        b"    l.depth = 1;\n"
        b"    transition { _ -> scan(l) }\n"
        b"\n"
        b"    state scan(l: &mut L) -> u64 in Trapping {\n"
        b"        transition l.cursor < l.source.length {\n"
        b"            true -> pair(l)\n"
        b"            _ -> done(l)\n"
        b"        }\n"
        b"    }\n"
        b"\n"
        b"    state pair(l: &mut L) -> u64 in Trapping {\n"
        b"        l.index = l.cursor + 1;\n"
        b"        transition l.index < l.source.length {\n"
        b"            true -> open(l)\n"
        b"            _ -> advance(l)\n"
        b"        }\n"
        b"    }\n"
        b"\n"
        b"    state open(l: &mut L) -> u64 in Trapping {\n"
        b"        l.depth = l.depth + 1;\n"
        b"        l.cursor = l.cursor + 2;\n"
        b"        transition { _ -> scan(l) }\n"
        b"    }\n"
        b"\n"
        b"    state advance(l: &mut L) -> u64 in Trapping {\n"
        b"        l.cursor = l.cursor + 1;\n"
        b"        transition { _ -> scan(l) }\n"
        b"    }\n"
        b"\n"
        b"    state done(l: &mut L) -> u64 in Trapping {\n"
        b"        l.source.length as u64 in Trapping\n"
        b"    }\n"
        b"}",
    )

    # A state's arrival carries the paths it, its successors and its
    # callees' `requires` facts read, not the first 16 a wide value's jump
    # happened to touch.
    accepted(
        program,
        "guard-survives-a-wide-arrival",
        b"data P { f0: u64; f1: u64; f2: u64; f3: u64; f4: u64; f5: u64; f6: u64; f7: u64; f8: u64; f9: u64; f10: u64; f11: u64; f12: u64; f13: u64; f14: u64; f15: u64; f16: u64; f17: u64; f18: u64; f19: u64; count: u64; items: [u8; 16] in Trapping; }\n"
        b"machine use20(a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, a6: u64, a7: u64, a8: u64, a9: u64, a10: u64, a11: u64, a12: u64, a13: u64, a14: u64, a15: u64, a16: u64, a17: u64, a18: u64, a19: u64) {}\n"
        b"machine m(p: &mut P) {\n"
        b"    use20(p.f0, p.f1, p.f2, p.f3, p.f4, p.f5, p.f6, p.f7, p.f8, p.f9, p.f10, p.f11, p.f12, p.f13, p.f14, p.f15, p.f16, p.f17, p.f18, p.f19);\n"
        b"    transition { _ -> root(p) }\n"
        b"\n"
        b"    state root(p: &mut P) {\n"
        b"        transition p.count < 16 {\n"
        b"            true -> bounded(p)\n"
        b"            _ -> {}\n"
        b"        }\n"
        b"    }\n"
        b"\n"
        b"    state bounded(p: &mut P) {\n"
        b"        p.items[p.count] = 1;\n"
        b"    }\n"
        b"}",
    )
    accepted(
        program,
        "guard-survives-a-wide-state",
        b"data P { f0: u64; f1: u64; f2: u64; f3: u64; f4: u64; f5: u64; f6: u64; f7: u64; f8: u64; f9: u64; f10: u64; f11: u64; f12: u64; f13: u64; f14: u64; f15: u64; f16: u64; f17: u64; f18: u64; f19: u64; count: u64; items: [u8; 16] in Trapping; }\n"
        b"machine use20(a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, a6: u64, a7: u64, a8: u64, a9: u64, a10: u64, a11: u64, a12: u64, a13: u64, a14: u64, a15: u64, a16: u64, a17: u64, a18: u64, a19: u64) {}\n"
        b"machine m(p: &mut P) {\n"
        b"    transition p.count < 16 {\n"
        b"        true -> store(p)\n"
        b"        _ -> {}\n"
        b"    }\n"
        b"\n"
        b"    state store(p: &mut P) {\n"
        b"        use20(p.f0, p.f1, p.f2, p.f3, p.f4, p.f5, p.f6, p.f7, p.f8, p.f9, p.f10, p.f11, p.f12, p.f13, p.f14, p.f15, p.f16, p.f17, p.f18, p.f19);\n"
        b"        p.items[p.count] = 1;\n"
        b"    }\n"
        b"}",
    )
    accepted(
        program,
        "callee-requirement-demands-a-lent-field",
        b"data T { count: u64; items: [u8; 16] in Trapping; }\n"
        b"machine T::put(t: &mut T)\nrequires t.count < 16\n{ t.items[t.count] = 1; }\n"
        b"machine m(t: &mut T) {\n"
        b"    transition t.count < 16 {\n"
        b"        true -> later(t)\n"
        b"        _ -> {}\n"
        b"    }\n"
        b"\n"
        b"    state later(t: &mut T) { transition { _ -> put(t) } }\n"
        b"    state put(t: &mut T) { T::put(t); }\n"
        b"}",
    )
    # An established value's `where` facts hold on every path, so a field
    # first read in a `&&` right operand keeps them past the join.
    accepted(
        program,
        "domain-fact-entered-in-a-right-operand",
        b"data Table where args <= 16, { args: u64; }\n"
        b"machine Table::rel(t: &mut Table, first: u64, count: u64) -> u64 {\n"
        b"    transition count > 0 && first <= t.args {\n"
        b"        true -> release(t, first)\n"
        b"        _ -> (0)\n"
        b"    }\n"
        b"\n"
        b"    state release(t: &mut Table, first: u64) -> u64\n"
        b"    requires first <= 16\n"
        b"    { (first) }\n"
        b"}",
    )

    # Write frames: a machine call forgets, below an exclusive argument, only
    # what its callee may write there: the fields it, its callees and its
    # states store, or the whole referent it may replace.
    accepted(
        program,
        "call-keeps-a-field-its-callee-does-not-write",
        b"data L { a: u64; b: u64; }\n"
        b"machine L::set_a(l: &mut L) { l.a = 99; }\n"
        b"machine L::set_b(l: &mut L) { l.b = 99; }\n"
        b"machine L::reset(l: &mut L) { l = L { a: 99, b: 0 }; }\n"
        b"machine L::outer(l: &mut L) { L::set_a(l); }\n"
        b"machine L::via(l: &mut L) {\n"
        b"    transition { _ -> put(l) }\n"
        b"    state put(l: &mut L) { l = L { a: 99, b: 0 }; }\n"
        b"}\n"
        b"machine set(x: &mut u64) { x = 99; }\n"
        b"machine read(l: &mut L, arr: &[u8; 16]) -> u8 {\n"
        b"    transition l.a < 16 {\n"
        b"        true -> ok(l, arr)\n"
        b"        _ -> (0)\n"
        b"    }\n"
        b"\n"
        b"    state ok(l: &mut L, arr: &[u8; 16]) -> u8 {\n"
        b"        L::set_b(l);\n"
        b"        arr[l.a]\n"
        b"    }\n"
        b"}",
    )
    rejected(
        program,
        "call-forgets-a-field-its-callee-writes",
        b"data L { a: u64; b: u64; }\n"
        b"machine L::set_a(l: &mut L) { l.a = 99; }\n"
        b"machine L::set_b(l: &mut L) { l.b = 99; }\n"
        b"machine L::reset(l: &mut L) { l = L { a: 99, b: 0 }; }\n"
        b"machine L::outer(l: &mut L) { L::set_a(l); }\n"
        b"machine L::via(l: &mut L) {\n"
        b"    transition { _ -> put(l) }\n"
        b"    state put(l: &mut L) { l = L { a: 99, b: 0 }; }\n"
        b"}\n"
        b"machine set(x: &mut u64) { x = 99; }\n"
        b"machine read(l: &mut L, arr: &[u8; 16]) -> u8 {\n"
        b"    transition l.a < 16 {\n"
        b"        true -> ok(l, arr)\n"
        b"        _ -> (0)\n"
        b"    }\n"
        b"\n"
        b"    state ok(l: &mut L, arr: &[u8; 16]) -> u8 {\n"
        b"        L::set_a(l);\n"
        b"        arr[l.a]\n"
        b"    }\n"
        b"}",
        "IndexOutOfBounds",
        b"arr[l.a]",
    )
    rejected(
        program,
        "call-forgets-a-whole-referent-write",
        b"data L { a: u64; b: u64; }\n"
        b"machine L::set_a(l: &mut L) { l.a = 99; }\n"
        b"machine L::set_b(l: &mut L) { l.b = 99; }\n"
        b"machine L::reset(l: &mut L) { l = L { a: 99, b: 0 }; }\n"
        b"machine L::outer(l: &mut L) { L::set_a(l); }\n"
        b"machine L::via(l: &mut L) {\n"
        b"    transition { _ -> put(l) }\n"
        b"    state put(l: &mut L) { l = L { a: 99, b: 0 }; }\n"
        b"}\n"
        b"machine set(x: &mut u64) { x = 99; }\n"
        b"machine read(l: &mut L, arr: &[u8; 16]) -> u8 {\n"
        b"    transition l.a < 16 {\n"
        b"        true -> ok(l, arr)\n"
        b"        _ -> (0)\n"
        b"    }\n"
        b"\n"
        b"    state ok(l: &mut L, arr: &[u8; 16]) -> u8 {\n"
        b"        L::reset(l);\n"
        b"        arr[l.a]\n"
        b"    }\n"
        b"}",
        "IndexOutOfBounds",
        b"arr[l.a]",
    )
    rejected(
        program,
        "call-forgets-a-callees-callee-write",
        b"data L { a: u64; b: u64; }\n"
        b"machine L::set_a(l: &mut L) { l.a = 99; }\n"
        b"machine L::set_b(l: &mut L) { l.b = 99; }\n"
        b"machine L::reset(l: &mut L) { l = L { a: 99, b: 0 }; }\n"
        b"machine L::outer(l: &mut L) { L::set_a(l); }\n"
        b"machine L::via(l: &mut L) {\n"
        b"    transition { _ -> put(l) }\n"
        b"    state put(l: &mut L) { l = L { a: 99, b: 0 }; }\n"
        b"}\n"
        b"machine set(x: &mut u64) { x = 99; }\n"
        b"machine read(l: &mut L, arr: &[u8; 16]) -> u8 {\n"
        b"    transition l.a < 16 {\n"
        b"        true -> ok(l, arr)\n"
        b"        _ -> (0)\n"
        b"    }\n"
        b"\n"
        b"    state ok(l: &mut L, arr: &[u8; 16]) -> u8 {\n"
        b"        L::outer(l);\n"
        b"        arr[l.a]\n"
        b"    }\n"
        b"}",
        "IndexOutOfBounds",
        b"arr[l.a]",
    )
    rejected(
        program,
        "call-forgets-a-states-whole-write",
        b"data L { a: u64; b: u64; }\n"
        b"machine L::set_a(l: &mut L) { l.a = 99; }\n"
        b"machine L::set_b(l: &mut L) { l.b = 99; }\n"
        b"machine L::reset(l: &mut L) { l = L { a: 99, b: 0 }; }\n"
        b"machine L::outer(l: &mut L) { L::set_a(l); }\n"
        b"machine L::via(l: &mut L) {\n"
        b"    transition { _ -> put(l) }\n"
        b"    state put(l: &mut L) { l = L { a: 99, b: 0 }; }\n"
        b"}\n"
        b"machine set(x: &mut u64) { x = 99; }\n"
        b"machine read(l: &mut L, arr: &[u8; 16]) -> u8 {\n"
        b"    transition l.a < 16 {\n"
        b"        true -> ok(l, arr)\n"
        b"        _ -> (0)\n"
        b"    }\n"
        b"\n"
        b"    state ok(l: &mut L, arr: &[u8; 16]) -> u8 {\n"
        b"        L::via(l);\n"
        b"        arr[l.a]\n"
        b"    }\n"
        b"}",
        "IndexOutOfBounds",
        b"arr[l.a]",
    )
    accepted(
        program,
        "call-keeps-a-field-it-does-not-lend",
        b"data L { a: u64; b: u64; }\n"
        b"machine L::set_a(l: &mut L) { l.a = 99; }\n"
        b"machine L::set_b(l: &mut L) { l.b = 99; }\n"
        b"machine L::reset(l: &mut L) { l = L { a: 99, b: 0 }; }\n"
        b"machine L::outer(l: &mut L) { L::set_a(l); }\n"
        b"machine L::via(l: &mut L) {\n"
        b"    transition { _ -> put(l) }\n"
        b"    state put(l: &mut L) { l = L { a: 99, b: 0 }; }\n"
        b"}\n"
        b"machine set(x: &mut u64) { x = 99; }\n"
        b"machine read(l: &mut L, arr: &[u8; 16]) -> u8 {\n"
        b"    transition l.a < 16 {\n"
        b"        true -> ok(l, arr)\n"
        b"        _ -> (0)\n"
        b"    }\n"
        b"\n"
        b"    state ok(l: &mut L, arr: &[u8; 16]) -> u8 {\n"
        b"        set(&mut l.b);\n"
        b"        arr[l.a]\n"
        b"    }\n"
        b"}",
    )
    rejected(
        program,
        "call-forgets-a-lent-field-written-whole",
        b"data L { a: u64; b: u64; }\n"
        b"machine L::set_a(l: &mut L) { l.a = 99; }\n"
        b"machine L::set_b(l: &mut L) { l.b = 99; }\n"
        b"machine L::reset(l: &mut L) { l = L { a: 99, b: 0 }; }\n"
        b"machine L::outer(l: &mut L) { L::set_a(l); }\n"
        b"machine L::via(l: &mut L) {\n"
        b"    transition { _ -> put(l) }\n"
        b"    state put(l: &mut L) { l = L { a: 99, b: 0 }; }\n"
        b"}\n"
        b"machine set(x: &mut u64) { x = 99; }\n"
        b"machine read(l: &mut L, arr: &[u8; 16]) -> u8 {\n"
        b"    transition l.a < 16 {\n"
        b"        true -> ok(l, arr)\n"
        b"        _ -> (0)\n"
        b"    }\n"
        b"\n"
        b"    state ok(l: &mut L, arr: &[u8; 16]) -> u8 {\n"
        b"        set(&mut l.a);\n"
        b"        arr[l.a]\n"
        b"    }\n"
        b"}",
        "IndexOutOfBounds",
        b"arr[l.a]",
    )

    # Crash routes: Trapping operations and crash statements are sites, a
    # private machine without crash clauses infers its causes, and every
    # other machine must declare what it derives.
    accepted(
        program,
        "declared-trapping-site",
        b"pub machine next(x: u64 in Trapping) -> u64 in Trapping\ncrashes Trap\n{ x + 1 }",
        {0: "T/T"},
    )
    rejected(
        program,
        "undeclared-trapping-site",
        b"pub machine next(x: u64 in Trapping) -> u64 in Trapping { x + 1 }",
        "CrashNotDeclared",
        b"pub machine next(x: u64 in Trapping) -> u64 in Trapping { x + 1 }",
    )
    accepted(
        program,
        "disproved-trapping-site",
        b"pub machine next(x: u8) -> u64 in Trapping { (x as u64 in Trapping) + 1 }",
        {0: "/"},
    )
    accepted(
        program,
        "private-machine-infers-and-callers-derive",
        b"machine bump(x: u64 in Trapping) -> u64 in Trapping { x + 1 }\n"
        b"pub machine outer(x: u64 in Trapping) -> u64 in Trapping\ncrashes Trap\n{ bump(x) }",
        {0: "?/T", 1: "T/T"},
    )
    rejected(
        program,
        "public-caller-must-declare-a-callee-cause",
        b"machine bump(x: u64 in Trapping) -> u64 in Trapping { x + 1 }\n"
        b"pub machine bare(x: u64 in Trapping) -> u64 in Trapping { bump(x) }",
        "CrashNotDeclared",
        b"pub machine bare(x: u64 in Trapping) -> u64 in Trapping { bump(x) }",
    )
    accepted(
        program,
        "crash-statement",
        b"pub machine stop()\ncrashes Abort\n{ crash Abort; }",
        {0: "A/A"},
    )
    rejected(
        program,
        "undeclared-crash-statement",
        b"pub machine stop()\ncrashes Trap\n{ crash Abort; }",
        "CrashNotDeclared",
        b"pub machine stop()\ncrashes Trap\n{ crash Abort; }",
    )

    # Contract facts: `requires` facts hold in their body and are owed at
    # each call and jump; `where` facts hold of an established value's
    # fields and are owed again after a field store.
    accepted(
        program,
        "requirement-holds-in-its-body",
        b"machine read(a: &[u8; 4], i: u64) -> u8\nrequires i < 4\n{ a[i] }",
    )
    accepted(
        program,
        "callee-requirement-proved-at-the-call",
        b"machine read(a: &[u8; 4], i: u64) -> u8\nrequires i < 4\n{ a[i] }\n"
        b"machine first(a: &[u8; 4]) -> u8 { read(a, 0) }\n"
        b"machine guarded(a: &[u8; 4], i: u64) -> u8 {\n"
        b"    transition i < 4 {\n"
        b"        true -> found(a, i)\n"
        b"        _ -> missing()\n"
        b"    }\n"
        b"\n"
        b"    state found(a: &[u8; 4], i: u64) -> u8 { read(a, i) }\n"
        b"    state missing() -> u8 { 0 }\n"
        b"}",
    )
    rejected(
        program,
        "callee-requirement-is-unproven",
        b"machine positive(x: u64) -> u64\nrequires x > 0\n{ x }\n"
        b"machine caller(x: u64) -> u64 { positive(x) }",
        "RequirementUnproven",
        b"positive(x)",
    )
    accepted(
        program,
        "state-requirement-proved-at-the-jump",
        b"machine f(x: u64) -> u64 {\n"
        b"    transition x > 3 {\n"
        b"        true -> g(x)\n"
        b"        _ -> h()\n"
        b"    }\n"
        b"\n"
        b"    state g(x: u64) -> u64\n"
        b"    requires x > 0\n"
        b"    { x - 1 }\n"
        b"\n"
        b"    state h() -> u64 { 0 }\n"
        b"}",
    )
    rejected(
        program,
        "state-requirement-is-unproven",
        b"machine f(x: u64) -> u64 {\n"
        b"    transition { _ -> g(x) }\n"
        b"\n"
        b"    state g(x: u64) -> u64\n"
        b"    requires x > 0\n"
        b"    { x }\n"
        b"}",
        "RequirementUnproven",
        b"g(x)",
    )
    # `ensures` facts: every normal return owes them, `result` standing for
    # the returned value, and a caller assumes them of the call's result.
    accepted(
        program,
        "guarantee-proved-at-each-return",
        b"machine clamp(x: u64) -> u64\nensures result <= 10\n{\n"
        b"    transition x <= 10 {\n"
        b"        true -> (x)\n"
        b"        _ -> capped()\n"
        b"    }\n"
        b"\n"
        b"    state capped() -> u64 { 10 }\n"
        b"}",
    )
    rejected(
        program,
        "guarantee-is-unproven",
        b"machine f(x: u64) -> u64\nensures result < 10\n{ x }",
        "GuaranteeUnproven",
        b"x!2",
    )
    rejected(
        program,
        "unit-return-owes-its-guarantee",
        b"machine f(x: u64)\nensures x > 0\n{}",
        "GuaranteeUnproven",
        b"machine f(x: u64)\nensures x > 0\n{}",
    )
    accepted(
        program,
        "caller-assumes-the-guarantee",
        b"machine small(x: u64) -> u64\nensures result < 4\n{ x % 4 }\n"
        b"machine read(a: &[u8; 4], x: u64) -> u8 {\n"
        b"    let i: u64 = small(x);\n"
        b"    a[i]\n"
        b"}",
    )
    accepted(
        program,
        "guarantee-relates-result-and-arguments",
        b"machine below(x: u64) -> u64\nrequires x > 0\nensures result < x\n{ x - 1 }\n"
        b"machine read(a: &[u8; 8], x: u64) -> u8\nrequires x > 0, x <= 8\n{ a[below(x)] }",
    )
    accepted(
        program,
        "terminal-call-returns-the-callee-guarantee",
        b"machine small(x: u64) -> u64\nensures result < 4\n{ x % 4 }\n"
        b"machine wrap(x: u64) -> u64\nensures result < 4\n{ transition { _ -> small(x) } }",
    )
    rejected(
        program,
        "unguaranteed-call-result-bounds-nothing",
        b"machine any(x: u64) -> u64 { x }\n"
        b"machine read(a: &[u8; 4], x: u64) -> u8 { a[any(x)] }",
        "IndexOutOfBounds",
        b"a[any(x)]",
    )
    # An operator typing selected a machine for is a call of that machine:
    # its `requires` facts are owed with the operands as arguments.
    vector = (
        b"data V { x: u64; }\n"
        b"machine - V::minus(a: V, b: V) -> V\nrequires a.x >= b.x\n{ V { x: a.x - b.x } }\n"
    )
    rejected(
        program,
        "operator-owes-its-machine-requirement",
        vector + b"machine f(a: V, b: V) -> V { a - b }",
        "RequirementUnproven",
        b"a - b",
    )
    accepted(
        program,
        "guard-proves-an-operator-requirement",
        vector + b"machine f(a: V, b: V) -> V {\n"
        b"    transition a.x >= b.x {\n"
        b"        true -> (a - b)\n"
        b"        _ -> (a)\n"
        b"    }\n"
        b"}",
    )

    # A machine binder's call owes its `where machine` contract's `requires`
    # facts and derives the crash causes the contract declares; whatever its
    # exclusive arguments reach may change. A static machine argument
    # refines the contract: its published crash causes lie within the
    # contract's, and premises of its own reject as unsupported.
    accepted(
        program,
        "binder-call-proves-its-contract",
        b"machine apply<machine F>(x: u64) -> u64\nwhere machine F(value: u64) -> u64 requires value > 0;\n{\n"
        b"    transition x > 0 {\n"
        b"        true -> called(x)\n"
        b"        _ -> skipped()\n"
        b"    }\n"
        b"\n"
        b"    state called(x: u64) -> u64 { F(x) }\n"
        b"    state skipped() -> u64 { 0 }\n"
        b"}",
    )
    rejected(
        program,
        "binder-call-owes-its-contract",
        b"machine apply<machine F>(x: u64) -> u64\nwhere machine F(value: u64) -> u64 requires value > 0;\n{ F(x) }",
        "RequirementUnproven",
        b"F(x)",
    )
    rejected(
        program,
        "binder-call-releases-exclusive-arguments",
        b"data P { x: u64; }\n"
        b"machine read(a: &[u8; 4], i: u64) -> u8\nrequires i < 4\n{ a[i] }\n"
        b"machine apply<machine F>(p: &mut P, a: &[u8; 4]) -> u8\nwhere machine F(target: &mut P);\n{\n"
        b"    p.x = 1;\n"
        b"    F(p);\n"
        b"    read(a, p.x)\n"
        b"}",
        "RequirementUnproven",
        b"read(a, p.x)",
    )
    accepted(
        program,
        "static-machine-argument-within-contract-crash-causes",
        b"machine bump(x: u64 in Trapping) -> u64 in Trapping { x + 1 }\n"
        b"machine apply<machine F>(x: u64 in Trapping) -> u64 in Trapping\n"
        b"where machine F(value: u64 in Trapping) -> u64 in Trapping crashes Trap;\n{ F(x) }\n"
        b"pub machine outer(x: u64 in Trapping) -> u64 in Trapping\ncrashes Trap\n{ apply<bump>(x) }",
        {0: "?/T", 1: "?/T", 2: "T/T"},
    )
    rejected(
        program,
        "binder-crash-causes-are-declared",
        b"pub machine apply<machine F>(x: u64 in Trapping) -> u64 in Trapping\n"
        b"where machine F(value: u64 in Trapping) -> u64 in Trapping crashes Trap;\n{ F(x) }",
        "CrashNotDeclared",
        b"pub machine apply<machine F>(x: u64 in Trapping) -> u64 in Trapping\n"
        b"where machine F(value: u64 in Trapping) -> u64 in Trapping crashes Trap;\n{ F(x) }",
    )
    rejected(
        program,
        "static-machine-argument-crash-outside-contract",
        b"machine bump(x: u64 in Trapping) -> u64 in Trapping { x + 1 }\n"
        b"machine apply<machine F>(x: u64 in Trapping) -> u64 in Trapping\n"
        b"where machine F(value: u64 in Trapping) -> u64 in Trapping;\n{ F(x) }\n"
        b"machine outer(x: u64 in Trapping) -> u64 in Trapping { apply<bump>(x) }",
        "CrashNotDeclared",
        b"bump!2",
    )
    rejected(
        program,
        "forwarded-binder-crash-outside-contract",
        b"machine apply<machine F>(x: u64 in Trapping) -> u64 in Trapping\n"
        b"where machine F(value: u64 in Trapping) -> u64 in Trapping;\n{ F(x) }\n"
        b"machine again<machine G>(x: u64 in Trapping) -> u64 in Trapping\n"
        b"where machine G(value: u64 in Trapping) -> u64 in Trapping crashes Trap;\n{ apply<G>(x) }",
        "CrashNotDeclared",
        b"G!3",
    )
    rejected(
        program,
        "static-machine-argument-premises-are-unsupported",
        b"machine positive(x: u64) -> u64\nrequires x > 0\n{ x }\n"
        b"machine apply<machine F>(x: u64) -> u64\nwhere machine F(value: u64) -> u64;\n{ F(x) }\n"
        b"machine outer(x: u64) -> u64 { apply<positive>(x) }",
        "UnsupportedFact",
        b"positive!2",
    )

    # A satisfier refines its requirement: its published crash causes lie
    # within the requirement's, and facts this slice cannot relate reject.
    # A body promising termination rejects until checking ranks it.
    accepted(
        program,
        "satisfier-within-requirement-crash-causes",
        b"trait Next { machine next(x: u64 in Trapping) -> u64 in Trapping crashes Trap; }\n"
        b"machine bump(x: u64 in Trapping) -> u64 in Trapping satisfies Next::next crashes Trap { x + 1 }",
    )
    rejected(
        program,
        "satisfier-crash-outside-requirement",
        b"trait Next { machine next(x: u64 in Trapping) -> u64 in Trapping; }\n"
        b"machine bump(x: u64 in Trapping) -> u64 in Trapping satisfies Next::next { x + 1 }",
        "CrashNotDeclared",
        b"satisfies Next::next",
    )
    rejected(
        program,
        "requirement-facts-are-unsupported",
        b"trait Positive { machine get() -> u64 ensures result > 0; }\n"
        b"machine one() -> u64 satisfies Positive::get { 1 }",
        "UnsupportedFact",
        b"satisfies Positive::get",
    )
    # A declared domain's predicates hold of a qualified parameter and are
    # proved where a value meets a qualified destination, unless it already
    # carries the qualification; a routed domain or a byte predicate is
    # established only by carrying it.
    small = b"domain u64::Small(value)\nrequires value < 4;\nmachine take(index: u64 in Small) -> u64 { index }\n"
    accepted(
        program,
        "qualified-parameter-assumes-its-domain",
        b"domain u64::Small(value)\nrequires value < 4;\nmachine at(items: &[u8; 4], index: u64 in Small) -> u8 { items[index] }",
    )
    rejected(
        program,
        "qualifying-argument-proves-the-domain",
        small + b"machine f(candidate: u64) -> u64 { take(candidate) }",
        "DomainUnproven",
        b"candidate!2",
    )
    accepted(
        program,
        "guarded-argument-meets-the-domain",
        small + b"machine f(i: u64) -> u64 {\n"
        b"    transition i < 4 {\n"
        b"        true -> held(i)\n"
        b"        _ -> missing()\n"
        b"    }\n"
        b"\n"
        b"    state held(i: u64) -> u64 { take(i) }\n"
        b"    state missing() -> u64 { 0 }\n"
        b"}",
    )
    accepted(program, "qualified-argument-carries-its-domain", small + b"machine f(i: u64 in Small) -> u64 { take(i) }")
    # An arm's `if` guard holds on the selected arm; passing over a guarded
    # wildcard establishes the guard's failure
    # (wiki/spec/language/patterns.md#coverage-and-domain-tests).
    guarded = (
        b"data T { case A; case B(v: u64); }\n"
        b"machine three(x: u64) requires x == 3 {}\n"
        b"machine small(x: u64) requires x < 4 {}\n"
        b"machine f(t: T, x: u64) {\n"
        b"    transition t {\n"
        b"        T::B { v } GUARD-> three(v)\n"
        b"        _ if x > 3 -> {}\n"
        b"        _ -> small(x)\n"
        b"    }\n"
        b"}"
    )
    accepted(program, "arm-guards-are-facts-of-their-arms", guarded.replace(b"GUARD", b"if v == 3 "))
    rejected(program, "an-unguarded-arm-proves-nothing", guarded.replace(b"GUARD", b""), "RequirementUnproven", b"three(v)")
    rejected(
        program,
        "a-guarded-case-arm-passed-over-proves-nothing",
        b"data T { case A; case B(v: u64); }\nmachine small(x: u64) requires x < 4 {}\n"
        b"machine f(t: T, x: u64) {\n    transition t {\n        T::B { v } if x > 3 -> {}\n        _ -> small(x)\n    }\n}",
        "RequirementUnproven",
        b"small(x)",
    )
    accepted(program, "literal-within-the-domain", small + b"machine f() -> u64 { take(3) }")
    rejected(program, "literal-outside-the-domain", small + b"machine f() -> u64 { take(9) }", "DomainUnproven", b"9")
    rejected(
        program,
        "qualified-result-proves-the-domain",
        b"domain u64::Small(value)\nrequires value < 4;\nmachine f(count: u64) -> u64 in Small { count }",
        "DomainUnproven",
        b"count!2",
    )
    rejected(
        program,
        "qualifying-cast-proves-the-domain",
        b"domain u64::Small(value)\nrequires value < 4;\nmachine f(i: u64) -> u64 { let s: u64 in Small = i as u64 in Small; s }",
        "DomainUnproven",
        b"i as u64 in Small",
    )
    rejected(
        program,
        "routed-domain-is-not-minted",
        b"machine mint() -> u64;\ndomain u64::Issued\nestablished by mint;\n"
        b"machine take(token: u64 in Issued) -> u64 in Issued { token }\nmachine f() -> u64 in Issued { take(1) }",
        "DomainUnproven",
        b"1",
    )
    text = b"domain [u8]::Text(bytes)\nrequires valid_utf8(bytes);\nmachine show(text: &[u8] in Text) {}\n"
    rejected(program, "byte-predicate-domain-needs-carried-evidence", text + b"machine f(raw: &[u8]) { show(raw); }",
             "DomainUnproven", b"raw!2")
    accepted(program, "byte-predicate-domain-carried", text + b"machine f(view: &[u8] in Text) { show(view); }")

    # A value-position match: each arm runs on the facts after its subject,
    # its own pattern selected and every earlier one passed over; the facts
    # after the match are its arms' join and its value their hull. An arm
    # value landed at a qualified match type proves the qualification.
    accepted(
        program,
        "match-arm-assumes-its-literal",
        b"machine f(a: &[u8; 4], i: u64) -> u8 { match i { 3 -> a[i], _ -> 0 } }",
    )
    rejected(
        program,
        "match-passed-over-literal-bounds-nothing",
        b"machine f(a: &[u8; 4], i: u64) -> u8 { match i { 9 -> 0, _ -> a[i] } }",
        "IndexOutOfBounds",
        b"a[i]",
    )
    accepted(
        program,
        "match-boolean-arms-assume-their-subject",
        b"machine f(a: &[u8; 4], i: u64) -> u8 { match i < 4 { false -> 0, true -> a[i] } }",
    )
    accepted(
        program,
        "match-passed-over-boolean-holds-after",
        b"machine f(a: &[u8; 4], i: u64) -> u8 { match i < 4 { false -> 0, _ -> a[i] } }",
    )
    rejected(
        program,
        "match-arm-facts-stay-in-their-arm",
        b"machine f(a: &[u8; 4], i: u64) -> u8 { match i < 4 { true -> 0, false -> a[i] } }",
        "IndexOutOfBounds",
        b"a[i]",
    )
    rejected(
        program,
        "match-join-keeps-only-shared-facts",
        b"machine f(a: &[u8; 4], i: u64) -> u8 { let x: u8 = match i < 4 { true -> a[i], false -> 0 }; a[i] }",
        "IndexOutOfBounds",
        b"a[i]!2",
    )
    accepted(
        program,
        "match-value-is-its-arms-hull",
        b"machine f(a: &[u8; 4], c: u8) -> u8 { let i: u64 = match c { 0 -> 1, 1 -> 3, _ -> 2 }; a[i] }",
    )
    rejected(
        program,
        "match-hull-reaches-every-arm",
        b"machine f(a: &[u8; 4], c: u8) -> u8 { let i: u64 = match c { 0 -> 1, 1 -> 4, _ -> 2 }; a[i] }",
        "IndexOutOfBounds",
        b"a[i]",
    )
    accepted(
        program,
        "match-nested-arms-keep-their-facts",
        b"machine f(a: &[u8; 4], i: u64, j: u64) -> u8 {\n"
        b"    match i { 2 -> match j { 1 -> a[i + j] _ -> a[i] } _ -> 0 }\n"
        b"}",
    )
    accepted(
        program,
        "match-constant-pattern-selects-equality",
        b"const LAST: u64 = 3;\nmachine f(a: &[u8; 4], i: u64) -> u8 { match i { LAST -> a[i], _ -> 0 } }",
    )
    accepted(
        program,
        "transition-constant-path-selects-equality",
        b"data Bounds { x: u8; }\nconst Bounds::LAST: u64 = 3;\n"
        b"machine f(a: &[u8; 4], i: u64) -> u8 {\n"
        b"    transition i {\n"
        b"        Bounds::LAST -> at(a, i)\n"
        b"        _ -> none()\n"
        b"    }\n"
        b"\n"
        b"    state at(a: &[u8; 4], i: u64) -> u8 { a[i] }\n"
        b"    state none() -> u8 { 0 }\n"
        b"}",
    )
    rejected(
        program,
        "match-constant-pattern-outside-the-array",
        b"const PAST: u64 = 4;\nmachine f(a: &[u8; 4], i: u64) -> u8 { match i { PAST -> a[i], _ -> 0 } }",
        "IndexOutOfBounds",
        b"a[i]",
    )
    # Every state is checked: one no walk reaches from its parameters' carrier
    # ranges and its own `requires` facts.
    # A whole membership fact is its domain's predicates of the value; a
    # case membership must hold only where checking keeps case knowledge.
    member = b"domain u64::Small(value)\nrequires value < 4;\n"
    accepted(
        program,
        "membership-requires-assumes-its-domain",
        member + b"machine at(a: &[u8; 4], i: u64) -> u8\nrequires i in Small\n{ a[i] }",
    )
    rejected(
        program,
        "membership-requires-is-proved-at-calls",
        member + b"machine take(i: u64) -> u64\nrequires i in Small\n{ i }\nmachine caller(c: u64) -> u64 { take(c) }",
        "RequirementUnproven",
        b"take(c)",
    )
    accepted(
        program,
        "membership-carried-by-the-argument",
        member + b"machine take(i: u64) -> u64\nrequires i in Small\n{ i }\nmachine caller(c: u64 in Small) -> u64 { take(c) }",
    )
    rejected(
        program,
        "case-membership-must-hold-unsupported",
        b"data K { case A; case B; }\nmachine f(k: K) -> u8\nrequires k in K::A\n{ 0 }\nmachine g(k: K) -> u8 { f(k) }",
        "UnsupportedFact",
        b"k in K::A",
    )
    rejected(
        program,
        "unreached-state-is-checked",
        b"machine f(a: &[u8; 4], i: u64) -> u8 { transition { _ -> 0 } state g(a: &[u8; 4], i: u64) -> u8 { a[i] } }",
        "IndexOutOfBounds",
        b"a[i]",
    )
    accepted(
        program,
        "unreached-state-assumes-its-requires",
        b"machine f(a: &[u8; 4], i: u64) -> u8 { transition { _ -> 0 } state g(a: &[u8; 4], i: u64) -> u8 requires i < 4 { a[i] } }",
    )
    rejected(
        program,
        "inclusive-slice-end-is-an-index",
        b"machine prefix(values: &[u8]) -> u64 { let end: i64 = -1; let view: &[u8] = &values[0..=end]; view.len }",
        "IndexOutOfBounds",
        b"values[0..=end]",
    )
    accepted(program, "array-literal-index-within-its-extent", b"machine f() -> u8 { let a: [u8; 4] = [1, 2, 3, 4]; a[3] }")
    rejected(
        program,
        "array-literal-index-past-its-extent",
        b"machine f() -> u8 { let a: [u8; 4] = [1, 2, 3, 4]; a[4] }",
        "IndexOutOfBounds",
        b"a[4]",
    )
    rejected(
        program,
        "array-literal-element-proves-its-domain",
        b"domain u8::Small(value)\nrequires value < 4;\nmachine f() -> u8 { let a: [u8 in Small; 2] = [1, 9]; 0 }",
        "DomainUnproven",
        b"9",
    )
    accepted(program, "literal-constant-is-its-value", b"const LAST: u64 = 3;\nmachine f(a: &[u8; 4]) -> u8 { a[LAST] }")
    rejected(
        program,
        "literal-constant-outside-the-array",
        b"const PAST: u64 = 4;\nmachine f(a: &[u8; 4]) -> u8 { a[PAST] }",
        "IndexOutOfBounds",
        b"a[PAST]",
    )
    accepted(program, "match-landed-arms-meet-the-domain", small + b"machine f(c: u8) -> u64 { take(match c { 0 -> 1, _ -> 3 }) }")
    rejected(
        program,
        "match-landed-arm-proves-the-domain",
        small + b"machine f(c: u8) -> u64 { take(match c { 0 -> 1, _ -> 9 }) }",
        "DomainUnproven",
        b"9",
    )

    # A `const` extent serves every specialization, so its body proves no
    # index below it.
    rejected(
        program,
        "const-extent-proves-no-index",
        b"machine at<const N: u64>(items: &[u8; N]) -> u8 { items[0] }",
        "IndexOutOfBounds",
        b"items[0]",
    )

    # Termination: a `terminates by` witness strictly descends on every
    # cyclic transition (a state cycle or a self call), each subject read in
    # the source as its state formal of that name and after the edge as the
    # argument its target formal receives; a bare `terminates;` promise
    # needs a witness for any cycle and terminating callees; a productive
    # loop that promises nothing owes no rank.
    countdown = (
        b"machine count(n: u64) -> u64\nterminates by n;\n{\n"
        b"    transition n > 0 {\n"
        b"        true -> step(n - 1)\n"
        b"        _ -> done()\n"
        b"    }\n"
        b"\n"
        b"    state step(n: u64) -> u64 {\n"
        b"        transition n > 0 {\n"
        b"            true -> STEP\n"
        b"            _ -> done()\n"
        b"        }\n"
        b"    }\n"
        b"\n"
        b"    state done() -> u64 { 0 }\n"
        b"}"
    )
    accepted(program, "ranked-state-cycle-descends", countdown.replace(b"STEP", b"step(n - 1)"))
    rejected(
        program,
        "ranked-state-cycle-must-descend",
        countdown.replace(b"STEP", b"step(n)"),
        "RankUnproven",
        b"step(n)",
    )
    rejected(
        program,
        "promised-cycle-needs-a-witness",
        countdown.replace(b"terminates by n;", b"terminates;").replace(b"STEP", b"step(n - 1)"),
        "RankUnproven",
        b"step(n - 1)!2",
    )
    accepted(
        program,
        "productive-loop-owes-no-rank",
        countdown.replace(b"terminates by n;\n", b"").replace(b"STEP", b"step(n)"),
    )
    walk = (
        b"machine walk(items: &[u8], i: u64) -> u64\n"
        b"terminates by (i, items.len) -> Nat::BoundedDistance;\n{\n"
        b"    transition i < items.len {\n"
        b"        true -> NEXT\n"
        b"        _ -> (i)\n"
        b"    }\n"
        b"}"
    )
    accepted(program, "self-call-climbs-its-bounded-distance", walk.replace(b"NEXT", b"walk(items, i + 1)"))
    # A bounded byte array (wiki/spec/terminal-psi/structural_access.md): a
    # quoted literal's byte predicates replay from its octets, a replacement
    # sets the live length, `.len` is that length, and a value's declared
    # domain supplies the predicates it requires.
    bounded = (
        b"domain [u8; 8]::Utf8(bytes) requires valid_utf8(bytes);\n"
        b"data M { line: [u8; 8] in Utf8; }\n"
        b"machine two(n: u64) requires n == 2 {}\n"
        b"machine text(bytes: &[u8; 8] in Utf8) requires valid_utf8(bytes) {}\n"
        b"machine f(m: &mut M) {\n    m.line = LITERAL;\n    two(m.line.len);\n    text(&m.line);\n}"
    )
    accepted(program, "a-quoted-literal-replaces-a-bounded-byte-array", bounded.replace(b"LITERAL", b"\"ok\""))
    rejected(
        program,
        "a-quoted-literal-must-satisfy-the-domain",
        bounded.replace(b"LITERAL", b"\"\\xff\\xfe\"").replace(b"two(m.line.len);\n", b""),
        "DomainUnproven",
        b"m.line = \"\\xff\\xfe\";",
    )
    rejected(
        program,
        "a-bounded-length-is-the-replacement's",
        bounded.replace(b"LITERAL", b"\"three\""),
        "RequirementUnproven",
        b"two(m.line.len)",
    )
    # A rank range (wiki/spec/language/termination.md#ranking) holds of the
    # entry rank under the entry's facts and of every cyclic arrival's rank,
    # whose endpoints must be the entry's.
    ranged = (
        b"machine count(n: u64, limit: u64) -> u64\nREQUIRES"
        b"terminates by n in 0..=limit;\n{\n"
        b"    transition n > 0 {\n        true -> step(n - 1, NEXT)\n        _ -> done()\n    }\n\n"
        b"    state step(n: u64, limit: u64) -> u64 {\n"
        b"        transition n > 0 {\n            true -> step(n - 1, limit)\n            _ -> done()\n        }\n    }\n\n"
        b"    state done() -> u64 { 0 }\n}"
    )
    accepted(program, "ranked-range-holds-at-entry-and-arrival", ranged.replace(b"REQUIRES", b"requires n <= limit\n").replace(b"NEXT", b"limit"))
    rejected(
        program,
        "ranked-range-needs-the-entry-rank-within",
        ranged.replace(b"REQUIRES", b"").replace(b"NEXT", b"limit"),
        "RankRangeUnproven",
        b"0..=limit",
    )
    rejected(
        program,
        "ranked-range-endpoints-are-pinned",
        ranged.replace(b"REQUIRES", b"requires n <= limit\n").replace(b"NEXT", b"0"),
        "RankRangeUnproven",
        b"step(n - 1, limit)",
    )
    # A rank range is proved under the subject's data `where` facts, a
    # distance view's rank lies within its limit by construction, and a
    # `Slice::Length` view ranks by the live length
    # (wiki/spec/language/termination.md#ranking).
    accepted(
        program,
        "a-measured-rank-range-holds-by-the-data's-where-facts",
        b"data Countdown where 0 <= remaining, remaining <= 5 { remaining: u64; }\n"
        b"measure Countdown::Remaining(countdown: Countdown) -> u64\n{ countdown.remaining }\n"
        b"machine walk(countdown: Countdown)\nterminates by countdown -> Countdown::Remaining in 0..=5;\n-> u64 {\n"
        b"    transition countdown.remaining > 0 {\n        true -> walk(Countdown { remaining: countdown.remaining - 1 })\n"
        b"        false -> countdown.remaining\n    }\n}",
    )
    accepted(
        program,
        "a-field-rank-range-holds-by-the-data's-where-facts",
        b"data Countdown where remaining <= 5 { remaining: u64; }\n"
        b"machine walk(countdown: Countdown)\nterminates by countdown.remaining in 0..=5;\n-> u64 {\n"
        b"    transition countdown.remaining > 0 {\n        true -> walk(Countdown { remaining: countdown.remaining - 1 })\n"
        b"        false -> countdown.remaining\n    }\n}",
    )
    rejected(
        program,
        "a-field-rank-range-beyond-the-data's-where-facts",
        b"data Countdown where remaining <= 9 { remaining: u64; }\n"
        b"machine walk(countdown: Countdown)\nterminates by countdown.remaining in 0..=5;\n-> u64 {\n"
        b"    transition countdown.remaining > 0 {\n        true -> walk(Countdown { remaining: countdown.remaining - 1 })\n"
        b"        false -> countdown.remaining\n    }\n}",
        "RankRangeUnproven",
        b"0..=5",
    )
    accepted(
        program,
        "an-increasing-view's-rank-lies-within-its-limit",
        b"machine walk(limit: u64, index: u64)\nterminates by index -> Nat::IncreasingTo(limit) in 0..=limit;\n-> u64\n{\n"
        b"    transition index < limit {\n        true -> walk(limit, index + 1)\n        false -> index\n    }\n}",
    )
    accepted(
        program,
        "a-bounded-distance-rank-lies-within-its-limit",
        b"machine walk(items: &[u8], i: u64) -> u64\nterminates by (i, items.len) -> Nat::BoundedDistance in 0..=items.len;\n{\n"
        b"    transition i < items.len {\n        true -> walk(items, i + 1)\n        _ -> (i)\n    }\n}",
    )
    # A layout query is known only by its `u64` type
    # (wiki/spec/language/counts_and_addresses.md).
    accepted(program, "a-layout-query-is-a-u64-value", b"data Pair { a: u64; b: u8; }\nmachine f() -> u64 { size_of<Pair>() }\nmachine g() -> u64 { align_of<Pair>() }")
    # A holder replaced through an exclusive reference, or by a call lending
    # it `&write`, retires its old loans and holds the replacement's
    # (wiki/spec/language/ownership.md#borrows-and-aliases); a call lending
    # it `&mut` keeps them.
    holders = (
        b"data Cell { value: i64; }\ndata Held { target: &Cell; stamp: i64; }\ndata Main { left: Cell; right: Cell; }\n"
        b"machine replace(held: &write Held, incoming: Held) { held = incoming; }\n"
        b"machine touch(held: &mut Held) {}\n"
        b"machine f(main: &mut Main) {\n    let mut held: Held = Held { target: &main.left, stamp: 17 };\n"
        b"    let incoming: Held = Held { target: &main.right, stamp: 23 };\n    REPLACE\n    main.left.value = 29;\n"
        b"    let observed: i64 = held.target.value;\n}"
    )
    accepted(program, "a-holder-replaced-through-a-reference-retires-its-loans", holders.replace(b"REPLACE", b"let slot: &mut Held = &mut held;\n    slot = incoming;"))
    accepted(program, "a-write-only-call-replaces-a-holder's-loans", holders.replace(b"REPLACE", b"replace(&write held, incoming);"))
    # The ownership walk reads the body's own places: a body followed by a
    # transition into states keeps the origins its reference locals captured.
    staged = holders.replace(b"    let observed: i64 = held.target.value;\n}", b"    let observed: i64 = held.target.value;\n    transition { _ -> done() }\n    state done() {}\n}")
    accepted(program, "a-holder-replaced-through-a-reference-retires-its-loans-before-a-transition", staged.replace(b"REPLACE", b"let slot: &mut Held = &mut held;\n    slot = incoming;"))
    rejected(
        program,
        "an-exclusive-call-keeps-a-holder's-loans-before-a-transition",
        staged.replace(b"REPLACE", b"touch(&mut held);"),
        "LoanConflict",
        b"main.left.value",
    )
    rejected(
        program,
        "an-exclusive-call-keeps-a-holder's-loans",
        holders.replace(b"REPLACE", b"touch(&mut held);"),
        "LoanConflict",
        b"main.left.value",
    )
    # A returned reference and a reference field stand for their referents
    # as a reference parameter does
    # (wiki/spec/language/lifetimes.md#returned-views): writes through them
    # are tracked, and a fact about the lent storage does not survive the
    # call that lent it.
    accepted(
        program,
        "a-returned-reference-is-written-through",
        b"data Cell { value: u64; }\nmachine nine(x: u64) requires x == 9 {}\n"
        b"machine pick(cell: &mut Cell) -> &mut u64 { &mut cell.value }\n"
        b"machine f(cell: &mut Cell) { let held: &mut u64 = pick(cell); held = 9; nine(held); }",
    )
    rejected(
        program,
        "a-lent-fact-does-not-survive-the-call",
        b"data Cell { value: u64; }\nmachine one(x: u64) requires x == 1 {}\n"
        b"machine pick(cell: &mut Cell) -> &mut u64 { &mut cell.value }\n"
        b"machine f(cell: &mut Cell) requires cell.value == 1 { let held: &mut u64 = pick(cell); held = 9; one(cell.value); }",
        "RequirementUnproven",
        b"one(cell.value)",
    )
    accepted(
        program,
        "a-reference-field-is-written-through",
        b"data Cell { value: i64; }\ndata Held { target: &mut Cell; }\nmachine same(x: i64) requires x == 89 {}\n"
        b"machine f(held: &mut Held) { held.target.value = 89; same(held.target.value); }",
    )
    # A range slice of a placed collection is a view with the range's extent
    # as its live length (wiki/spec/terminal-psi/byte_views.md); a write
    # through it forgets the collection's contents.
    accepted(
        program,
        "a-range-view-knows-its-length",
        b"machine two(n: u64) requires n == 2 {}\nmachine four(n: u64) requires n == 4 {}\n"
        b"machine f(items: &[u8; 4]) -> u8 {\n    let view: &[u8] = &items[..];\n    let middle: &[u8] = view[1..3];\n"
        b"    four(view.len);\n    two(middle.len);\n    view[3]\n}",
    )
    rejected(
        program,
        "a-range-view-bounds-its-elements",
        b"machine f(items: &[u8; 4]) -> u8 {\n    let middle: &[u8] = items[1..3];\n    middle[2]\n}",
        "IndexOutOfBounds",
        b"middle[2]",
    )
    rejected(
        program,
        "a-write-through-a-view-forgets-the-collection",
        b"machine three(x: u8) requires x == 3 {}\n"
        b"machine f(items: &mut [u8; 4]) requires items[0] == 3 {\n    let head: &mut [u8] = &mut items[0..2];\n    head[0] = 9;\n    three(items[0]);\n}",
        "RequirementUnproven",
        b"three(items[0])",
    )
    # A measure's body computes a rank over naturals
    # (wiki/spec/language/termination.md#ranking): its arithmetic is exact,
    # while the same operation in an ordinary body keeps its carrier
    # obligation.
    accepted(
        program,
        "a-measure-body-is-exact",
        b"data Countdown {}\nmeasure Countdown::Shifted(value: u8) -> u8 { value + 1 }\n"
        b"machine walk(remaining: u8)\nterminates by remaining -> Countdown::Shifted;\n-> u8 {\n"
        b"    transition remaining > 0 {\n        true -> walk(remaining - 1)\n        false -> remaining\n    }\n}",
    )
    rejected(program, "an-ordinary-body-owes-its-carrier", b"machine shift(value: u8) -> u8 { value + 1 }", "ArithmeticOverflow", b"value + 1")
    # A state formal carries the entry subject every arrival passes it,
    # whatever its spelling (wiki/spec/language/termination.md#subject-preservation);
    # a formal receiving another parameter carries no subject.
    carried = (
        b"data Countdown { remaining: u64; }\n"
        b"machine walk(countdown: Countdown, other: Countdown)\nterminates by countdown.remaining;\n-> u64 {\n"
        b"    transition { _ -> iterate(ARRIVAL) }\n"
        b"    state iterate(pending: Countdown) {\n        transition pending.remaining > 0 {\n"
        b"            true -> iterate(Countdown { remaining: pending.remaining - 1 })\n            false -> pending.remaining\n        }\n    }\n}"
    )
    accepted(program, "a-state-formal-carries-the-subject-it-receives", carried.replace(b"ARRIVAL", b"countdown"))
    rejected(program, "a-state-formal-receiving-another-parameter-ranks-nothing", carried.replace(b"ARRIVAL", b"other"), "RankUnproven", b"iterate(Countdown { remaining: pending.remaining - 1 })")
    accepted(
        program,
        "a-formal-carries-through-another-formal",
        b"machine walk(count: u64)\nterminates by count;\n-> u64 {\n    transition { _ -> first(count) }\n"
        b"    state first(pending: u64) {\n        transition { _ -> second(pending) }\n    }\n"
        b"    state second(left: u64) {\n        transition left > 0 {\n            true -> second(left - 1)\n            false -> left\n        }\n    }\n}",
    )
    # A `Slice::Length` witness rooted at a field path ranks the record's
    # slice by its live length. A rebuilt record's field takes the paths
    # below the view it was written from, so the view's shorter length
    # arrives in the state's formal.
    rebuilt = (
        b"data Bag { items: &[u32]; }\n"
        b"machine walk(bag: Bag, capacity: u64)\nrequires bag.items.len <= capacity;\n"
        b"terminates by bag.items -> Slice::Length in 0..=capacity;\n-> u64 {\n"
        b"    transition { _ -> step(bag, capacity) }\n"
        b"    state step(current: Bag, bound: u64) {\n        transition current.items.len > 0 {\n"
        b"            true -> step(Bag { items: current.items[RANGE] }, bound)\n            false -> 0\n        }\n    }\n}"
    )
    accepted(program, "a-rebuilt-record-field-ranks-by-its-view-length", rebuilt.replace(b"RANGE", b"1.."))
    rejected(
        program,
        "a-rebuilt-record-field-of-the-same-length-ranks-nothing",
        rebuilt.replace(b"RANGE", b"0.."),
        "RankUnproven",
        b"step(Bag { items: current.items[0..] }, bound)",
    )
    accepted(
        program,
        "a-nested-rebuilt-record-field-ranks-by-its-view-length",
        b"data Inner { items: &[u32]; }\ndata Outer { inner: Inner; }\n"
        b"machine walk(outer: Outer)\nterminates by outer.inner.items -> Slice::Length;\n-> u64 {\n"
        b"    transition { _ -> step(outer) }\n"
        b"    state step(current: Outer) {\n        transition current.inner.items.len > 0 {\n"
        b"            true -> step(Outer { inner: Inner { items: current.inner.items[1..] } })\n            false -> 0\n        }\n    }\n}",
    )
    # A declared measure ranks its subject by its body, or lexicographically
    # by the subject's named fields (wiki/spec/language/termination.md#ranking).
    measured = (
        b"data Card { power: u64; }\n"
        b"measure Card::PowerOrder(card: Card) -> u64 { card.power }\n"
        b"machine weaken(card: Card) -> u64\nterminates by card -> Card::PowerOrder;\n{\n"
        b"    transition card.power > 0 {\n        true -> NEXT\n        _ -> card.power\n    }\n}"
    )
    accepted(program, "self-call-descends-a-declared-measure", measured.replace(b"NEXT", b"weaken(Card { power: card.power - 1 })"))
    rejected(
        program,
        "a-declared-measure-must-descend",
        measured.replace(b"NEXT", b"weaken(card)"),
        "RankUnproven",
        b"weaken(card)",
    )
    ordered = (
        b"data Progress { outer: u64; inner: u64; }\n"
        b"measure Progress::Steps lexicographic { outer, inner }\n"
        b"machine scan(progress: Progress) -> u64\nterminates by progress -> Progress::Steps;\n{\n"
        b"    transition progress.inner > 0 {\n        true -> NEXT\n        _ -> progress.outer\n    }\n}"
    )
    accepted(
        program,
        "self-call-descends-a-lexicographic-measure",
        ordered.replace(b"NEXT", b"scan(Progress { outer: progress.outer, inner: progress.inner - 1 })"),
    )
    rejected(
        program,
        "a-lexicographic-measure-must-descend",
        ordered.replace(b"NEXT", b"scan(progress)"),
        "RankUnproven",
        b"scan(progress)",
    )
    rejected(
        program,
        "self-call-must-climb",
        walk.replace(b"NEXT", b"walk(items, i)"),
        "RankUnproven",
        b"walk(items, i)",
    )
    shrink = (
        b"machine total(bytes: &[u8]) -> u64\n"
        b"terminates by bytes -> Slice::Length;\n{\n"
        b"    transition bytes.len > 0 {\n"
        b"        true -> emit(bytes[1..])\n"
        b"        _ -> done()\n"
        b"    }\n"
        b"\n"
        b"    state emit(bytes: &[u8]) -> u64 {\n"
        b"        transition bytes.len > 0 {\n"
        b"            true -> TAIL\n"
        b"            _ -> done()\n"
        b"        }\n"
        b"    }\n"
        b"\n"
        b"    state done() -> u64 { 0 }\n"
        b"}"
    )
    accepted(program, "slice-length-shrinks", shrink.replace(b"TAIL", b"emit(bytes[1..])"))
    rejected(
        program,
        "slice-length-must-shrink",
        shrink.replace(b"TAIL", b"emit(bytes[0..])"),
        "RankUnproven",
        b"emit(bytes[0..])",
    )
    accepted(
        program,
        "promise-over-terminating-callees",
        b"machine helper(x: u64) -> u64 { x }\n"
        b"machine leaf(x: u64) -> u64 terminates;\n"
        b"machine run(x: u64) -> u64\nterminates;\n{\n"
        b"    let a: u64 = helper(x);\n"
        b"    leaf(a)\n"
        b"}",
    )
    rejected(
        program,
        "promise-over-a-productive-loop",
        b"machine spin(x: u64) -> u64 {\n"
        b"    transition { _ -> again(x) }\n"
        b"\n"
        b"    state again(x: u64) -> u64 {\n"
        b"        transition { _ -> again(x) }\n"
        b"    }\n"
        b"}\n"
        b"machine run(x: u64) -> u64\nterminates;\n{ spin(x) }",
        "ProgressUnproven",
        b"terminates;",
    )
    rejected(
        program,
        "promise-over-an-unpromised-leaf",
        b"machine leaf(x: u64) -> u64;\nmachine run(x: u64) -> u64\nterminates;\n{ leaf(x) }",
        "ProgressUnproven",
        b"terminates;",
    )
    rejected(
        program,
        "promise-over-mutual-recursion",
        b"machine ping(x: u64) -> u64\nterminates;\n{ pong(x) }\n"
        b"machine pong(x: u64) -> u64\nterminates;\n{ ping(x) }",
        "ProgressUnproven",
        b"terminates;",
    )
    rejected(
        program,
        "bodyless-witness-ranks-nothing",
        b"machine leaf(n: u64) -> u64 terminates by n;",
        "UnsupportedCheck",
        b"terminates by n;",
    )
    accepted(
        program,
        "bodyless-termination-promise",
        b"trait Algebra { machine zero() -> u64 terminates; }",
    )
    # The product's own accessor and push shapes: a guard on a field and the
    # field's `where` bound reach the state that indexes and stores.
    table = (
        b"data Table\nwhere\n    count <= 4,\n{ items: [u8; 4]; count: u64; }\n"
    )
    accepted(
        program,
        "field-guard-and-domain-bound-an-index",
        table
        + b"machine read(t: &Table, i: u64) -> u8 {\n"
        b"    transition i < t.count {\n"
        b"        true -> found(t, i)\n"
        b"        _ -> missing()\n"
        b"    }\n"
        b"\n"
        b"    state found(t: &Table, i: u64) -> u8 { t.items[i] }\n"
        b"    state missing() -> u8 { 0 }\n"
        b"}",
    )
    accepted(
        program,
        "field-guard-crosses-a-jump-and-closes-the-window",
        table
        + b"machine push(t: &mut Table, value: u8) -> bool {\n"
        b"    transition t.count < 4 {\n"
        b"        true -> store(t, value)\n"
        b"        _ -> full()\n"
        b"    }\n"
        b"\n"
        b"    state store(t: &mut Table, value: u8) -> bool {\n"
        b"        t.items[t.count] = value;\n"
        b"        t.count = t.count + 1;\n"
        b"        true\n"
        b"    }\n"
        b"\n"
        b"    state full() -> bool { false }\n"
        b"}",
    )
    rejected(
        program,
        "unclosed-window",
        table + b"machine grow(t: &mut Table) { t.count = t.count + 1; }",
        "DomainUnproven",
        b"t.count = t.count + 1;",
    )
    accepted(
        program,
        "construction-establishes-the-domain",
        b"data Range\nwhere\n    lo <= hi,\n{ lo: u64; hi: u64; }\n"
        b"machine make(x: u64) -> Range {\n"
        b"    transition x < 10 {\n"
        b"        true -> made(x)\n"
        b"        _ -> empty()\n"
        b"    }\n"
        b"\n"
        b"    state made(x: u64) -> Range { Range { lo: x, hi: x + 1 } }\n"
        b"    state empty() -> Range { Range { lo: 0, hi: 0 } }\n"
        b"}",
    )
    # A case's own `where` facts: construction proves the selected case's,
    # over the payload values it writes; a chosen arm assumes them over its
    # binders and the subject's fields; a store to a field one names is not
    # proved, since the active case is unknown there.
    accepted(
        program,
        "case-construction-proves-its-facts",
        b"data Size { case Some(v: u64) where v < 9; case None; }\n"
        b"machine make() -> Size { Size::Some { v: 1 } }",
    )
    rejected(
        program,
        "case-construction-breaks-its-facts",
        b"data Size { case Some(v: u64) where v < 9; case None; }\n"
        b"machine make() -> Size { Size::Some { v: 9 } }",
        "DomainUnproven",
        b"Size::Some { v: 9 }",
    )
    rejected(
        program,
        "case-construction-omits-a-payload",
        b"data Pair { case Both(a: u64, b: u64) where b > 0; case None; }\n"
        b"machine make() -> Pair { Pair::Both { a: 1 } }",
        "DomainUnproven",
        b"Pair::Both { a: 1 }",
    )
    accepted(
        program,
        "case-arm-assumes-its-facts",
        b"data Interval { case Empty; case Range(lo: u64, hi: u64) where lo <= hi; }\n"
        b"machine width(i: Interval) -> u64 {\n"
        b"    transition i {\n"
        b"        Interval::Range { lo, hi } -> (hi - lo)\n"
        b"        _ -> (0)\n"
        b"    }\n"
        b"}",
    )
    rejected(
        program,
        "case-arm-without-the-fact",
        b"data Interval { case Empty; case Range(lo: u64, hi: u64); }\n"
        b"machine width(i: Interval) -> u64 {\n"
        b"    transition i {\n"
        b"        Interval::Range { lo, hi } -> (hi - lo)\n"
        b"        _ -> (0)\n"
        b"    }\n"
        b"}",
        "ArithmeticOverflow",
        b"hi - lo",
    )
    accepted(
        program,
        "case-arm-assumes-common-field-facts",
        b"data Tally { n: u64; case Some(v: u64) where v < n; case None; }\n"
        b"machine room(t: &Tally) -> u64 {\n"
        b"    transition t {\n"
        b"        Tally::Some { v } -> (t.n - v)\n"
        b"        _ -> (0)\n"
        b"    }\n"
        b"}",
    )
    rejected(
        program,
        "case-value-owes-the-data-facts",
        b"data Flag\nwhere\n    x > 0,\n{ x: u64; case On; case Off; }\n"
        b"machine make() -> Flag { Flag::On }",
        "DomainUnproven",
        b"Flag::On",
    )
    rejected(
        program,
        "store-to-a-field-a-case-fact-names",
        b"data Tally { n: u64; m: u64; case Some(v: u64) where v < n; case None; }\n"
        b"machine reset(t: &mut Tally) { t.n = 0; }",
        "DomainUnproven",
        b"t.n = 0;",
    )
    accepted(
        program,
        "store-to-a-field-no-case-fact-names",
        b"data Tally { n: u64; m: u64; case Some(v: u64) where v < n; case None; }\n"
        b"machine reset(t: &mut Tally) { t.m = 0; }",
    )
    # A proposition application states what no zone fact expresses:
    # assumed, it establishes nothing; owed by a call, a return or a
    # construction, checking cannot decide it and rejects it as unsupported
    # rather than proving or refuting it.
    small = b"proposition Small(x: u64) = x < 4;\n"
    accepted(
        program,
        "proposition-requirement-is-assumed-harmlessly",
        small + b"machine f(x: u64)\nrequires Small(x)\n{}",
    )
    rejected(
        program,
        "proposition-requirement-establishes-nothing",
        small + b"machine read(a: &[u8; 4], x: u64) -> u8\nrequires Small(x)\n{ a[x] }",
        "IndexOutOfBounds",
        b"a[x]",
    )
    rejected(
        program,
        "owed-proposition-requirement-is-unsupported",
        small + b"machine take(x: u64)\nrequires Small(x)\n{}\nmachine call() { take(0); }",
        "UnsupportedFact",
        b"Small(x)",
    )
    rejected(
        program,
        "proposition-guarantee-is-unsupported",
        small + b"machine make() -> u64\nensures Small(result)\n{ 1 }",
        "UnsupportedFact",
        b"Small(result)",
    )
    rejected(
        program,
        "proposition-data-fact-is-unsupported",
        b"proposition Ordered(lo: u64, hi: u64) = lo <= hi;\n"
        b"data Range\nwhere\n    Ordered(lo, hi),\n{ lo: u64; hi: u64; }\n"
        b"machine make() -> Range { Range { lo: 1, hi: 2 } }",
        "UnsupportedFact",
        b"Ordered(lo, hi)",
    )
    rejected(
        program,
        "construction-breaks-the-domain",
        b"data Range\nwhere\n    lo <= hi,\n{ lo: u64; hi: u64; }\n"
        b"machine make() -> Range { Range { lo: 1 } }",
        "DomainUnproven",
        b"Range { lo: 1 }",
    )

    # Machines check independently: each pass (bodies, then crash routes)
    # records the first rejection of every machine it rejects, in order.
    every = (
        b"machine first(a: &[u8; 4], i: u64) -> u8 { a[i] }\n"
        b"machine fine(a: &[u8; 4]) -> u8 { a[0] }\n"
        b"pub machine leaks(x: u64 in Trapping) -> u64 in Trapping { x + 1 }\n"
        b"machine second(x: u64, y: u64) -> u64 { x / y }\n"
        b"machine twice(a: &[u8; 4], i: u64, x: u64) -> u64 { let b: u8 = a[i]; x / i }"
    )
    status, payload = run(program, every)
    observation = decode(payload)
    expected = (
        ("IndexOutOfBounds", located(every, b"a[i]")),
        ("DivisorMayBeZero", located(every, b"x / y")),
        ("IndexOutOfBounds", located(every, b"a[i]!2")),
        ("CrashNotDeclared", located(every, b"pub machine leaks(x: u64 in Trapping) -> u64 in Trapping { x + 1 }")),
    )
    assert status == 247 and observation.rejections == expected, (
        f"every-machine-first-rejection: status {status}, {observation.rejections}"
    )

    # Ownership (ownership.omg): moves, borrowed binders and linear exits.
    receipt = b"data Receipt [linear] { code: i32; }\nmachine settle(receipt: Receipt) -> i32;\n"
    rejected(
        program,
        "affine-reuse-after-call-move",
        b"data Settings { count: i32; }\n"
        b"machine Settings::observe(settings: Settings) -> i32 { settings.count }\n"
        b"machine inspect() -> i32 {\n"
        b"    let mut settings: Settings = Settings { count: 17 };\n"
        b"    let first: i32 = Settings::observe(settings);\n"
        b"    Settings::observe(settings)\n"
        b"}",
        "UseAfterMove",
        b"settings!5",
    )
    rejected(
        program,
        "read-through-consumed-owner",
        b"data Guard { handle: i32; }\n"
        b"machine consume(value: Guard) {}\n"
        b"machine run() -> i32 {\n"
        b"    let g: Guard = Guard { handle: 7 };\n"
        b"    consume(g);\n"
        b"    g.handle\n"
        b"}",
        "UseAfterMove",
        b"g.handle",
    )
    rejected(
        program,
        "reestablished-owner-moves-once",
        b"data Item { number: i32; }\n"
        b"machine take(item: Item) -> i32 { item.number }\n"
        b"machine run() -> i32 {\n"
        b"    let mut item: Item = Item { number: 17 };\n"
        b"    let first: i32 = take(item);\n"
        b"    item = Item { number: 42 };\n"
        b"    let second: i32 = take(item);\n"
        b"    take(item)\n"
        b"}",
        "UseAfterMove",
        b"item!7",
    )
    rejected(
        program,
        "computed-subject-moves-its-argument",
        b"data Choice { case Left(value: u64); case Right(value: u64); }\n"
        b"machine forward(choice: Choice) -> Choice { choice }\n"
        b"machine choose(choice: Choice) -> Choice {\n"
        b"    transition forward(choice) {\n"
        b"        Choice::Left { value } -> forward(choice)\n"
        b"        Choice::Right { value } -> forward(choice)\n"
        b"    }\n"
        b"}",
        "UseAfterMove",
        b"choice!5",
    )
    token = b"data Token { value: u64; }\nmachine consume(token: Token) -> u64 { token.value }\n"
    accepted(
        program,
        "transition-arms-move-separately",
        token + b"machine run(choice: bool) -> u64 {\n"
        b"    let token: Token = Token { value: 1 };\n"
        b"    transition choice {\n"
        b"        true -> consume(token)\n"
        b"        _ -> consume(token)\n"
        b"    }\n"
        b"}",
    )
    accepted(
        program,
        "match-arms-move-separately",
        token + b"machine run(choice: bool) -> u64 {\n"
        b"    let token: Token = Token { value: 1 };\n"
        b"    match choice { true -> consume(token), _ -> consume(token) }\n"
        b"}",
    )
    rejected(
        program,
        "match-arm-move-holds-after-the-match",
        token + b"machine run(choice: bool) -> u64 {\n"
        b"    let token: Token = Token { value: 1 };\n"
        b"    let first: u64 = match choice { true -> consume(token), _ -> 0 };\n"
        b"    consume(token)\n"
        b"}",
        "UseAfterMove",
        b"token!5",
    )
    accepted(
        program,
        "copy-data-reuses",
        b"data Point [copy] { x: u64; }\n"
        b"machine take(point: Point) -> u64 { point.x }\n"
        b"machine run() -> u64 {\n"
        b"    let point: Point = Point { x: 1 };\n"
        b"    let first: u64 = take(point);\n"
        b"    take(point)\n"
        b"}",
    )
    accepted(
        program,
        "shared-lend-keeps-the-owner",
        b"data Item { value: u64; }\n"
        b"machine look(item: &Item) -> u64 { item.value }\n"
        b"machine run() -> u64 {\n"
        b"    let item: Item = Item { value: 1 };\n"
        b"    let first: u64 = look(&item);\n"
        b"    look(&item)\n"
        b"}",
    )
    pair = token + b"data Pair { left: Token; right: Token; }\nmachine whole(pair: Pair) -> u64 { 0 }\n"
    accepted(
        program,
        "sibling-fields-move-apart",
        pair + b"machine run() -> u64 {\n"
        b"    let pair: Pair = Pair { left: Token { value: 1 }, right: Token { value: 2 } };\n"
        b"    let first: u64 = consume(pair.left);\n"
        b"    consume(pair.right)\n"
        b"}",
    )
    rejected(
        program,
        "whole-after-partial-move",
        pair + b"machine run() -> u64 {\n"
        b"    let pair: Pair = Pair { left: Token { value: 1 }, right: Token { value: 2 } };\n"
        b"    let first: u64 = consume(pair.left);\n"
        b"    whole(pair)\n"
        b"}",
        "UseAfterMove",
        b"pair!4",
    )
    rejected(
        program,
        "borrowed-binder-cannot-move",
        b"data Token { value: i32; }\n"
        b"data Choice { case Present(value: Token); case Empty; }\n"
        b"machine inspect(choice: &Choice) -> bool {\n"
        b"    transition choice {\n"
        b"        Choice::Present { value } -> observe(value)\n"
        b"        _ -> (false)\n"
        b"    }\n"
        b"    state observe(value: Token) -> bool { value.value == 42 }\n"
        b"}",
        "MoveOutOfBorrow",
        b"value!4",
    )
    rejected(
        program,
        "field-behind-reference-cannot-move",
        token + b"data Holder { token: Token; }\n"
        b"machine run(holder: &Holder) -> u64 { consume(holder.token) }",
        "MoveOutOfBorrow",
        b"holder.token",
    )
    rejected(
        program,
        "linear-local-reaches-its-exit",
        receipt + b"machine run() -> i32 {\n"
        b"    let receipt: Receipt = Receipt { code: 7 };\n"
        b"    let observed: i32 = receipt.code;\n"
        b"    0\n"
        b"}",
        "LinearUnconsumed",
        b"0",
    )
    rejected(
        program,
        "linear-live-overwrite",
        receipt + b"machine run() -> i32 {\n"
        b"    let mut receipt: Receipt = Receipt { code: 1 };\n"
        b"    receipt = Receipt { code: 2 };\n"
        b"    settle(receipt)\n"
        b"}",
        "LinearOverwritten",
        b"receipt!3",
    )
    accepted(
        program,
        "linear-moved-then-reassigned",
        receipt + b"machine run() -> i32 {\n"
        b"    let mut receipt: Receipt = Receipt { code: 1 };\n"
        b"    let first: i32 = settle(receipt);\n"
        b"    receipt = Receipt { code: 2 };\n"
        b"    settle(receipt)\n"
        b"}",
    )
    accepted(
        program,
        "linear-parameter-consumed",
        receipt + b"machine pass(receipt: Receipt) -> i32 { settle(receipt) }",
    )
    accepted(
        program,
        "jump-moves-linear-into-state",
        receipt + b"machine run() -> i32 {\n"
        b"    let receipt: Receipt = Receipt { code: 1 };\n"
        b"    transition { _ -> held(receipt) }\n"
        b"    state held(receipt: Receipt) -> i32 { settle(receipt) }\n"
        b"}",
    )
    rejected(
        program,
        "linear-arm-exit-unconsumed",
        receipt + b"machine run(ready: bool) -> i32 {\n"
        b"    let receipt: Receipt = Receipt { code: 1 };\n"
        b"    transition ready {\n"
        b"        true -> settle(receipt)\n"
        b"        _ -> 0\n"
        b"    }\n"
        b"}",
        "LinearUnconsumed",
        b"_ -> 0",
    )
    rejected(
        program,
        "linear-statement-value-discarded",
        b"data Receipt [linear] { code: i32; }\nmachine issue() -> Receipt;\n"
        b"machine run() {\n"
        b"    _ = issue();\n"
        b"}",
        "LinearUnconsumed",
        b"issue()!2",
    )
    rejected(
        program,
        "linear-temporary-projected",
        b"data Receipt [linear] { code: i32; }\n"
        b"machine run() -> i32 {\n"
        b"    (Receipt { code: 7 }).code\n"
        b"}",
        "LinearUnconsumed",
        b"Receipt { code: 7 }",
    )
    rejected(
        program,
        "linear-moved-on-one-operand-path",
        b"data Receipt [linear] { code: i32; }\nmachine settle(receipt: Receipt) -> bool;\n"
        b"machine run(ready: bool) -> bool {\n"
        b"    let receipt: Receipt = Receipt { code: 1 };\n"
        b"    ready && settle(receipt)\n"
        b"}",
        "LinearUnconsumed",
        b"receipt!3",
    )

    # Holds (holds.omg): a call's arguments against earlier arguments' loans.
    put = b"data Main { value: i32; other: i32; }\nmachine put(target: &mut i32, value: i32) -> i32 { value }\n"
    rejected(
        program,
        "exclusive-loan-then-read",
        put + b"machine run(main: &mut Main) -> i32 { put(&mut main.value, main.value) }",
        "OverlappingArguments",
        b"main.value!2",
    )
    accepted(
        program,
        "read-completes-before-a-loan",
        b"data Main { value: i32; }\nmachine put(value: i32, target: &mut i32) -> i32 { value }\n"
        b"machine run(main: &mut Main) -> i32 { put(main.value, &mut main.value) }",
    )
    accepted(
        program,
        "disjoint-fields-lend-apart",
        put + b"machine run(main: &mut Main) -> i32 { put(&mut main.value, main.other) }",
    )
    accepted(
        program,
        "distinct-literal-indexes-lend-apart",
        b"machine put(target: &mut u64, value: u64) -> u64 { value }\n"
        b"machine run(items: &mut [u64; 4]) -> u64 { put(&mut items[0], items[1]) }",
    )
    rejected(
        program,
        "local-lent-exclusively-twice",
        b"data Score { points: i32; }\n"
        b"machine join(left: &mut Score, right: &mut Score) {}\n"
        b"machine run() {\n"
        b"    let mut score: Score = Score { points: 1 };\n"
        b"    join(&mut score, &mut score);\n"
        b"}",
        "OverlappingArguments",
        b"score!3",
    )
    hits = b"data Main { count: u64; }\nmachine peek(main: &Main) -> u64 { main.count }\n"
    rejected(
        program,
        "exclusive-reference-passed-twice",
        hits + b"machine take(first: &mut Main, second: &mut Main) -> u64 { 0 }\n"
        b"machine run(main: &mut Main) -> u64 { take(main, main) }",
        "OverlappingArguments",
        b"main!5",
    )
    rejected(
        program,
        "nested-call-reads-a-held-place",
        hits + b"machine take(main: &mut Main, value: u64) -> u64 { value }\n"
        b"machine run(main: &mut Main) -> u64 { take(main, peek(main)) }",
        "OverlappingArguments",
        b"main!6",
    )
    accepted(
        program,
        "shared-references-share",
        hits + b"machine both(first: &Main, second: &Main) -> u64 { first.count }\n"
        b"machine run(main: &Main) -> u64 { both(main, main) }",
    )
    accepted(
        program,
        "jump-forwards-its-data-parent",
        b"data Main { count: u64; }\n"
        b"machine Main::run(main: &mut Main) -> u64 {\n"
        b"    transition { _ -> next(main, main.count) }\n"
        b"    state next(held: &mut Main, value: u64) -> u64 { value }\n"
        b"}",
    )
    rejected(
        program,
        "jump-forwards-its-data-parent-twice",
        b"data Main { count: u64; }\n"
        b"machine Main::run(main: &mut Main) -> u64 {\n"
        b"    transition { _ -> next(main, main) }\n"
        b"    state next(first: &mut Main, second: &mut Main) -> u64 { first.count }\n"
        b"}",
        "OverlappingArguments",
        b"main!3",
    )
    rejected(
        program,
        "jump-fresh-loan-then-read",
        b"data Inner { value: u64; }\ndata Main { inner: Inner; }\n"
        b"machine Main::run(main: &mut Main) -> u64 {\n"
        b"    transition { _ -> next(&mut main.inner, main.inner.value) }\n"
        b"    state next(held: &mut Inner, value: u64) -> u64 { value }\n"
        b"}",
        "OverlappingArguments",
        b"main.inner.value",
    )

    # A nested call whose result holds references keeps its operands' loans
    # for the outer call; one with a reference-free result has released them.
    rejected(
        program,
        "nested-result-keeps-its-operand-loan",
        b"data Held { value: &mut u64; }\n"
        b"machine hold(target: &mut u64) -> Held { Held { value: target } }\n"
        b"machine bump(target: &mut u64) -> u64 { 0 }\n"
        b"machine observe(held: Held, changed: u64) -> u64 { changed }\n"
        b"machine run(count: &mut u64) -> u64 { observe(hold(count), bump(count)) }",
        "OverlappingArguments",
        b"count!3",
    )
    accepted(
        program,
        "completed-nested-call-releases-its-operand",
        b"machine bump(target: &mut u64) -> u64 { 0 }\n"
        b"machine observe(first: u64, second: u64) -> u64 { first }\n"
        b"machine run(count: &mut u64) -> u64 { observe(bump(count), bump(count)) }",
    )

    # Remainders and policy results of constants (evaluation.omg, arithmetic.omg).
    accepted(
        program,
        "signed-remainder-of-constants-folds",
        b"machine f() -> i32\nensures result == -1\n{ -3i32 % 2 }",
    )
    accepted(
        program,
        "signed-dividend-remainder-stays-below-its-divisor",
        b"machine f(x: i32) -> i32\nensures result < 8 && result > -8\n{ x % 8 }",
    )
    accepted(
        program,
        "nonpositive-dividend-remainder-is-nonpositive",
        b"machine f(x: i32) -> i32\nrequires x <= 0\nensures result <= 0\n{ x % 5 }",
    )
    accepted(
        program,
        "policy-results-of-constants-fold",
        b"machine wrapped_sum() -> u8\nensures result == 1\n{ ((255u8 as u8 in Wrapping) + 2) as u8 }\n"
        b"machine saturated_sum() -> u8\nensures result == 255\n{ ((255u8 as u8 in Saturating) + 2) as u8 }\n"
        b"machine wrapped_operand() -> u8\nensures result == 127\n{ (((0u8 as u8 in Wrapping) - 2) >> 1) as u8 }\n"
        b"machine wrapped_signed() -> i8\nensures result == -128\n{ ((127i8 as i8 in Wrapping) + 1) as i8 }",
    )
    rejected(
        program,
        "wrapped-constant-keeps-its-value",
        b"machine f() -> u8\nensures result == 2\n{ ((255u8 as u8 in Wrapping) + 2) as u8 }",
        "GuaranteeUnproven",
        b"((255u8 as u8 in Wrapping) + 2) as u8",
    )

    # A reference read for its scalar holds nothing beside the call's other
    # arguments, but reading a place an earlier argument lends exclusively
    # still overlaps it.
    accepted(
        program,
        "reference-read-wraps-its-referent",
        b"machine bump(count: &mut u64) { count = ((count as u64 in Wrapping) + 1) as u64; }",
    )
    accepted(
        program,
        "read-before-an-exclusive-loan",
        b"machine take(a: u64, b: &mut u64) -> u64 { a }\n"
        b"machine run(count: &mut u64) -> u64 { take(count, count) }",
    )
    rejected(
        program,
        "read-after-an-exclusive-loan",
        b"machine take(b: &mut u64, a: u64) -> u64 { a }\n"
        b"machine run(count: &mut u64) -> u64 { take(count, count) }",
        "OverlappingArguments",
        b"count!3",
    )

    # Loans held by locals (loans.omg): a loan lasts until its holder's last
    # use, and an exit may not return a loan of the body's own storage.
    storage = (b"data Storage { value: i32; }\n"
               b"machine use_value(value: &mut i32) { value = 1; }\n"
               b"machine write_alias(value: &mut i32) { value = 2; }\n")
    rejected(
        program,
        "exclusive-loan-live-until-its-last-use",
        storage + b"machine run() {\n"
        b"    let mut storage: Storage = Storage { value: 0 };\n"
        b"    let alias: &mut i32 = &mut storage.value;\n"
        b"    alias = 3;\n"
        b"    use_value(&mut storage.value);\n"
        b"    write_alias(alias);\n"
        b"}",
        "LoanConflict",
        b"storage.value!2",
    )
    accepted(
        program,
        "loan-ends-at-its-last-use",
        storage + b"machine run() {\n"
        b"    let mut storage: Storage = Storage { value: 0 };\n"
        b"    let alias: &mut i32 = &mut storage.value;\n"
        b"    alias = 3;\n"
        b"    use_value(&mut storage.value);\n"
        b"}",
    )
    score = (b"data Score { points: i32; }\n"
             b"machine take(score: Score) {}\n"
             b"machine read(score: &Score) -> i32 { score.points }\n")
    rejected(
        program,
        "move-under-a-shared-loan",
        score + b"machine run() -> i32 {\n"
        b"    let score: Score = Score { points: 1 };\n"
        b"    let alias: &Score = &score;\n"
        b"    take(score);\n"
        b"    read(alias)\n"
        b"}",
        "LoanConflict",
        b"score!6",
    )
    accepted(
        program,
        "shared-loan-admits-reads",
        score + b"machine run() -> i32 {\n"
        b"    let score: Score = Score { points: 1 };\n"
        b"    let alias: &Score = &score;\n"
        b"    let direct: i32 = score.points;\n"
        b"    let seen: i32 = read(alias);\n"
        b"    direct\n"
        b"}",
    )
    rejected(
        program,
        "call-result-holds-its-argument-loan",
        b"data Cell { value: i32; }\n"
        b"data Tally { first: Cell; hits: i32; }\n"
        b"machine pick(tally: &mut Tally) -> &mut Cell { &mut tally.first }\n"
        b"machine run() {\n"
        b"    let mut tally: Tally = Tally { first: Cell { value: 0 }, hits: 0 };\n"
        b"    let cell: &mut Cell = pick(&mut tally);\n"
        b"    tally.hits = 5;\n"
        b"    let observed: i32 = cell.value;\n"
        b"}",
        "LoanConflict",
        b"tally.hits",
    )
    rejected(
        program,
        "stored-reference-holds-its-loan",
        b"data Cell { value: i32; }\n"
        b"data Message { body: &Cell; }\n"
        b"machine read_cell(cell: &Cell) -> i32 { cell.value }\n"
        b"machine exercise(cell: &mut Cell) -> i32 {\n"
        b"    let message: Message = Message { body: cell };\n"
        b"    cell.value = 71;\n"
        b"    read_cell(message.body)\n"
        b"}",
        "LoanConflict",
        b"cell.value!2",
    )
    rejected(
        program,
        "local-loan-escapes-its-body",
        b"data Score { points: i32; }\n"
        b"machine leak() -> &Score {\n"
        b"    let score: Score = Score { points: 1 };\n"
        b"    &score\n"
        b"}",
        "LoanEscapes",
        b"&score",
    )
    accepted(
        program,
        "borrowed-view-returns",
        b"data Score { points: i32; }\n"
        b"machine view(score: &Score) -> &i32 { &score.points }",
    )

    # Boolean aliases (aliases.omg): a Boolean bound from a condition stands
    # for it at a dispatch or guard until an older place is written.
    accepted(
        program,
        "boolean-local-stands-for-its-condition",
        b"machine shift(value: u64, count: u32) -> u64 {\n"
        b"    let valid: bool = count < 64u32;\n"
        b"    transition valid {\n"
        b"        true -> calculate(value, count)\n"
        b"        false -> empty()\n"
        b"    }\n"
        b"    state calculate(value: u64, count: u32) -> u64 { value >> count }\n"
        b"    state empty() -> u64 { 0 }\n"
        b"}",
    )
    accepted(
        program,
        "boolean-field-guard-stands-for-its-condition",
        b"data Guard { count: u32; valid: bool; }\n"
        b"machine shift(count: u32) -> u64 {\n"
        b"    let mut guard: Guard = Guard { count: 0, valid: false };\n"
        b"    guard.count = count;\n"
        b"    guard.valid = guard.count < 64u32;\n"
        b"    transition {\n"
        b"        guard.valid -> (8u64 >> guard.count)\n"
        b"        _ -> 0u64\n"
        b"    }\n"
        b"}",
    )
    rejected(
        program,
        "a-write-ends-the-alias",
        b"machine shift(value: u64, count: u32, other: u32) -> u64 {\n"
        b"    let mut limit: u32 = count;\n"
        b"    let valid: bool = limit < 64u32;\n"
        b"    limit = other;\n"
        b"    transition valid {\n"
        b"        true -> (value >> limit)\n"
        b"        false -> 0\n"
        b"    }\n"
        b"}",
        "ShiftCountOutOfRange",
        b"value >> limit",
    )

    # A routed domain's route establishes its own result (qualification.omg).
    ticket = b"data Ticket [copy] { amount: i32; }\ndomain Ticket::Issued established by Ticket::issue;\n"
    accepted(
        program,
        "route-establishes-its-own-result",
        ticket + b"machine Ticket::issue() -> Ticket\nensures result in Ticket::Issued\n{ Ticket { amount: 41 } }",
    )
    accepted(
        program,
        "route-mints-its-qualified-result",
        ticket + b"machine Ticket::issue() -> Ticket in Issued { Ticket { amount: 41 } }",
    )
    rejected(
        program,
        "only-a-route-establishes-a-routed-domain",
        ticket + b"machine Ticket::issue() -> Ticket { Ticket { amount: 41 } }\n"
        b"machine Ticket::forge() -> Ticket\nensures result in Ticket::Issued\n{ Ticket { amount: 41 } }",
        "GuaranteeUnproven",
        b"Ticket { amount: 41 }!2",
    )

    accepted(
        program,
        "a-place-equals-itself",
        b"machine identity(value: bool) -> bool\nensures result == value\n{ value }",
    )

    accepted(
        program,
        "record-copy-keeps-its-fields",
        b"data Player [copy] { health: i32; }\n"
        b"domain Player::Alive(player)\nrequires player.health > 0;\n"
        b"machine inspect(player: Player)\nrequires player in Player::Alive\n{}\n"
        b"machine run(player: Player)\nrequires player in Player::Alive\n{\n"
        b"    let copy: Player = player;\n"
        b"    inspect(copy);\n"
        b"}",
    )

    # An operand evaluated before a later call writes what it read keeps
    # the value it captured (snapshots.omg).
    rewrite = b"machine set(x: &mut u64) -> bool { x = 5; true }\n"
    rejected(
        program,
        "later-write-leaves-the-earlier-read",
        rewrite + b"machine compare(before: u64, done: bool, after: u64)\nrequires before == after\n{}\n"
        b"machine run() {\n    let mut x: u64 = 1;\n    compare(x, set(&mut x), x);\n}",
        "RequirementUnproven",
        b"compare(x, set(&mut x), x)",
    )
    accepted(
        program,
        "earlier-read-keeps-its-value",
        rewrite + b"machine compare(before: u64, done: bool)\nrequires before == 1\n{}\n"
        b"machine run() {\n    let mut x: u64 = 1;\n    compare(x, set(&mut x));\n}",
    )
    bump = b"machine bump(x: &mut u64) -> u64\nensures result == 0\n{ x = 7; 0 }\n"
    accepted(
        program,
        "left-operand-keeps-its-value",
        bump + b"machine run() -> u64\nensures result == 1\n{\n    let mut x: u64 = 1;\n    x + bump(&mut x)\n}",
    )
    rejected(
        program,
        "right-operand-reads-the-write",
        bump + b"machine run() -> u64\nensures result == 1\n{\n    let mut x: u64 = 1;\n    bump(&mut x) + x\n}",
        "GuaranteeUnproven",
        b"bump(&mut x) + x",
    )
    accepted(
        program,
        "record-operand-keeps-its-fields",
        b"data Pair [copy] { first: u64; second: u64; }\n"
        b"machine reset(pair: &mut Pair) -> bool { pair.first = 9; true }\n"
        b"machine same(left: Pair, done: bool)\nrequires left.first == 1\n{}\n"
        b"machine run() {\n    let mut pair: Pair = Pair { first: 0, second: 0 };\n    pair.first = 1;\n"
        b"    same(pair, reset(&mut pair));\n}",
    )

    # A construction's fields hold the values it wrote, read through its
    # value, a copy of it or a state's parameter (constructions.omg).
    pair = (b"data Pair [copy] { first: u64; second: u64; replaced: bool; }\n"
            b"machine same(pair: Pair)\nrequires pair.first == pair.second\n{}\n")
    holder = (b"data Holder { amount: u64; }\n"
              b"machine replace(holder: &mut Holder) -> bool { holder.amount = 9; true }\n")
    accepted(
        program,
        "constructed-argument-keeps-its-fields",
        pair + b"machine run(value: u64) { same(Pair { first: value, second: value, replaced: false }); }",
    )
    accepted(
        program,
        "constructed-local-keeps-its-fields",
        pair + b"machine run(value: u64) {\n    let saved: Pair = Pair { first: value, second: value, replaced: false };\n"
        b"    same(saved);\n}",
    )
    rejected(
        program,
        "constructed-fields-differ",
        pair + b"machine run(value: u64) { same(Pair { first: value, second: 0, replaced: false }); }",
        "RequirementUnproven",
        b"same(Pair { first: value, second: 0, replaced: false })",
    )
    accepted(
        program,
        "construction-captures-before-a-later-field",
        pair + holder + b"machine run(start: u64) {\n    let mut holder: Holder = Holder { amount: start };\n"
        b"    same(Pair { first: holder.amount, second: holder.amount, replaced: replace(&mut holder) });\n}",
    )
    rejected(
        program,
        "field-after-a-write-reads-the-write",
        pair + holder + b"machine run(start: u64) {\n    let mut holder: Holder = Holder { amount: start };\n"
        b"    same(Pair { first: holder.amount, replaced: replace(&mut holder), second: holder.amount });\n}",
        "RequirementUnproven",
        b"same(Pair { first: holder.amount, replaced: replace(&mut holder), second: holder.amount })",
    )
    accepted(
        program,
        "stored-construction-outlives-a-later-write",
        pair + holder + b"machine run(start: u64) {\n    let mut holder: Holder = Holder { amount: start };\n"
        b"    let saved: Pair = Pair { first: holder.amount, second: holder.amount, replaced: false };\n"
        b"    _ = replace(&mut holder);\n    same(saved);\n}",
    )
    accepted(
        program,
        "jump-carries-a-construction",
        b"data Pair [copy] { first: u64; second: u64; }\nmachine run(value: u64) {\n"
        b"    transition { _ -> next(Pair { first: value, second: value }) }\n"
        b"    state next(pair: Pair)\n    requires pair.first == pair.second\n    {}\n}",
    )

    # A Boolean is a subject, 1 when true and 0 when false: a stored literal
    # proves a later fact naming the place, and a jump carries it.
    accepted(
        program,
        "stored-true-proves-its-field",
        b"data Token { value: u64; ready: bool; }\nmachine prepare(token: &mut Token)\n"
        b"ensures token.value == 9 && token.ready\n{\n    token.value = 9;\n    token.ready = true;\n}",
    )
    accepted(
        program,
        "assigned-true-proves-its-local",
        b"machine ready() -> bool\nensures result\n{\n    let mut flag: bool = false;\n    flag = true;\n    flag\n}",
    )
    rejected(
        program,
        "assigned-false-fails-its-local",
        b"machine ready() -> bool\nensures result\n{\n    let mut flag: bool = true;\n    flag = false;\n    flag\n}",
        "GuaranteeUnproven",
        b"flag!3",
    )
    pick = b"machine pick(flag: bool) -> u64\nrequires flag\n{ 1 }\n"
    accepted(
        program,
        "jump-carries-a-boolean",
        pick + b"machine run() -> u64 {\n    transition { _ -> next(5, true) }\n"
        b"    state next(count: u64, flag: bool) -> u64\n    requires count == 5\n    { pick(flag) }\n}",
    )
    rejected(
        program,
        "jump-carries-a-false-boolean",
        pick + b"machine run() -> u64 {\n    transition { _ -> next(5, false) }\n"
        b"    state next(count: u64, flag: bool) -> u64 { pick(flag) }\n}",
        "RequirementUnproven",
        b"pick(flag)",
    )

    # Captured reads survive later effects; aliases may only replay the
    # values they originally observed. These false obligations once passed.
    rejected(
        program,
        'composite-guard-keeps-captured-comparison',
        b'data Token [copy] { value: u64; }\n'
        b'machine mutate(token: &mut Token) -> bool { token.value = 9; true }\n'
        b'machine run() -> u64 ensures result == 1 {\n'
        b'let mut token: Token = Token { value: 1 };\n'
        b'transition { token.value == 1 && mutate(&mut token) -> token.value _ -> 1 }\n'
        b'}',
        'GuaranteeUnproven',
        b'token.value == 1 && mutate(&mut token) -> token.value',
    )
    rejected(
        program,
        'stored-condition-must-not-replay-mutated-read',
        b'data Token [copy] { value: u64; }\n'
        b'machine mutate(token: &mut Token) -> bool { token.value = 9; true }\n'
        b'machine run() -> u64 ensures result == 1 {\n'
        b'let mut token: Token = Token { value: 1 };\n'
        b'let valid: bool = token.value == 1 && mutate(&mut token);\n'
        b'transition valid { true -> token.value false -> 1 }\n'
        b'}',
        'GuaranteeUnproven',
        b'true -> token.value',
    )
    rejected(
        program,
        'nested-borrow-write-invalidates-original',
        b'data Token [copy] { value: u64; }\n'
        b'data Outer { inner: Token; }\n'
        b'machine run() -> u64 ensures result == 1 {\n'
        b'let mut outer: Outer = Outer { inner: Token { value: 0 } };\n'
        b'outer.inner.value = 1;\n'
        b'let alias: &mut Token = &mut outer.inner;\n'
        b'alias.value = 9;\n'
        b'outer.inner.value\n'
        b'}',
        'GuaranteeUnproven',
        b'outer.inner.value!2',
    )
    rejected(
        program,
        'construction-expression-reused-on-backedge',
        b'data Token [copy] { value: u64; }\n'
        b'machine consume(token: Token) -> u64 requires token.value == 1 { token.value }\n'
        b'machine run() -> u64 {\n'
        b'transition { _ -> again(Token { value: 1 }, 0) }\n'
        b'state again(token: Token, step: u64) -> u64 {\n'
        b'let saved: Token = Token { value: token.value };\n'
        b'transition { step == 0 -> again(Token { value: 9 }, 1) _ -> consume(saved) }\n'
        b'}\n'
        b'}',
        'RequirementUnproven',
        b'consume(saved)',
    )
    rejected(
        program,
        'composite-transition-subject-keeps-captured-comparison',
        b'data Token [copy] { value: u64; }\n'
        b'machine mutate(token: &mut Token) -> bool { token.value = 9; true }\n'
        b'machine run() -> u64 ensures result == 1 {\n'
        b'let mut token: Token = Token { value: 1 };\n'
        b'transition token.value == 1 && mutate(&mut token) { true -> token.value false -> 1 }\n'
        b'}',
        'GuaranteeUnproven',
        b'true -> token.value',
    )
    rejected(
        program,
        'self-assignment-must-not-create-replay-alias',
        b'machine run() -> u64 ensures result == 1 {\n'
        b'let mut flag: bool = false;\n'
        b'flag = !flag;\n'
        b'transition flag { true -> 9 false -> 1 }\n'
        b'}',
        'GuaranteeUnproven',
        b'true -> 9',
    )
    accepted(
        program,
        'pure-comparison-alias-still-proves',
        b'machine run(value: u64) -> u64 ensures result == 1 {\n'
        b'let valid: bool = value == 1;\n'
        b'transition valid { true -> value false -> 1 }\n'
        b'}',
    )
    accepted(
        program,
        'boolean-argument-capture',
        b'data Token [copy] { ready: bool; value: u64; }\n'
        b'machine reset(token: &mut Token) -> bool { token.ready = false; true }\n'
        b'machine consume(before: bool, done: bool) requires before {}\n'
        b'machine run() { let mut token: Token = Token { ready: true, value: 0 }; consume(token.ready, reset(&mut token)); }',
    )
    rejected(
        program,
        'boolean-live-write-invalidates',
        b'data Token [copy] { ready: bool; value: u64; }\n'
        b'machine reset(token: &mut Token) -> bool { token.ready = false; true }\n'
        b'machine consume(now: bool) requires now {}\n'
        b'machine run() { let mut token: Token = Token { ready: true, value: 0 }; _ = reset(&mut token); consume(token.ready); }',
        'RequirementUnproven',
        b'consume(token.ready)',
    )
    accepted(
        program,
        'boolean-unwritten-field-survives',
        b'data Token [copy] { ready: bool; value: u64; }\n'
        b'machine write(token: &mut Token) { token.value = 9; }\n'
        b'machine consume(now: bool) requires now {}\n'
        b'machine run() { let mut token: Token = Token { ready: true, value: 0 }; write(&mut token); consume(token.ready); }',
    )
    rejected(
        program,
        'pending-written-field-loses-old-value',
        b'data Token [copy] { ready: bool; value: u64; }\n'
        b'machine write(token: &mut Token) { token.value = 9; }\n'
        b'machine consume(value: u64) requires value == 0 {}\n'
        b'machine run() { let mut token: Token = Token { ready: true, value: 0 }; write(&mut token); consume(token.value); }',
        'RequirementUnproven',
        b'consume(token.value)',
    )
    rejected(
        program,
        'boolean-join-keeps-both-arrivals',
        b'machine consume(flag: bool) requires flag {}\n'
        b'machine run(choice: bool) { transition choice { true -> next(true) false -> next(false) } state next(flag: bool) { consume(flag); } }',
        'RequirementUnproven',
        b'consume(flag)',
    )

    accepted(
        program,
        "scalar-reference-store-proves-referent",
        b"machine set(value: &mut u64) ensures value == 9 { value = 9; }",
    )

    accepted(
        program,
        'scalar-reference-argument-capture',
        b'machine mutate(value: &mut u64) -> u64 ensures result == 0 && value == 9 { value = 9; 0 }\n'
        b'machine consume(before: u64, done: u64) requires before == 1 {}\n'
        b'machine run() { let mut value: u64 = 1; let alias: &mut u64 = &mut value; consume(alias, mutate(alias)); }',
    )
    accepted(
        program,
        'scalar-reference-operator-capture',
        b'machine mutate(value: &mut u64) -> u64 ensures result == 0 && value == 9 { value = 9; 0 }\n'
        b'machine run() -> u64 ensures result == 1 { let mut value: u64 = 1; let alias: &mut u64 = &mut value; alias + mutate(alias) }',
    )
    rejected(
        program,
        'scalar-reference-cannot-read-the-later-write',
        b'machine mutate(value: &mut u64) -> u64 ensures result == 0 && value == 9 { value = 9; 0 }\n'
        b'machine run() -> u64 ensures result == 9 { let mut value: u64 = 1; let alias: &mut u64 = &mut value; alias + mutate(alias) }',
        "GuaranteeUnproven",
        b"alias + mutate(alias)",
    )

    # A store captures its destination before it evaluates the value
    # (wiki/spec/language/expressions.md#evaluation-schedule): a selector's
    # write is what the value then reads.
    rejected(
        program,
        'selector-writes-before-the-value-reads',
        b'data Token { value: u64; }\n'
        b'machine mutate(token: &mut Token) -> u64 ensures result == 0 && token.value == 9 { token.value = 9; 0 }\n'
        b'domain u64::One(value) requires value == 1;\n'
        b'machine run() { let mut items: [u64 in One; 1] = [1]; let mut token: Token = Token { value: 1 }; items[mutate(&mut token)] = token.value; }',
        "DomainUnproven",
        b"items[mutate(&mut token)] = token.value;",
    )
    accepted(
        program,
        'value-reads-the-selector-write',
        b'data Token { value: u64; }\n'
        b'machine mutate(token: &mut Token) -> u64 ensures result == 0 && token.value == 9 { token.value = 9; 0 }\n'
        b'domain u64::Nine(value) requires value == 9;\n'
        b'machine run() { let mut items: [u64 in Nine; 1] = [9]; let mut token: Token = Token { value: 1 }; items[mutate(&mut token)] = token.value; }',
    )

    # Borrowed locals keep the captured storage identity through reborrows
    # and binding replacement. Unrepresented write origins fail closed.
    borrowed = (b"data Token { value: u64; }\n"
                b"machine run() -> u64 ensures result == ")
    body = (b" { let mut token: Token = Token { value: 1 }; "
            b"let first: &mut Token = &mut token; "
            b"let second: &mut Token = &mut first; second.value = 9; token.value }")
    accepted(program, "transitive-reference-write", borrowed + b"9" + body)
    rejected(program, "transitive-reference-old-guarantee", borrowed + b"1" + body,
             "GuaranteeUnproven", b"token.value")
    # A reference returned from an exclusive argument of known storage is a
    # view of that storage (wiki/spec/language/lifetimes.md#returned-views).
    accepted(
        program,
        "a-reference-returned-from-a-placed-argument-is-written-through",
        b"data Token { value: u64; }\n"
        b"machine relay(token: &mut Token) -> &mut Token { token }\n"
        b"machine run() -> u64 { let mut token: Token = Token { value: 1 }; "
        b"let alias: &mut Token = relay(&mut token); alias.value = 9; token.value }",
    )
    # A reference field stands for its referent (references.omg).
    accepted(
        program,
        "a-reference-field-write-reaches-its-referent",
        b"data Token { value: u64; }\n"
        b"data Holder { token: &mut Token; }\n"
        b"machine change(holder: &mut Holder) { holder.token.value = 9; }",
    )
    accepted(
        program,
        "shared-reference-rebinding-keeps-earlier-alias",
        b"machine run() -> u64 ensures result == 1 { "
        b"let first: u64 = 1; let second: u64 = 9; let mut current: &u64 = &first; "
        b"let captured: &u64 = current; current = &second; captured }",
    )
    accepted(
        program,
        "reference-rebinding-releases-old-loan",
        b"machine run() -> u64 ensures result == 9 { "
        b"let mut first: u64 = 1; let second: u64 = 9; let mut current: &u64 = &first; "
        b"current = &second; first = 2; current }",
    )
    rejected(
        program,
        "reference-rebinding-retains-captured-old-loan",
        b"machine run() -> u64 { "
        b"let mut first: u64 = 1; let second: u64 = 9; let mut current: &u64 = &first; "
        b"let captured: &u64 = current; current = &second; first = 2; captured }",
        "LoanConflict",
        b"first!3",
    )
    rejected(
        program,
        "reference-rebinding-holds-new-loan",
        b"machine run() -> u64 { "
        b"let first: u64 = 1; let mut second: u64 = 9; let mut current: &u64 = &first; "
        b"current = &second; second = 10; current }",
        "LoanConflict",
        b"second!3",
    )
    rejected(
        program,
        "reference-self-assignment-retains-loan",
        b"machine run() -> u64 { "
        b"let mut first: u64 = 1; let mut current: &u64 = &first; "
        b"current = current; first = 2; current }",
        "LoanConflict",
        b"first!3",
    )
    rejected(
        program,
        "copied-reference-retains-owned-escape",
        b"machine run() -> &u64 { let first: u64 = 1; let second: u64 = 9; "
        b"let mut current: &u64 = &first; let captured: &u64 = current; "
        b"current = &second; captured }",
        "LoanEscapes",
        b"captured!2",
    )
    rejected(
        program,
        "reference-carrier-self-assignment-retains-loan",
        b"data Holder [copy] { value: &u64; }\n"
        b"machine run() -> u64 { let mut first: u64 = 1; "
        b"let mut holder: Holder = Holder { value: &first }; "
        b"holder = holder; first = 2; holder.value }",
        "LoanConflict",
        b"first!3",
    )
    rejected(
        program,
        "reference-carrier-assignment-captures-source-loans",
        b"data Holder [copy] { value: &u64; }\n"
        b"machine run() -> u64 { let mut first: u64 = 1; let second: u64 = 9; "
        b"let source: Holder = Holder { value: &first }; "
        b"let mut holder: Holder = Holder { value: &second }; "
        b"holder = source; first = 2; holder.value }",
        "LoanConflict",
        b"first!3",
    )
    accepted(
        program,
        "reference-carrier-loan-ends-after-copied-value-use",
        b"data Holder [copy] { value: &u64; }\n"
        b"machine run() -> u64 { let mut first: u64 = 1; let second: u64 = 9; "
        b"let mut holder: Holder = Holder { value: &first }; "
        b"let captured: Holder = holder; holder = Holder { value: &second }; "
        b"let observed: u64 = captured.value; first = 2; holder.value }",
    )
    rejected(
        program,
        "conditional-reference-carrier-retains-either-source-loan",
        b"data Holder [copy] { value: &u64; }\n"
        b"machine run(choice: bool) -> u64 { let mut first: u64 = 1; let second: u64 = 9; "
        b"let source: Holder = Holder { value: &first }; "
        b"let mut holder: Holder = Holder { value: &second }; "
        b"holder = match choice { true -> source, false -> holder }; first = 2; holder.value }",
        "LoanConflict",
        b"first!3",
    )
    rejected(
        program,
        "reference-carrier-cannot-escape-owned-backing",
        b"data Holder [copy] { value: &u64; }\n"
        b"machine run() -> Holder { let first: u64 = 1; "
        b"let holder: Holder = Holder { value: &first }; holder }",
        "LoanEscapes",
        b"holder!2",
    )
    rejected(
        program,
        "copied-reference-carrier-cannot-escape-owned-backing",
        b"data Holder [copy] { value: &u64; }\n"
        b"machine run() -> Holder { let first: u64 = 1; "
        b"let holder: Holder = Holder { value: &first }; let copied: Holder = holder; copied }",
        "LoanEscapes",
        b"copied!2",
    )
    accepted(
        program,
        "projected-and-copied-reference-carrier-can-return-parameter-loan",
        b"data Holder [copy] { value: &u64; }\n"
        b"data Outer { inner: Holder; }\n"
        b"machine run(value: &u64) -> Holder { "
        b"let outer: Outer = Outer { inner: Holder { value: value } }; "
        b"let holder: Holder = outer.inner; let copied: Holder = holder; copied }",
    )
    rejected(
        program,
        "projected-reference-carrier-retains-backing-loan",
        b"data Holder [copy] { value: &u64; }\n"
        b"data Outer { inner: Holder; }\n"
        b"machine run() -> u64 { let mut first: u64 = 1; let second: u64 = 9; "
        b"let source: Outer = Outer { inner: Holder { value: &first } }; "
        b"let mut holder: Holder = Holder { value: &second }; "
        b"holder = source.inner; first = 2; holder.value }",
        "LoanConflict",
        b"first!3",
    )
    rejected(
        program,
        "projected-reference-carrier-cannot-escape-owned-backing",
        b"data Holder [copy] { value: &u64; }\n"
        b"data Outer { inner: Holder; }\n"
        b"machine run() -> Holder { let first: u64 = 1; "
        b"let outer: Outer = Outer { inner: Holder { value: &first } }; outer.inner }",
        "LoanEscapes",
        b"outer.inner",
    )

    accepted(
        program,
        "reference-rebinding-preserves-new-referent-facts",
        b"machine run() -> u64 ensures result == 9 { "
        b"let first: u64 = 1; let second: u64 = 9; let mut current: &u64 = &first; "
        b"let valid: bool = current == 1; current = &second; "
        b"transition valid { true -> current false -> 9 } }",
    )
    rejected(
        program,
        "reference-rebinding-retires-boolean-replay",
        b"machine run() -> u64 ensures result == 1 { "
        b"let first: u64 = 1; let second: u64 = 9; let mut current: &u64 = &first; "
        b"let valid: bool = current == 1; current = &second; "
        b"transition valid { true -> second false -> 1 } }",
        "GuaranteeUnproven",
        b"true -> second",
    )

    # Element stores preserve the collection's length, including the
    # bounds needed by later stores, without making invalid indices valid.
    accepted(
        program,
        "slice-content-stores-preserve-length-bounds",
        b"machine put(items: &mut [u64], i: u64, j: u64) "
        b"requires i < items.len && j < items.len "
        b"{ items[i] = 1; items[j] = 2; }",
    )
    rejected(
        program,
        "slice-content-store-keeps-invalid-second-index",
        b"machine put(items: &mut [u64], i: u64, j: u64) "
        b"requires i < items.len && j == items.len "
        b"{ items[i] = 1; items[j] = 2; }",
        "IndexOutOfBounds",
        b"items[j]",
    )

    # A store captures its destination before its value runs
    # (wiki/spec/language/expressions.md#evaluation-schedule): bounds are
    # proven and the place fixed with the selectors as they were, a
    # selector the value changes neither redirects the store nor keeps its
    # old fact, and a value that invalidates the captured backing rejects.
    token = (b"data Token { value: u64; }\n"
             b"machine Token::drop(token: &mut Token) { token.value = 0; }\n")
    accepted(
        program,
        "store-captures-its-selector-before-the-value",
        token + b"machine replacement_value(index: &mut u64) -> u64 { index = 3; 19 }\n"
        b"machine run() {\n    let mut tokens: [Token; 2] = [Token { value: 11 }, Token { value: 7 }];\n"
        b"    let mut index: u64 = 0;\n    tokens[index] = Token { value: replacement_value(&mut index) };\n}",
    )
    rejected(
        program,
        "store-proves-its-bounds-before-the-value",
        token + b"machine fix(index: &mut u64) -> Token { index = 0; Token { value: 19 } }\n"
        b"machine run() {\n    let mut tokens: [Token; 2] = [Token { value: 11 }, Token { value: 7 }];\n"
        b"    let mut index: u64 = 5;\n    tokens[index] = fix(&mut index);\n}",
        "IndexOutOfBounds",
        b"tokens[index]",
    )
    replacing = (token + b"machine replacement(index: &mut u64) -> Token\nensures index == 1\n"
                 b"{ index = 1; Token { value: 19 } }\n")
    rejected(
        program,
        "store-keeps-no-stale-selector-fact",
        replacing + b"machine zero(index: u64)\nrequires index == 0\n{}\n"
        b"machine run() {\n    let mut tokens: [Token; 2] = [Token { value: 11 }, Token { value: 7 }];\n"
        b"    let mut index: u64 = 0;\n    tokens[index] = replacement(&mut index);\n    zero(index);\n}",
        "RequirementUnproven",
        b"zero(index)",
    )
    accepted(
        program,
        "store-keeps-the-value's-selector-fact",
        replacing + b"machine one(index: u64)\nrequires index == 1\n{}\n"
        b"machine run() {\n    let mut tokens: [Token; 2] = [Token { value: 11 }, Token { value: 7 }];\n"
        b"    let mut index: u64 = 0;\n    tokens[index] = replacement(&mut index);\n    one(index);\n}",
    )
    rejected(
        program,
        "value-may-not-move-the-captured-backing",
        token + b"machine consume(tokens: [Token; 2]) -> Token { Token { value: 1 } }\n"
        b"machine run() {\n    let mut tokens: [Token; 2] = [Token { value: 11 }, Token { value: 7 }];\n"
        b"    let index: u64 = 0;\n    tokens[index] = consume(tokens);\n}",
        "UseAfterMove",
        b"tokens[index]",
    )
    accepted(
        program,
        "value-may-write-through-the-captured-place",
        b"machine bump(x: &mut u64) -> u64 ensures result == 3 { x = 2; 3 }\nmachine three(x: u64)\nrequires x == 3\n{}\n"
        b"machine run() {\n    let mut x: u64 = 1;\n    x = bump(&mut x);\n    three(x);\n}",
    )
    accepted(
        program,
        "field-store-survives-a-write-to-its-owner",
        b"data Holder { count: u64; flag: u64; }\n"
        b"machine touch(holder: &mut Holder) -> u64 ensures result == 4 { holder.flag = 9; 4 }\n"
        b"machine four(x: u64)\nrequires x == 4\n{}\n"
        b"machine run() {\n    let mut holder: Holder = Holder { count: 0, flag: 0 };\n"
        b"    holder.count = touch(&mut holder);\n    four(holder.count);\n}",
    )

    # A recast's representations (recasts.omg): same-width integers and
    # floats view each other either way, a Boolean views as a byte shared
    # only, a byte views as a Boolean only as a field its data's `where`
    # facts bound to zero or one, a qualified source views shared only and
    # a qualified target is unsupported.
    accepted(
        program,
        "scalar-recasts-share-every-bit-pattern",
        b"data M { rate: f32; half: i16; flag: bool; }\n"
        b"machine f(m: &mut M) -> bool {\n"
        b"    let bits: &mut u32 = &mut m.rate as &mut u32;\n    let wide: &u16 = &m.half as &u16;\n"
        b"    let byte: &u8 = &m.flag as &u8;\n    true\n}",
    )
    accepted(
        program,
        "a-bounded-byte-field-views-as-a-boolean",
        b"data Byte where value >= 0 && value <= 1 { value: u8; }\n"
        b"machine read(byte: &mut Byte) -> bool {\n"
        b"    let flag: &bool = &byte.value as &bool;\n    let writable: &mut bool = &mut byte.value as &mut bool;\n    true\n}",
    )
    rejected(
        program,
        "a-local-byte-never-views-as-a-boolean",
        b"machine f() -> bool { let x: u8 = 1; let flag: &bool = &x as &bool; true }",
        "RepresentationUnproven",
        b"&x as &bool",
    )
    rejected(
        program,
        "an-unbounded-byte-field-never-views-as-a-boolean",
        b"data Byte { value: u8; }\nmachine read(byte: &Byte) -> bool { let flag: &bool = &byte.value as &bool; true }",
        "RepresentationUnproven",
        b"&byte.value as &bool",
    )
    rejected(
        program,
        "a-boolean-never-views-as-a-mutable-byte",
        b"data M { flag: bool; }\nmachine f(m: &mut M) { let bits: &mut u8 = &mut m.flag as &mut u8; }",
        "RepresentationUnproven",
        b"&mut m.flag as &mut u8",
    )
    accepted(
        program,
        "a-qualified-source-views-shared",
        b"domain u8::Small(value) requires value < 8;\ndata P { v: u8 in Small; }\n"
        b"machine f(p: &P) { let signed: &i8 = &p.v as &i8; }",
    )
    rejected(
        program,
        "a-qualified-source-never-views-mutably",
        b"domain u8::Small(value) requires value < 8;\ndata P { v: u8 in Small; }\n"
        b"machine f(p: &mut P) { let signed: &mut i8 = &mut p.v as &mut i8; }",
        "RepresentationUnproven",
        b"&mut p.v as &mut i8",
    )
    rejected(
        program,
        "a-recast-argument-holds-its-operand",
        b"data M { raw: i32; }\nmachine take(a: &mut u32, b: i32) {}\n"
        b"machine f(m: &mut M) { take(&mut m.raw as &mut u32, m.raw); }",
        "OverlappingArguments",
        b"m.raw!2",
    )

    # The product's own psi package, each module under source/psi as
    # `<directory>::<file>` of package `psi`: how far checking reaches today.
    psi_root = Path(__file__).resolve().parents[3] / "source" / "psi"
    psi_units = tuple(
        ("::".join(path.relative_to(psi_root).with_suffix("").parts).encode(), path.read_bytes())
        for path in sorted(psi_root.rglob("*.omg"))
        if path.parent != psi_root
    )
    status, payload = run_units(program, psi_units, ((b"psi", False),))
    assert status in (0, 247) and payload, f"psi-package: status {status}, the package must reach checking"
    package = decode(payload)
    print(f"psi-package: status {status}, {len(package.rejections)} rejections")
    for diagnostic, (unit, start, end) in package.rejections:
        print(f"  {diagnostic} at {psi_units[unit][0].decode()} {start}..{end}: {psi_units[unit][1][start:end][:80]!r}")

    phase_rejected(program, "typing-rejection-has-no-observation", b"machine f() -> u8 { true }", 249)
    # A write-only reference is never observed
    # (wiki/spec/language/ownership.md#borrows-and-aliases): a place reached
    # through it is written or lent `&write`; `.len` reads metadata only.
    accepted(
        program,
        "write-only-references-replace-without-observation",
        b"data Cell { value: i64; }\n"
        b"machine replace(cell: &write Cell, value: i64) { cell.value = value; }\n"
        b"machine fill(cells: &write [Cell; 2], value: i64) -> u64 {\n"
        b"    replace(&write cells[0], value);\n    cells[1] = Cell { value: value };\n    cells.len\n}\n"
        b"machine main() -> u64 {\n    let mut cells: [Cell; 2] = [Cell { value: 0 }, Cell { value: 0 }];\n"
        b"    fill(&write cells, 7)\n}",
    )
    rejected(
        program,
        "a-write-only-referent-is-not-read",
        b"data Cell { value: i64; }\nmachine observe(cell: &write Cell) -> i64 { cell.value }",
        "WriteOnlyObservation",
        b"cell.value",
    )
    rejected(
        program,
        "a-write-only-reference-is-not-widened",
        b"machine take(value: &mut i32) { value = 1; }\nmachine widen(value: &write i32) { take(&mut value); }",
        "WriteOnlyObservation",
        b"value!4",
    )
    rejected(
        program,
        "a-write-only-reference-is-not-lent-shared",
        b"machine peek(value: &i32) {}\nmachine f(sink: &write i32) { peek(&sink); }",
        "WriteOnlyObservation",
        b"sink!2",
    )
    phase_rejected(program, "resolution-rejection-has-no-observation", b"data A { x: Q; }", 248)

    determinism = b"machine read(a: &[u8; 4], i: u64) -> u8 { a[i] }"
    first = run(program, determinism)
    second = run(program, determinism)
    assert first == second, "determinism"

    print(f"Psi checking slices: {len(CASE_SOURCES)} cases passed")


if __name__ == "__main__":
    main()
