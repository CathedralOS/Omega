#!/usr/bin/env python3
"""Handcrafted CKIR4 opcode-13 fixtures and focused mutation controls."""

from __future__ import annotations

import argparse
import copy
import struct
from pathlib import Path

import checked_elf_v4_reference as elf4
import checked_ir_v4_reference as ir4


NO_ID = ir4.NO_ID


def encode(
    tables: dict[str, list[tuple[int, ...]]],
    values: int,
    places: int,
    entry: int = 0,
) -> bytes:
    counts = {name: len(tables[name]) for name in ir4.TABLE_ORDER}
    counts["values"] = values
    counts["places"] = places
    payload = b"".join(
        ir4.ROWS[name].pack(*row)
        for name in ir4.TABLE_ORDER
        for row in tables[name]
    )
    return ir4.HEADER.pack(
        b"OMGCKIR\0", 4, 0, 1, 1, entry, ir4.HEADER.size + len(payload),
        *(counts[name] for name in ir4.COUNT_NAMES),
    ) + payload


def canonical_tables() -> dict[str, list[tuple[int, ...]]]:
    # Type 6 is the deliberately narrower source value type used for the
    # Inner.value destination interval [0,100].
    types = [
        (0, 1, 0, 0, 0, 0, 0, 255),
        (1, 2, 0, 0, 0, 0, 0, 100),
        (2, 3, 0, 0, 0, 0, 0, 1),
        (3, 4, 0, 0, 0, 0, 0, 0),
        (4, 4, 0, 0, 1, 0, 0, 0),
        (5, 4, 0, 0, 2, 0, 0, 0),
        (6, 2, 0, 0, 0, 0, 70, 70),
    ]
    records = [
        (0, 3, 0, 1, 0, 0, 0, 0),
        (1, 4, 1, 1, 1, 0, 0, 0),
        (2, 5, 2, 2, 1, 0, 0, 0),
    ]
    fields = [
        (0, 0, 0, 5),
        (1, 1, 0, 1),
        (2, 2, 0, 4),
        (3, 2, 1, 0),
    ]
    machines = [
        (0, 0, 2, 0, 0, 1, 0, 0, 0, 1, 0),
        (1, 0, 2, 0, 0, NO_ID, 0, 1, 1, 1, 1),
    ]
    machine_params = [(0, 1, 0, 5, 0)]
    blocks = [
        (0, 0, 2, 0, 0, 0, 0, 0, 10, 0),
        (1, 1, 2, 0, 0, 0, 0, 10, 3, 1),
    ]
    operations = [
        (0, 0, 0, 2, 2, 0, 0, 3, 0, 0, 0, 0),
        (1, 0, 0, 1, 1, 0, 1, 6, 0, 0, 70, 0),
        (2, 0, 0, 13, 1, 0, 2, 4, 0, 1, 0, 0),
        (3, 0, 0, 1, 1, 0, 3, 0, 1, 0, 7, 0),
        (4, 0, 0, 13, 1, 0, 4, 5, 1, 2, 0, 0),
        (5, 0, 0, 10, 0, 0, NO_ID, NO_ID, 3, 2, 1, 0),
        (6, 0, 0, 3, 2, 0, 1, 5, 5, 1, 0, 0),
        (7, 0, 0, 3, 2, 0, 2, 4, 6, 1, 2, 0),
        (8, 0, 0, 3, 2, 0, 3, 1, 7, 1, 1, 0),
        (9, 0, 0, 5, 1, 0, 5, 1, 8, 1, 0, 0),
        (10, 1, 1, 2, 2, 0, 4, 3, 9, 0, 0, 0),
        (11, 1, 1, 3, 2, 0, 5, 5, 9, 1, 0, 0),
        (12, 1, 1, 7, 0, 0, NO_ID, NO_ID, 10, 2, 1, 0),
    ]
    operands = [(1,), (2,), (3,), (0,), (4,), (0,), (1,), (2,), (3,), (4,), (5,), (0,)]
    terminators = [
        (0, 0, 0, 4, 0, 0, 5, NO_ID, 12, 0, NO_ID, 12, 0),
        (1, 1, 1, 3, 0, 0, NO_ID, NO_ID, 12, 0, NO_ID, 12, 0),
    ]
    return {
        "types": types,
        "records": records,
        "fields": fields,
        "machines": machines,
        "machine_params": machine_params,
        "blocks": blocks,
        "block_params": [],
        "constants": [],
        "constant_children": [],
        "operations": operations,
        "operands": operands,
        "terminators": terminators,
    }


