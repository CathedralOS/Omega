#!/usr/bin/env python3
"""Independent OMGRSW4 fixtures and normalized-witness reference checks."""

from __future__ import annotations

import argparse
import json
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


DEP_KEY = "11" * 32
ROOT_KEY = "22" * 32
NO_ID = 0xFFFFFFFF

DEP_SOURCE = b"""module model;
pub data Leaf [copy] { value: u8; }
"""

VALID_SOURCE = b"""module app;
use dep::model::Leaf;
data Event [copy] { case None; case Byte(value: u8); }
data Probe { leaf: Leaf; }
machine Probe::consume(&self, bytes: &[u8]) -> u8 { 70 }
machine Probe::run(&mut self) -> u8 {
    self.consume("Fp");
    transition { _ -> scan("A{B}") }
    state scan(&mut self, bytes: &[u8]) { 70 }
}
"""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def encode(source: bytes, *, dependency: bool = False) -> bytes:
    if dependency:
        entries = [bundle.Entry("dep/model.omg", DEP_SOURCE), bundle.Entry("root/main.omg", source)]
        packages = [
            {"key": DEP_KEY, "sources": [{"label": "dep/model.omg", "module": "model"}]},
            {"key": ROOT_KEY, "sources": [{"label": "root/main.omg", "module": "app"}]},
        ]
        aliases = [{"requester": ROOT_KEY, "alias": "dep", "target": DEP_KEY}]
        root_key, root_label = ROOT_KEY, "root/main.omg"
    else:
        entries = [bundle.Entry("main.omg", source)]
        packages = [{"key": ROOT_KEY, "sources": [{"label": "main.omg", "module": "app"}]}]
        aliases = []
        root_key, root_label = ROOT_KEY, "main.omg"
    packed = bundle.encode(entries)
    manifest = {
        "target": "linux_x86_64",
        "packages": packages,
        "aliases": aliases,
        "root": {"package": root_key, "source": root_label, "owner": "Probe", "machine": "run"},
    }
    return compilation.encode_manifest(manifest, packed)


def root(body: bytes) -> bytes:
    return b"module app;\ndata Probe {}\n" + body + b"\n"


def root_with_slice(body: bytes) -> bytes:
    return root(b"machine Probe::view(&self, bytes: &[u8]) -> u8 { 0 }\n" + body)


