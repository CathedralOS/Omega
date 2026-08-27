#!/usr/bin/env python3
"""Phase-isolated OMGRSW1/OMGLOW1 mutations for the resolved-to-CKIR gate."""

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

import omega_bootstrap_omglow as omglow  # noqa: E402
from resolution_handoff_reference import decode_witness, one_source  # noqa: E402


U32 = struct.Struct("<I")


def parameter_envelope(path: Path) -> None:
    source = """module app;
data Probe {}
machine Probe::run(&self) -> u32 { 0 }
machine Probe::helper(&self, x: u8) {
    state again(&self, y: u8) {}
}
"""
    path.write_bytes(one_source(source, module="app"))


def replace_u32(raw: bytes, offset: int, value: int) -> bytes:
    result = bytearray(raw)
    U32.pack_into(result, offset, value)
    return bytes(result)


def mutate_component(frame: omglow.Frame, *, comp: bytes | None = None, witness: bytes | None = None) -> bytes:
    return omglow.encode(
        frame.compilation if comp is None else comp,
        frame.witness if witness is None else witness,
    )


def build(canonical_path: Path, parameter_path: Path, output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    canonical = omglow.decode(canonical_path.read_bytes())
    parameter = omglow.decode(parameter_path.read_bytes())
    cw = decode_witness(canonical.witness)
    pw = decode_witness(parameter.witness)
    rows: list[dict[str, object]] = []

    def add(name: str, status: int, raw: bytes) -> None:
        path = output / f"{name}.omglow"
        path.write_bytes(raw)
        rows.append({"name": name, "status": status, "bytes": len(raw)})

    def canonical_word(name: str, table: str, row: int, field_offset: int, value: int) -> None:
        witness = replace_u32(
            canonical.witness,
            cw.offsets[table] + row * dict((
                ("units", 36), ("imports", 48), ("bindings", 28),
                ("declarations", 28), ("types", 24), ("records", 24),
                ("fields", 24), ("machines", 40), ("machine_parameters", 24),
                ("blocks", 40), ("block_parameters", 24),
            ))[table] + field_offset,
            value,
        )
        add(name, 251, mutate_component(canonical, witness=witness))

    canonical_word("unit-owner", "units", 0, 4, 1)
    canonical_word("import-target-module", "imports", 0, 32, 0)
    binding = bytearray(canonical.witness)
    binding[cw.offsets["bindings"] + 8] = 0
    add("binding-role", 251, mutate_component(canonical, witness=bytes(binding)))
    declaration = bytearray(canonical.witness)
    declaration[cw.offsets["declarations"] + 6] = 1
    add("declaration-reserved", 251, mutate_component(canonical, witness=bytes(declaration)))
    canonical_word("type-bool-range", "types", cw.counts[5], 20, 2)
    canonical_word("record-nominal-type", "records", 0, 8, 1)
    canonical_word("field-owner", "fields", 0, 4, 1)
    canonical_word("machine-owner", "machines", 0, 8, cw.counts[5])
    canonical_word("block-body-end", "blocks", 0, 20, 0x7FFFFFFF)

    add("selected-root", 251, mutate_component(
        canonical, witness=replace_u32(canonical.witness, 64, 0xFFFFFFFF),
    ))

    require_parameter_counts = (pw.counts[8] > 0 and pw.counts[10] > 0)
    if not require_parameter_counts:
        raise ValueError("parameter fixture did not produce both parameter row families")
    machine_parameter = replace_u32(
        parameter.witness, pw.offsets["machine_parameters"] + 4, pw.counts[7],
    )
    add("machine-parameter-owner", 251, mutate_component(parameter, witness=machine_parameter))
    block_parameter = replace_u32(
        parameter.witness, pw.offsets["block_parameters"] + 4, 0,
    )
    add("block-parameter-owner", 251, mutate_component(parameter, witness=block_parameter))

    mismatched = bytearray(canonical.compilation)
    needle = b"23;"
    if mismatched.count(needle) != 1:
        raise ValueError("canonical source-body needle drifted")
    at = mismatched.index(needle)
    mismatched[at:at + 2] = b"xx"
    add("source-witness-body", 251, mutate_component(canonical, comp=bytes(mismatched)))

    excessive_count = replace_u32(canonical.witness, 36, 2049)
    add("witness-type-count-2049", 252, mutate_component(canonical, witness=excessive_count))
    add("omgcomp-bytes-267281", 252, struct.pack(
        "<8sHHHH4I", b"OMGLOW1\0", 1, 0, 0, 32,
        32 + 267_281, 267_281, 0, 0,
    ))
    add("omgrsw1-bytes-524289", 252, struct.pack(
        "<8sHHHH4I", b"OMGLOW1\0", 1, 0, 0, 32,
        32 + 524_289, 0, 524_289, 0,
    ))

    add("trailing-byte", 251, canonical.raw + b"\0")
    (output / "index.json").write_text(json.dumps(rows, indent=2) + "\n", encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    parameter = commands.add_parser("parameter-envelope")
    parameter.add_argument("output", type=Path)
    generate = commands.add_parser("build")
    generate.add_argument("canonical", type=Path)
    generate.add_argument("parameter", type=Path)
    generate.add_argument("output", type=Path)
    args = parser.parse_args()
    if args.command == "parameter-envelope":
        parameter_envelope(args.output)
    else:
        build(args.canonical, args.parameter, args.output)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, struct.error) as error:
        raise SystemExit(f"resolved-to-CKIR mutations: {error}")
