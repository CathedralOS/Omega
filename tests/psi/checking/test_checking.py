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
    "DomainUnproven", "CrashNotDeclared", "GuaranteeUnproven",
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
    rejected(
        program,
        "termination-promise-is-unsupported",
        b"machine f(n: u64) terminates by n; { }",
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
