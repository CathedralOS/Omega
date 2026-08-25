#!/usr/bin/env python3
"""Focused OMGCOMP/OMGRSW1 fixture for ordinary attached self-call bindings."""

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
from resolution_handoff_reference import NO_ID, decode_witness, source_contents, span  # noqa: E402


PACKAGE_KEY = "33" * 32

ROOT_SOURCE = """module app;
data Probe {}
machine Probe::run(&mut self) -> u8 {
    self.local(68)
}
machine Probe::local(&mut self, value: u8) -> u8 {
    self.cross(value)
}
"""

SECOND_SOURCE = """module app;
machine Probe::cross(&mut self, value: u8) -> u8 {
    value + 2
}
machine Probe::decoy(&self) -> u8 {
    7
}
"""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def encode(first: str, second: str, second_module: str = "app") -> bytes:
    entries = [
        bundle.Entry("a-main.omg", first.encode("ascii")),
        bundle.Entry("b-helper.omg", second.encode("ascii")),
    ]
    packed = bundle.encode(entries)
    manifest = {
        "target": "linux_x86_64",
        "packages": [{
            "key": PACKAGE_KEY,
            "sources": [
                {"label": "a-main.omg", "module": "app"},
                {"label": "b-helper.omg", "module": second_module},
            ],
        }],
        "aliases": [],
        "root": {
            "package": PACKAGE_KEY,
            "source": "a-main.omg",
            "owner": "Probe",
            "machine": "run",
        },
    }
    return compilation.encode_manifest(manifest, packed)


def build(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    (output / "valid.omgc").write_bytes(encode(ROOT_SOURCE, SECOND_SOURCE))
    (output / "missing.omgc").write_bytes(
        encode(ROOT_SOURCE.replace("self.local(68)", "self.missing(68)"), SECOND_SOURCE)
    )
    wrong_owner = SECOND_SOURCE.replace(
        "machine Probe::cross", "data Other {}\nmachine Other::cross"
    )
    (output / "wrong-owner.omgc").write_bytes(encode(ROOT_SOURCE, wrong_owner))
    (output / "private-cross-module.omgc").write_bytes(
        encode(ROOT_SOURCE, SECOND_SOURCE.replace("module app;", "module other;"), "other")
    )


def check(envelope_path: Path, witness_path: Path) -> None:
    envelope = envelope_path.read_bytes()
    witness = decode_witness(witness_path.read_bytes())
    sources = source_contents(envelope)
    require(witness.counts[0:4] == (2, 0, 6, 5), "unit/import/binding/declaration counts")
    require(witness.counts[7:11] == (4, 2, 4, 0), "machine/parameter/block counts")
    require(witness.selected == 0, "exact selected run machine")

    expected = [
        (0, 2, 1, b"Probe", 0),
        (0, 3, 2, b"local", 2),
        (0, 2, 1, b"Probe", 0),
        (0, 3, 2, b"cross", 3),
        (1, 2, 1, b"Probe", 0),
        (1, 2, 1, b"Probe", 0),
    ]
    actual = []
    previous_key = (-1, -1, -1)
    for index in range(6):
        row = witness.row("bindings", index)
        bid, source_id = struct.unpack_from("<2I", row)
        role, kind = row[8], row[9]
        start, length, declaration, import_id = struct.unpack_from("<4I", row, 12)
        require(bid == index and row[10:12] == b"\0\0", "binding dense/reserved")
        require(import_id == NO_ID, "ordinary self call must not invent an import")
        key = (source_id, start, role)
        require(key > previous_key, "binding source/start/role order")
        previous_key = key
        actual.append((source_id, role, kind, span(sources[source_id], start, length), declaration))
    require(actual == expected, "exact owner and role-3 binding rows")

    machine_declarations = []
    for index in range(4):
        row = witness.row("machines", index)
        machine_id, declaration_id, owner_id = struct.unpack_from("<3I", row)
        require(machine_id == index and owner_id == 0, "machine owner identity")
        machine_declarations.append(declaration_id)
    require(machine_declarations == [1, 2, 3, 4], "semantic machine declaration order")
    print("role-3 resolution fixture: exact same-module cross-source self-call bindings passed")


def main() -> None:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    build_parser = sub.add_parser("build")
    build_parser.add_argument("output", type=Path)
    check_parser = sub.add_parser("check")
    check_parser.add_argument("envelope", type=Path)
    check_parser.add_argument("witness", type=Path)
    args = parser.parse_args()
    if args.command == "build":
        build(args.output)
    else:
        check(args.envelope, args.witness)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, compilation.CompilationError, bundle.BundleError, struct.error) as error:
        raise SystemExit(f"role-3 resolution fixture: {error}")