def direct_edge_fixture() -> bytes:
    tables = {
        "types": [
            (0, 2, 0, 0, 0, 0, 0, 100),
            (1, 4, 0, 0, 0, 0, 0, 0),
            (2, 4, 0, 0, 1, 0, 0, 0),
        ],
        "records": [
            (0, 1, 0, 0, 0, 0, 0, 0),
            (1, 2, 0, 1, 1, 0, 0, 0),
        ],
        "fields": [(0, 1, 0, 0)],
        "machines": [(0, 0, 2, 0, 0, 0, 0, 0, 0, 2, 0)],
        "machine_params": [],
        "blocks": [
            (0, 0, 2, 0, 0, 0, 0, 0, 2, 0),
            (1, 0, 2, 0, 0, 0, 1, 2, 1, 1),
        ],
        "block_params": [(0, 1, 0, 2, 0)],
        "constants": [],
        "constant_children": [],
        "operations": [
            (0, 0, 0, 1, 1, 0, 1, 0, 0, 0, 70, 0),
            (1, 0, 0, 13, 1, 0, 2, 2, 0, 1, 0, 0),
            (2, 0, 1, 1, 1, 0, 3, 0, 1, 0, 70, 0),
        ],
        "operands": [(1,), (2,)],
        "terminators": [
            (0, 0, 0, 1, 0, 0, NO_ID, 1, 1, 1, NO_ID, 2, 0),
            (1, 0, 1, 4, 0, 0, 3, NO_ID, 2, 0, NO_ID, 2, 0),
        ],
    }
    return encode(tables, values=4, places=0)


def five_field_fixture(malformed: bool = False) -> bytes:
    operands = [(0,), (1,), (2,), (3,), (4,)]
    if malformed:
        operands[-1] = (6,)  # later result: full arity but invalid visibility
    tables = {
        "types": [
            (0, 1, 0, 0, 0, 0, 0, 255),
            (1, 4, 0, 0, 0, 0, 0, 0),
            (2, 4, 0, 0, 1, 0, 0, 0),
        ],
        "records": [
            (0, 1, 0, 0, 0, 0, 0, 0),
            (1, 2, 0, 5, 1, 0, 0, 0),
        ],
        "fields": [(index, 1, index, 0) for index in range(5)],
        "machines": [(0, 0, 2, 0, 0, 0, 0, 0, 0, 1, 0)],
        "machine_params": [],
        "blocks": [(0, 0, 2, 0, 0, 0, 0, 0, 7, 0)],
        "block_params": [],
        "constants": [],
        "constant_children": [],
        "operations": [
            *[
                (index, 0, 0, 1, 1, 0, index, 0, 0, 0, index + 1, 0)
                for index in range(5)
            ],
            (5, 0, 0, 13, 1, 0, 5, 2, 0, 5, 0, 0),
            (6, 0, 0, 1, 1, 0, 6, 0, 5, 0, 70, 0),
        ],
        "operands": operands,
        "terminators": [
            (0, 0, 0, 4, 0, 0, 6, NO_ID, 5, 0, NO_ID, 5, 0),
        ],
    }
    return encode(tables, values=7, places=0)


