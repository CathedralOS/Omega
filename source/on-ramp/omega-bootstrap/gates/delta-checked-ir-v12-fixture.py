#!/usr/bin/env python3
"""Handcrafted CKIR12 shared static-byte-view carriers and mutations."""

from __future__ import annotations

import argparse
import copy
import struct
from pathlib import Path

import checked_ir_v12_reference as ir12


NO_ID = ir12.NO_ID


def replace(row: tuple[int, ...], index: int, value: int) -> tuple[int, ...]:
    return row[:index] + (value,) + row[index + 1:]


def tables(byte_values: tuple[int, ...] = (70,)) -> dict[str, list[tuple[int, ...]]]:
    result = {name: [] for name in ir12.TABLE_ORDER}
    result["types"] = [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 3, 0, 0, 0, 0, 0, 1),
        (2, 1, 0, 0, 0, 0, 0, 255),
        (3, 7, 0, 0, 2, 0, 0, 0),
    ]
    result["records"] = [(0, 0, 0, 0, 0, 0, 0, 0)]
    result["machines"] = [(0, 0, 2, 0, 0, 2, 0, 0, 0, 6, 0)]
    result["block_params"] = [
        (0, 1, 0, 3, 0),
        (1, 3, 0, 2, 1),
        (2, 3, 1, 3, 2),
        (3, 5, 0, 2, 3),
    ]
    result["blocks"] = [
        (0, 0, 2, 0, 0, 0, 0, 0, 2, 0),
        (1, 0, 2, 1, 0, 0, 1, 2, 2, 1),
        (2, 0, 2, 0, 0, 1, 0, 4, 1, 2),
        (3, 0, 2, 0, 0, 1, 2, 5, 1, 3),
        (4, 0, 2, 0, 0, 3, 0, 6, 1, 4),
        (5, 0, 2, 0, 0, 3, 1, 7, 0, 5),
    ]

    scalar_nodes = [
        (index, 2, 0, 0, value, 0)
        for index, value in enumerate(byte_values)
    ]
    root_id = len(scalar_nodes)
    result["constants"] = scalar_nodes + [
        (root_id, 3, 0, len(byte_values), 0, 0),
    ]
    result["constant_children"] = [(index,) for index in range(len(byte_values))]

    result["operations"] = [
        (0, 0, 0, 22, 1, 0, 4, 3, 0, 0, root_id, 0),
        (1, 0, 0, 23, 1, 0, 5, 1, 0, 1, 0, 0),
        (2, 0, 1, 24, 1, 0, 6, 2, 1, 1, 0, 0),
        (3, 0, 1, 25, 1, 0, 7, 3, 2, 1, 0, 0),
        (4, 0, 2, 1, 1, 0, 8, 2, 3, 0, 70, 0),
        (5, 0, 3, 23, 1, 0, 9, 1, 3, 1, 0, 0),
        (6, 0, 4, 1, 1, 0, 10, 2, 4, 0, 0, 0),
    ]
    result["operands"] = [
        (4,), (0,), (0,), (2,),
        (4,), (6,), (7,), (1,),
    ]
    result["terminators"] = [
        (0, 0, 0, 2, 0, 0, 5, 1, 4, 1, 2, 5, 0, 0, 0),
        (1, 0, 1, 1, 0, 0, NO_ID, 3, 5, 2, NO_ID, 7, 0, 0, 0),
        (2, 0, 2, 4, 0, 0, 8, NO_ID, 7, 0, NO_ID, 7, 0, 0, 0),
        (3, 0, 3, 2, 0, 0, 9, 4, 7, 0, 5, 7, 1, 0, 0),
        (4, 0, 4, 4, 0, 0, 10, NO_ID, 8, 0, NO_ID, 8, 0, 0, 0),
        (5, 0, 5, 4, 0, 0, 3, NO_ID, 8, 0, NO_ID, 8, 0, 0, 0),
    ]
    return result


def encode(raw_tables: dict[str, list[tuple[int, ...]]], *, major: int = 12,
           values: int = 11, places: int = 0) -> bytes:
    counts = {name: len(raw_tables[name]) for name in ir12.TABLE_ORDER}
    counts.update(values=values, places=places)
    payload = b"".join(
        ir12.ROWS[name].pack(*row)
        for name in ir12.TABLE_ORDER
        for row in raw_tables[name]
    )
    return ir12.HEADER.pack(
        b"OMGCKIR\0", major, 0, 1, 1, 0, ir12.HEADER.size + len(payload),
        *(counts[name] for name in ir12.COUNT_NAMES),
    ) + payload


def mutate_count(contents: bytes, name: str, value: int) -> bytes:
    changed = bytearray(contents)
    struct.pack_into("<I", changed, 24 + 4 * ir12.COUNT_NAMES.index(name), value)
    return bytes(changed)


