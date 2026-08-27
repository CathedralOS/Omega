#!/usr/bin/env python3
"""Focused OMGCOMP/OMGRSW3 fixtures for the bounded pure-sum relation."""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path


HERE = Path(__file__).resolve().parent
COMPILER = HERE.parent / "compiler"
sys.path.insert(0, str(COMPILER))
sys.path.insert(0, str(HERE))

import omega_bootstrap_bundle as bundle  # noqa: E402
import omega_bootstrap_compilation as compilation  # noqa: E402
from resolution_handoff_reference import source_contents, span  # noqa: E402


PACKAGE_KEY = "44" * 32

VALID = """module app;
data Leaf [copy] { value: u8; }
data Event [copy] {
    case None;
    case Byte(value: u8);
    case Pair(left: Leaf, bytes: [u8; 2]);
}
data Cell [copy] { value: u8; }
machine Cell::read(&self) -> u8 { 70 }
data Probe { cell: Cell; event: Event; }
machine Probe::run(&mut self) -> u8 { self.cell.read() }
"""

LEGACY_V1 = """module app;
data Probe {}
machine Probe::run(&mut self) -> u8 { 70 }
"""

LEGACY_V2 = """module app;
data Cell { value: u8; }
machine Cell::read(&self) -> u8 { 70 }
data Probe { cell: Cell; }
machine Probe::run(&mut self) -> u8 { self.cell.read() }
"""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def encode(source: str) -> bytes:
    packed = bundle.encode([bundle.Entry("main.omg", source.encode("ascii"))])
    manifest = {
        "target": "linux_x86_64",
        "packages": [{
            "key": PACKAGE_KEY,
            "sources": [{"label": "main.omg", "module": "app"}],
        }],
        "aliases": [],
        "root": {
            "package": PACKAGE_KEY,
            "source": "main.omg",
            "owner": "Probe",
            "machine": "run",
        },
    }
    return compilation.encode_manifest(manifest, packed)


def with_root(body: str) -> str:
    return "module app;\n" + body + "\ndata Probe {}\nmachine Probe::run(&mut self) -> u8 { 70 }\n"