def empty_record_fixture() -> bytes:
    tables = {
        "types": [
            (0, 1, 0, 0, 0, 0, 0, 255),
            (1, 4, 0, 0, 0, 0, 0, 0),
            (2, 4, 0, 0, 1, 0, 0, 0),
        ],
        "records": [
            (0, 1, 0, 0, 0, 0, 0, 0),
            (1, 2, 0, 0, 1, 0, 0, 0),
        ],
        "fields": [],
        "machines": [(0, 0, 2, 0, 0, 0, 0, 0, 0, 1, 0)],
        "machine_params": [],
        "blocks": [(0, 0, 2, 0, 0, 0, 0, 0, 2, 0)],
        "block_params": [],
        "constants": [],
        "constant_children": [],
        "operations": [
            (0, 0, 0, 13, 1, 0, 0, 2, 0, 0, 0, 0),
            (1, 0, 0, 1, 1, 0, 1, 0, 0, 0, 70, 0),
        ],
        "operands": [],
        "terminators": [
            (0, 0, 0, 4, 0, 0, 1, NO_ID, 0, 0, NO_ID, 0, 0),
        ],
    }
    return encode(tables, values=2, places=0)


def _replace_row(row: tuple[int, ...], index: int, value: int) -> tuple[int, ...]:
    mutable = list(row)
    mutable[index] = value
    return tuple(mutable)


def malformed_cases() -> dict[str, bytes]:
    base = canonical_tables()
    cases: dict[str, bytes] = {}

    def one(name: str, change) -> None:
        tables = copy.deepcopy(base)
        change(tables)
        cases[name] = encode(tables, values=6, places=6)

    one("constructor-owner", lambda t: t["operations"].__setitem__(
        2, _replace_row(t["operations"][2], 1, 1)))
    one("constructor-result-kind", lambda t: t["operations"].__setitem__(
        2, _replace_row(t["operations"][2], 4, 0)))
    one("constructor-result-id", lambda t: t["operations"].__setitem__(
        2, _replace_row(t["operations"][2], 6, 3)))
    one("constructor-result-type", lambda t: t["operations"].__setitem__(
        2, _replace_row(t["operations"][2], 7, 1)))
    one("constructor-immediate-zero", lambda t: t["operations"].__setitem__(
        2, _replace_row(t["operations"][2], 10, 1)))
    one("constructor-immediate-one", lambda t: t["operations"].__setitem__(
        2, _replace_row(t["operations"][2], 11, 1)))
    one("constructor-operand-order", lambda t: t["operands"].__setitem__(
        slice(1, 3), list(reversed(t["operands"][1:3]))))
    one("constructor-operand-visibility", lambda t: t["operands"].__setitem__(0, (4,)))
    one("constructor-scalar-interval", lambda t: t["types"].__setitem__(
        6, _replace_row(t["types"][6], 7, 101)))
    one("constructor-noncopyable", lambda t: t["records"].__setitem__(
        2, _replace_row(t["records"][2], 4, 0)))

    def arity(tables) -> None:
        del tables["operands"][2]
        tables["operations"][4] = _replace_row(tables["operations"][4], 9, 1)
        for index in range(5, len(tables["operations"])):
            row = tables["operations"][index]
            tables["operations"][index] = _replace_row(row, 8, row[8] - 1)
        for index, row in enumerate(tables["terminators"]):
            row = _replace_row(row, 8, row[8] - 1)
            row = _replace_row(row, 11, row[11] - 1)
            tables["terminators"][index] = row

    one("constructor-arity", arity)
    schema = bytearray(encode(base, values=6, places=6))
    struct.pack_into("<H", schema, 8, 3)
    cases["schema-major-3"] = bytes(schema)
    cases["constructor-direct-edge"] = direct_edge_fixture()
    return cases