def build(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    cases: dict[str, tuple[int, bytes]] = {
        "valid-v4": (0, encode(VALID_SOURCE, dependency=True)),
        "legacy-v1": (0, encode(root(b'machine Probe::run(&mut self) -> u8 { 70 }'))),
        "legacy-v1-literal": (251, encode(root(b'machine Probe::run(&mut self) -> u8 { "plain"; 70 }'))),
        "legacy-v2": (0, encode(b"""module app;
data Cell { value: u8; }
machine Cell::read(&self) -> u8 { 70 }
data Probe { cell: Cell; }
machine Probe::run(&mut self) -> u8 { self.cell.read() }
""")),
        "legacy-v3": (0, encode(b"""module app;
data Choice [copy] { case None; case Byte(value: u8); }
data Probe { choice: Choice; }
machine Probe::run(&mut self) -> u8 { 70 }
""")),
        "slice-before-sum": (0, encode(b"""module app;
data Probe {}
machine Probe::view(&self, bytes: &[u8]) -> u8 { 0 }
data Choice [copy] { case None; case Byte(value: u8); }
machine Probe::run(&mut self) -> u8 { 70 }
""")),
        "literal-before-state-slice": (0, encode(b"""module app;
data Probe {}
machine Probe::run(&mut self) -> u8 {
    "Fp";
    transition { _ -> scan("tail") }
    state scan(&mut self, bytes: &[u8]) { 70 }
}
""")),
        "literal-empty": (0, encode(root_with_slice(b'machine Probe::run(&mut self) -> u8 { ""; 70 }'))),
        "literal-32": (0, encode(root_with_slice(b'machine Probe::run(&mut self) -> u8 { "01234567890123456789012345678901"; 70 }'))),
        "literal-33": (252, encode(root_with_slice(b'machine Probe::run(&mut self) -> u8 { "012345678901234567890123456789012"; 70 }'))),
        "slice-mutable": (251, encode(root(b"machine Probe::helper(&self, bytes: &mut [u8]) -> u8 { 0 }\nmachine Probe::run(&mut self) -> u8 { 70 }"))),
        "slice-u32": (251, encode(root(b"machine Probe::helper(&self, bytes: &[u32]) -> u8 { 0 }\nmachine Probe::run(&mut self) -> u8 { 70 }"))),
        "slice-bool": (251, encode(root(b"machine Probe::helper(&self, bytes: &[bool]) -> u8 { 0 }\nmachine Probe::run(&mut self) -> u8 { 70 }"))),
        "slice-array": (251, encode(root(b"machine Probe::helper(&self, bytes: &[[u8; 2]]) -> u8 { 0 }\nmachine Probe::run(&mut self) -> u8 { 70 }"))),
        "slice-field": (251, encode(b"module app; data Probe { bytes: &[u8]; } machine Probe::run(&mut self) -> u8 { 70 }")),
        "slice-result": (251, encode(root(b"machine Probe::helper(&self) -> &[u8] { \"x\" }\nmachine Probe::run(&mut self) -> u8 { 70 }"))),
        "literal-escape": (251, encode(root(b'machine Probe::run(&mut self) -> u8 { "F\\n"; 70 }'))),
        "literal-codepoint": (251, encode(root(b'machine Probe::run(&mut self) -> u8 { "\\u{46}"; 70 }'))),
        "literal-raw": (251, encode(root(b'machine Probe::run(&mut self) -> u8 { r"Fp"; 70 }'))),
        "literal-raw-hash": (251, encode(root(b'machine Probe::run(&mut self) -> u8 { r#"Fp"#; 70 }'))),
        "literal-control": (251, encode(root(b'machine Probe::run(&mut self) -> u8 { "F\tp"; 70 }'))),
        "literal-nonascii": (251, encode(root(b'machine Probe::run(&mut self) -> u8 { "F\x80p"; 70 }'))),
        "literal-unterminated": (251, encode(root(b'machine Probe::run(&mut self) -> u8 { "Fp; 70 }'))),
    }
    index = []
    for name, (status, payload) in cases.items():
        (output / f"{name}.omgc").write_bytes(payload)
        index.append({"name": name, "status": status})
    (output / "index.json").write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")


WIDTHS = (
    ("units", 36), ("imports", 48), ("bindings", 28),
    ("declarations", 28), ("types", 24), ("records", 24),
    ("fields", 24), ("sums", 24), ("cases", 28), ("payloads", 24),
    ("machines", 40), ("machine_parameters", 24),
    ("blocks", 40), ("block_parameters", 24),
)


def decode(raw: bytes) -> tuple[dict[str, int], dict[str, list[bytes]]]:
    require(len(raw) >= 84, "truncated OMGRSW4")
    require(raw[:8] == b"OMGRSW4\0", "OMGRSW4 magic")
    require(struct.unpack_from("<4H", raw, 8) == (4, 0, 0, 84), "OMGRSW4 fixed header")
    words = struct.unpack_from("<17I", raw, 16)
    require(words[0] == len(raw), "OMGRSW4 exact length")
    names = (
        "sources", "imports", "bindings", "declarations", "types", "records",
        "fields", "machines", "machine_parameters", "blocks", "block_parameters",
        "sums", "cases", "payloads", "selected", "reserved",
    )
    counts = dict(zip(names, words[1:]))
    require(counts["reserved"] == 0, "OMGRSW4 reserved")
    table_counts = (
        counts["sources"], counts["imports"], counts["bindings"], counts["declarations"],
        counts["types"], counts["records"], counts["fields"], counts["sums"],
        counts["cases"], counts["payloads"], counts["machines"],
        counts["machine_parameters"], counts["blocks"], counts["block_parameters"],
    )
    rows: dict[str, list[bytes]] = {}
    at = 84
    for (name, width), count in zip(WIDTHS, table_counts):
        end = at + width * count
        require(end <= len(raw), f"{name} extent")
        rows[name] = [raw[at + width * i:at + width * (i + 1)] for i in range(count)]
        at = end
    require(at == len(raw), "OMGRSW4 exact EOF")
    return counts, rows


def check(envelope_path: Path, witness_path: Path) -> None:
    envelope = envelope_path.read_bytes()
    counts, rows = decode(witness_path.read_bytes())
    sources = source_contents(envelope)
    require(counts == {
        "sources": 2, "imports": 1, "bindings": 4, "declarations": 5,
        "types": 7, "records": 2, "fields": 2, "machines": 2,
        "machine_parameters": 1, "blocks": 3, "block_parameters": 1,
        "sums": 1, "cases": 2, "payloads": 1, "selected": 1, "reserved": 0,
    }, f"exact OMGRSW4 counts: {counts!r}")

    types = [struct.unpack("<IBBHIIII", row) for row in rows["types"]]
    require(types == [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 4, 0, 0, 1, 0, 0, 0),
        (2, 6, 0, 0, 0, 0, 0, 0),
        (3, 3, 0, 0, 0, 0, 0, 1),
        (4, 2, 0, 0, 0, 0, 0, 0x7FFFFFFF),
        (5, 1, 0, 0, 0, 0, 0, 255),
        (6, 7, 0, 0, 5, 0, 0, 0),
    ], f"canonical OMGRSW4 types: {types!r}")

    machine_slice = struct.unpack_from("<I", rows["machine_parameters"][0], 12)[0]
    block_slice = struct.unpack_from("<I", rows["block_parameters"][0], 12)[0]
    require((machine_slice, block_slice) == (6, 6), "machine/state slice parameter types")

    declarations = []
    for row in rows["declarations"]:
        did = struct.unpack_from("<I", row)[0]
        kind = row[4]
        source_id, ordinal, start, length, kind_id = struct.unpack_from("<5I", row, 8)
        declarations.append((did, kind, source_id, ordinal, span(sources[source_id], start, length), kind_id))
    require(declarations == [
        (0, 1, 0, 0, b"Leaf", 0),
        (1, 3, 1, 0, b"Event", 0),
        (2, 1, 1, 1, b"Probe", 1),
        (3, 2, 1, 2, b"consume", 0),
        (4, 2, 1, 3, b"run", 1),
    ], f"declaration identities: {declarations!r}")

    literal_source = sources[1]
    require(literal_source.count(b'"Fp"') == 1 and literal_source.count(b'"A{B}"') == 1,
            "literal source custody")
    require(all(len(row) == width for (name, width) in WIDTHS for row in rows[name]), "row widths")
    print("shared byte view resolution: exact OMGRSW4 V3-shaped tables, kind7, parameters, literals passed")


def check_magic(path: Path, expected: str) -> None:
    raw = path.read_bytes()
    require(raw[:8] == expected.encode("ascii") + b"\0", f"expected {expected}")


def main() -> None:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    p = sub.add_parser("build"); p.add_argument("output", type=Path)
    p = sub.add_parser("check"); p.add_argument("envelope", type=Path); p.add_argument("witness", type=Path)
    p = sub.add_parser("check-magic"); p.add_argument("witness", type=Path); p.add_argument("expected")
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
    except (OSError, ValueError, struct.error, bundle.BundleError, compilation.CompilationError) as error:
        raise SystemExit(f"shared byte view resolution fixture: {error}")