def build(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    fixtures = {
        "valid": VALID,
        "legacy-v1": LEGACY_V1,
        "legacy-v2": LEGACY_V2,
        "mixed": with_root("data Bad { case A; value: u8; }"),
        "numbered": with_root("data Bad { case A = 1; }"),
        "duplicate-case": with_root("data Bad { case A; case A; }"),
        "duplicate-payload": with_root("data Bad { case A(value: u8, value: u8); }"),
        "noncopy": with_root("data Owned { value: u8; }\ndata Bad [copy] { case A(value: Owned); }"),
        "cycle": with_root("data Bad { case Again(value: Bad); }"),
        "sum-machine": with_root("data Bad { case A; }\nmachine Bad::no(&self) -> u8 { 1 }"),
        "payloads-5": with_root("data Wide { case A(a: u8, b: u8, c: u8, d: u8, e: u8); }"),
        "payloads-5-malformed": with_root("data Wide { case A(a: u8, b: u8, c: u8, d: u8, a: u8); }"),
        "cases-65": with_root("data Wide {\n" + "".join(f"case C{i};\n" for i in range(65)) + "}"),
    }
    for name, source in fixtures.items():
        (output / f"{name}.omgc").write_bytes(encode(source))


WIDTHS = (
    ("units", 36), ("imports", 48), ("bindings", 28),
    ("declarations", 28), ("types", 24), ("records", 24),
    ("fields", 24), ("sums", 24), ("cases", 28), ("payloads", 24),
    ("machines", 40), ("machine_parameters", 24),
    ("blocks", 40), ("block_parameters", 24),
)


def decode_v3(raw: bytes) -> tuple[dict[str, int], dict[str, list[bytes]], int]:
    require(len(raw) >= 84, "truncated OMGRSW3 header")
    require(raw[:8] == b"OMGRSW3\0", "wrong OMGRSW3 magic")
    major, minor, flags, header = struct.unpack_from("<4H", raw, 8)
    require((major, minor, flags, header) == (3, 0, 0, 84), "wrong OMGRSW3 fixed header")
    words = struct.unpack_from("<17I", raw, 16)
    require(words[0] == len(raw), "OMGRSW3 exact length")
    names = (
        "sources", "imports", "bindings", "declarations", "types", "records",
        "fields", "machines", "machine_parameters", "blocks", "block_parameters",
        "sums", "cases", "payloads", "selected", "reserved",
    )
    counts = dict(zip(names, words[1:]))
    require(counts["reserved"] == 0, "OMGRSW3 reserved word")
    table_counts = (
        counts["sources"], counts["imports"], counts["bindings"],
        counts["declarations"], counts["types"], counts["records"], counts["fields"],
        counts["sums"], counts["cases"], counts["payloads"], counts["machines"],
        counts["machine_parameters"], counts["blocks"], counts["block_parameters"],
    )
    offset = 84
    rows: dict[str, list[bytes]] = {}
    for (name, width), count in zip(WIDTHS, table_counts):
        end = offset + width * count
        require(end <= len(raw), f"truncated {name} table")
        rows[name] = [raw[offset + width * i:offset + width * (i + 1)] for i in range(count)]
        offset = end
    require(offset == len(raw), "OMGRSW3 trailing bytes")
    return counts, rows, counts["selected"]


def check(envelope_path: Path, witness_path: Path) -> None:
    envelope = envelope_path.read_bytes()
    counts, rows, selected = decode_v3(witness_path.read_bytes())
    sources = source_contents(envelope)
    require(counts == {
        "sources": 1, "imports": 0, "bindings": 6, "declarations": 6,
        "types": 8, "records": 3, "fields": 4, "machines": 2,
        "machine_parameters": 0, "blocks": 2, "block_parameters": 0,
        "sums": 1, "cases": 3, "payloads": 3, "selected": 1, "reserved": 0,
    }, "exact OMGRSW3 table counts")
    require(selected == 1, "selected Probe::run identity")

    declarations = []
    for index, row in enumerate(rows["declarations"]):
        did = struct.unpack_from("<I", row)[0]
        kind = row[4]
        source_id, ordinal, start, length, kind_id = struct.unpack_from("<5I", row, 8)
        require(did == index and row[6:8] == b"\0\0", "declaration dense/reserved")
        declarations.append((kind, source_id, ordinal, span(sources[source_id], start, length), kind_id))
    require(declarations == [
        (1, 0, 0, b"Leaf", 0), (3, 0, 1, b"Event", 0),
        (1, 0, 2, b"Cell", 1), (2, 0, 3, b"read", 0),
        (1, 0, 4, b"Probe", 2), (2, 0, 5, b"run", 1),
    ], "record/sum/machine filtered identities")

    type_heads = []
    for index, row in enumerate(rows["types"]):
        tid = struct.unpack_from("<I", row)[0]
        kind, flags = row[4], row[5]
        payload0, payload1, low, high = struct.unpack_from("<4I", row, 8)
        require(tid == index and row[6:8] == b"\0\0", "type dense/reserved")
        type_heads.append((kind, flags, payload0, payload1, low, high))
    require(type_heads[:6] == [
        (4, 0, 0, 0, 0, 0), (4, 0, 1, 0, 0, 0), (4, 0, 2, 0, 0, 0),
        (6, 0, 0, 0, 0, 0), (3, 0, 0, 0, 0, 1),
        (2, 0, 0, 0, 0, 2147483647),
    ], "records then sums then bool/u32 canonical prefix")
    require(type_heads[6] == (1, 0, 0, 0, 0, 255), "canonical u8")
    require(type_heads[7] == (5, 0, 6, 2, 0, 0), "canonical payload array")

    sum_row = rows["sums"][0]
    require(struct.unpack_from("<5I", sum_row) == (0, 1, 3, 0, 3), "exact sum row")
    require(sum_row[20:] == b"\1\0\0\0", "checked sum copy flag")
    expected_cases = [(0, 0, 0, 0, 0, b"None"), (1, 0, 1, 0, 1, b"Byte"), (2, 0, 2, 1, 2, b"Pair")]
    actual_cases = []
    for row in rows["cases"]:
        cid, owner, ordinal, start, count, name_start, name_len = struct.unpack("<7I", row)
        actual_cases.append((cid, owner, ordinal, start, count, span(sources[0], name_start, name_len)))
    require(actual_cases == expected_cases, "exact case identities/spans")
    expected_payloads = [(0, 1, 0, 6, b"value"), (1, 2, 0, 0, b"left"), (2, 2, 1, 7, b"bytes")]
    actual_payloads = []
    for row in rows["payloads"]:
        pid, owner, ordinal, type_id, name_start, name_len = struct.unpack("<6I", row)
        actual_payloads.append((pid, owner, ordinal, type_id, span(sources[0], name_start, name_len)))
    require(actual_payloads == expected_payloads, "exact payload identities/types/spans")

    role3 = []
    for row in rows["bindings"]:
        source_id = struct.unpack_from("<I", row, 4)[0]
        role, kind = row[8], row[9]
        start, length, declaration = struct.unpack_from("<3I", row, 12)
        if role == 3:
            role3.append((source_id, kind, span(sources[source_id], start, length), declaration))
    require(role3 == [(0, 2, b"read", 3)], "inherited direct-field role-3 resolution")
    print("sum resolution fixture: exact OMGRSW3 identities, types, rows, and field call passed")


def check_magic(witness_path: Path, expected: str) -> None:
    raw = witness_path.read_bytes()
    require(raw[:8] == expected.encode("ascii") + b"\0", f"expected {expected}")


def main() -> None:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    build_parser = sub.add_parser("build")
    build_parser.add_argument("output", type=Path)
    check_parser = sub.add_parser("check")
    check_parser.add_argument("envelope", type=Path)
    check_parser.add_argument("witness", type=Path)
    magic_parser = sub.add_parser("check-magic")
    magic_parser.add_argument("witness", type=Path)
    magic_parser.add_argument("expected")
    args = parser.parse_args()
    if args.command == "build":
        build(args.output)
    elif args.command == "check":
        check(args.envelope, args.witness)
    else:
        check_magic(args.witness, args.expected)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, compilation.CompilationError, bundle.BundleError, struct.error) as error:
        raise SystemExit(f"sum resolution fixture: {error}")
