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
    "ProgressUnproven",
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
    phase_rejected(program, "resolution-rejection-has-no-observation", b"data A { x: Q; }", 248)

    determinism = b"machine read(a: &[u8; 4], i: u64) -> u8 { a[i] }"
    first = run(program, determinism)
    second = run(program, determinism)
    assert first == second, "determinism"

    print(f"Psi checking slices: {len(CASE_SOURCES)} cases passed")


if __name__ == "__main__":
    main()