def emit(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    (directory / "canonical.ckir4").write_bytes(
        encode(canonical_tables(), values=6, places=6)
    )
    (directory / "empty.ckir4").write_bytes(empty_record_fixture())
    manifest: list[tuple[str, int]] = []
    for name, contents in malformed_cases().items():
        (directory / f"{name}.ckir4").write_bytes(contents)
        manifest.append((name, 251))
    (directory / "constructor-five-valid.ckir4").write_bytes(five_field_fixture())
    manifest.append(("constructor-five-valid", 252))
    (directory / "constructor-five-malformed.ckir4").write_bytes(five_field_fixture(True))
    manifest.append(("constructor-five-malformed", 251))
    (directory / "manifest.tsv").write_text(
        "".join(f"{name}\t{status}\n" for name, status in manifest),
        encoding="ascii",
    )


def check_artifact(ckir_path: Path, elf_path: Path) -> str:
    module = ir4.decode(ckir_path.read_bytes())
    reconstructor = elf4.Reconstructor(module)
    artifact = reconstructor.reconstruct()
    require = ir4.require
    require(artifact == elf_path.read_bytes(), "fixture artifact mismatch")
    require(reconstructor.constructor_offsets == {2: 84, 4: 92},
            f"constructor offsets {reconstructor.constructor_offsets}")
    require(reconstructor.frame_sizes == {0: 112, 1: 32},
            f"frame sizes {reconstructor.frame_sizes}")
    text = artifact[4096:]
    for displacement in (-84, -92):
        require(b"\x4c\x8d\x95" + struct.pack("<i", displacement) in text,
                f"missing constructor LEA {displacement}")
    require(b"\x41\x89\x82\x00\x00\x00\x00" in text,
            "missing u32 constructor store")
    require(b"\x41\x88\x82\x04\x00\x00\x00" in text,
            "missing u8 constructor store")
    require(
        b"\x41\x8b\x83\x00\x00\x00\x00\x41\x89\x82\x00\x00\x00\x00" in text,
        "missing nested structural leaf copy",
    )
    require(b"\x4c\x89\x95" + struct.pack("<i", -24) in text,
            "missing inner value-slot publication")
    require(b"\x4c\x89\x95" + struct.pack("<i", -40) in text,
            "missing outer value-slot publication")
    return (
        f"objects=2 offsets=84/92 frames=112/32 artifact={len(artifact)} "
        "nested-u32/u8/copy/publication-templates=exact"
    )


def mutate_artifact(source: Path, destination: Path) -> None:
    artifact = bytearray(source.read_bytes())
    anchor = artifact.find(b"\x4c\x8d\x95" + struct.pack("<i", -84))
    if anchor < 0:
        raise ValueError("constructor LEA mutation anchor absent")
    artifact[anchor] ^= 1
    destination.write_bytes(artifact)


def check_empty_artifact(ckir_path: Path, elf_path: Path) -> str:
    module = ir4.decode(ckir_path.read_bytes())
    reconstructor = elf4.Reconstructor(module)
    artifact = reconstructor.reconstruct()
    ir4.require(artifact == elf_path.read_bytes(), "empty artifact mismatch")
    ir4.require(reconstructor.constructor_offsets == {0: 21},
                f"empty anchor offset {reconstructor.constructor_offsets}")
    ir4.require(reconstructor.frame_sizes == {0: 32},
                f"empty frame size {reconstructor.frame_sizes}")
    text = artifact[4096:]
    ir4.require(b"\x4c\x8d\x95" + struct.pack("<i", -21) in text,
                "missing empty constructor LEA")
    ir4.require(b"\x4c\x89\x95" + struct.pack("<i", -16) in text,
                "missing empty constructor publication")
    return "empty-anchor=1B offset=21 frame=32 publication-template=exact"


def main() -> None:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    emit_parser = subparsers.add_parser("emit")
    emit_parser.add_argument("directory", type=Path)
    check_parser = subparsers.add_parser("check-artifact")
    check_parser.add_argument("ckir", type=Path)
    check_parser.add_argument("elf", type=Path)
    mutate_parser = subparsers.add_parser("mutate-artifact")
    mutate_parser.add_argument("source", type=Path)
    mutate_parser.add_argument("destination", type=Path)
    empty_parser = subparsers.add_parser("check-empty-artifact")
    empty_parser.add_argument("ckir", type=Path)
    empty_parser.add_argument("elf", type=Path)
    args = parser.parse_args()
    if args.command == "emit":
        emit(args.directory)
    elif args.command == "check-artifact":
        print(check_artifact(args.ckir, args.elf))
    elif args.command == "check-empty-artifact":
        print(check_empty_artifact(args.ckir, args.elf))
    else:
        mutate_artifact(args.source, args.destination)


if __name__ == "__main__":
    main()