def emit(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    positives = {
        "one-byte": tables((70,)),
        "empty": tables(()),
    }
    for name, value in positives.items():
        (directory / f"{name}.ckir12").write_bytes(encode(value))
    canonical = encode(positives["one-byte"])
    (directory / "canonical.ckir12").write_bytes(canonical)

    base = positives["one-byte"]
    manifest: list[tuple[str, int]] = []

    def mutation(name: str, change, expected_status: int = 251,
                 major: int = 12) -> None:
        changed = copy.deepcopy(base)
        change(changed)
        (directory / f"{name}.ckir12").write_bytes(encode(changed, major=major))
        manifest.append((name, expected_status))

    mutation("old-schema-major-11", lambda _: None, major=11)
    mutation("slice-type-flag", lambda t: t["types"].__setitem__(3, replace(t["types"][3], 2, 1)))
    mutation("slice-element-bool", lambda t: t["types"].__setitem__(3, replace(t["types"][3], 4, 1)))
    mutation("slice-payload-one", lambda t: t["types"].__setitem__(3, replace(t["types"][3], 5, 1)))
    mutation("slice-low", lambda t: t["types"].__setitem__(3, replace(t["types"][3], 6, 1)))
    mutation("slice-high", lambda t: t["types"].__setitem__(3, replace(t["types"][3], 7, 1)))
    mutation("non-full-u8-element", lambda t: t["types"].__setitem__(2, replace(t["types"][2], 7, 254)))
    mutation("literal-scalar", lambda t: t["constants"].__setitem__(1, replace(t["constants"][1], 4, 1)))
    mutation("literal-child-type", lambda t: t["constants"].__setitem__(0, replace(t["constants"][0], 1, 1)))
    mutation("literal-child-order", lambda t: t["constant_children"].__setitem__(0, (1,)))
    mutation("static-root-scalar", lambda t: t["operations"].__setitem__(0, replace(t["operations"][0], 10, 0)))
    mutation("static-immediate-one", lambda t: t["operations"].__setitem__(0, replace(t["operations"][0], 11, 1)))
    mutation("static-result-u8", lambda t: t["operations"].__setitem__(0, replace(t["operations"][0], 7, 2)))
    mutation("static-has-operand", lambda t: t["operations"].__setitem__(0, replace(t["operations"][0], 9, 1)))
    mutation("constant-copy-slice-root", lambda t: t["operations"].__setitem__(0, replace(t["operations"][0], 3, 11)))
    mutation("nonempty-result-u8", lambda t: t["operations"].__setitem__(1, replace(t["operations"][1], 7, 2)))
    mutation("nonempty-invisible-source", lambda t: t["operands"].__setitem__(0, (0,)))
    mutation("head-result-bool", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 7, 1)))
    mutation("tail-result-u8", lambda t: t["operations"].__setitem__(3, replace(t["operations"][3], 7, 2)))
    mutation("byte-op-immediate", lambda t: t["operations"].__setitem__(3, replace(t["operations"][3], 10, 1)))
    mutation("missing-synthetic-flag", lambda t: t["blocks"].__setitem__(1, replace(t["blocks"][1], 3, 0)))
    mutation("unknown-block-flag", lambda t: t["blocks"].__setitem__(1, replace(t["blocks"][1], 3, 2)))
    mutation("multiple-synthetic-blocks", lambda t: t["blocks"].__setitem__(2, replace(t["blocks"][2], 3, 1)))
    mutation("synthetic-wrong-param-type", lambda t: t["block_params"].__setitem__(0, replace(t["block_params"][0], 3, 2)))
    mutation("synthetic-on-false-edge", lambda t: t["terminators"].__setitem__(0,
        (0, 0, 0, 2, 0, 0, 5, 2, 4, 0, 1, 4, 1, 0, 0)))
    mutation("synthetic-condition-not-nonempty", lambda t: t["terminators"].__setitem__(0, replace(t["terminators"][0], 6, 4)))
    mutation("synthetic-operation-not-head-tail", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 3, 23)))
    mutation("synthetic-not-jump", lambda t: t["terminators"].__setitem__(1, replace(t["terminators"][1], 3, 2)))
    mutation("synthetic-target-is-synthetic", lambda t: t["terminators"].__setitem__(1, replace(t["terminators"][1], 7, 1)))
    mutation("wrong-value-result-id", lambda t: t["operations"].__setitem__(0, replace(t["operations"][0], 6, 5)))
    mutation("wrong-edge-slice", lambda t: t["operands"].__setitem__(4, (5,)))
    mutation("missing-tail-opcode", lambda t: t["operations"].__setitem__(3, replace(t["operations"][3], 3, 24)))

    over = tables(tuple(range(33)))
    (directory / "literal-33-children.ckir12").write_bytes(encode(over))
    manifest.append(("literal-33-children", 252))
    (directory / "constants-over.ckir12").write_bytes(
        mutate_count(canonical, "constants", 8_193)
    )
    manifest.append(("constants-over", 252))

    (directory / "positives.tsv").write_text(
        "".join(f"{name}\t70\n" for name in positives), encoding="ascii"
    )
    (directory / "manifest.tsv").write_text(
        "".join(f"{name}\t{status}\n" for name, status in manifest), encoding="ascii"
    )


def check(path: Path) -> None:
    module = ir12.decode(path.read_bytes())
    ir12.v5.require(module.layouts[3] == (16, 8), "shared byte-view private layout")
    ir12.v5.require(ir12.selected_counts(module) == {22: 1, 23: 2, 24: 1, 25: 1},
                    "shared byte-view operation counts")
    ir12.v5.require(ir12.interpret(module) == 70,
                    "shared byte-view carrier result")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("emit", "check"))
    parser.add_argument("path", type=Path)
    args = parser.parse_args()
    emit(args.path) if args.command == "emit" else check(args.path)


if __name__ == "__main__":
    main()
