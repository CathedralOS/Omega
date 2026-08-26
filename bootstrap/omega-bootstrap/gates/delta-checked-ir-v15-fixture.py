#!/usr/bin/env python3
"""Handcrafted CKIR15 recurrent shared-view carriers and local mutations."""

from __future__ import annotations

import argparse
import copy
import struct
from pathlib import Path

import checked_ir_v15_reference as ir15


NO_ID = ir15.NO_ID


def replace(row: tuple[int, ...], index: int, value: int) -> tuple[int, ...]:
    return row[:index] + (value,) + row[index + 1:]


def tables(byte_values: tuple[int, ...] = (70, 71)) -> dict[str, list[tuple[int, ...]]]:
    result = {name: [] for name in ir15.TABLE_ORDER}
    result["types"] = [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 3, 0, 0, 0, 0, 0, 1),
        (2, 1, 0, 0, 0, 0, 0, 255),
        (3, 7, 0, 0, 2, 0, 0, 0),
    ]
    result["records"] = [(0, 0, 0, 0, 0, 0, 0, 0)]
    result["machines"] = [(0, 0, 2, 0, 0, 2, 0, 0, 0, 6, 0)]
    result["block_params"] = [
        (0, 1, 0, 2, 0), (1, 1, 1, 3, 1), (2, 1, 2, 2, 2),
        (3, 2, 0, 2, 3), (4, 2, 1, 2, 4), (5, 2, 2, 3, 5),
        (6, 2, 3, 2, 6), (7, 3, 0, 2, 7), (8, 3, 1, 2, 8),
        (9, 4, 0, 3, 9), (10, 4, 1, 2, 10), (11, 4, 2, 2, 11),
        (12, 5, 0, 3, 12), (13, 5, 1, 2, 13), (14, 5, 2, 2, 14),
    ]
    result["blocks"] = [
        (0, 0, 2, 0, 0, 0, 0, 0, 3, 0),
        (1, 0, 2, 0, 0, 0, 3, 3, 1, 1),
        (2, 0, 2, 0, 0, 3, 4, 4, 1, 2),
        (3, 0, 2, 0, 0, 7, 2, 5, 1, 3),
        (4, 0, 2, 1, 0, 9, 3, 6, 2, 4),
        (5, 0, 2, 1, 0, 12, 3, 8, 2, 5),
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
        (0, 0, 0, 22, 1, 0, 15, 3, 0, 0, root_id, 0),
        (1, 0, 0, 1, 1, 0, 16, 2, 0, 0, 69, 0),
        (2, 0, 0, 1, 1, 0, 17, 2, 0, 0, 68, 0),
        (3, 0, 1, 23, 1, 0, 18, 1, 0, 1, 0, 0),
        (4, 0, 2, 23, 1, 0, 19, 1, 1, 1, 0, 0),
        (5, 0, 3, 1, 1, 0, 20, 2, 2, 0, 70, 0),
        (6, 0, 4, 24, 1, 0, 21, 2, 2, 1, 0, 0),
        (7, 0, 4, 25, 1, 0, 22, 3, 3, 1, 0, 0),
        (8, 0, 5, 24, 1, 0, 23, 2, 4, 1, 0, 0),
        (9, 0, 5, 25, 1, 0, 24, 3, 5, 1, 0, 0),
    ]
    result["operands"] = [
        (1,), (5,), (9,), (9,), (12,), (12,),
        (16,), (15,), (17,),
        (1,), (0,), (2,), (0,), (2,),
        (5,), (3,), (6,), (3,), (6,),
        (10,), (21,), (22,), (11,),
        (23,), (13,), (24,), (14,),
    ]
    result["terminators"] = [
        (0, 0, 0, 1, 0, 0, NO_ID, 1, 6, 3, NO_ID, 9, 0, 0, 0),
        (1, 0, 1, 2, 0, 0, 18, 4, 9, 3, 3, 12, 2, 0, 0),
        (2, 0, 2, 2, 0, 0, 19, 5, 14, 3, 3, 17, 2, 0, 0),
        (3, 0, 3, 4, 0, 0, 20, NO_ID, 19, 0, NO_ID, 19, 0, 0, 0),
        (4, 0, 4, 1, 0, 0, NO_ID, 2, 19, 4, NO_ID, 23, 0, 0, 0),
        (5, 0, 5, 1, 0, 0, NO_ID, 2, 23, 4, NO_ID, 27, 0, 0, 0),
    ]
    return result


def encode(raw_tables: dict[str, list[tuple[int, ...]]], *, major: int = 15,
           values: int = 25, places: int = 0, entry: int = 0) -> bytes:
    counts = {name: len(raw_tables[name]) for name in ir15.TABLE_ORDER}
    counts.update(values=values, places=places)
    payload = b"".join(
        ir15.ROWS[name].pack(*row)
        for name in ir15.TABLE_ORDER
        for row in raw_tables[name]
    )
    return ir15.HEADER.pack(
        b"OMGCKIR\0", major, 0, 1, int(entry != NO_ID), entry,
        ir15.HEADER.size + len(payload),
        *(counts[name] for name in ir15.COUNT_NAMES),
    ) + payload


def mutate_count(contents: bytes, name: str, value: int) -> bytes:
    changed = bytearray(contents)
    struct.pack_into("<I", changed, 24 + 4 * ir15.COUNT_NAMES.index(name), value)
    return bytes(changed)


def runtime_parameter_tables() -> dict[str, list[tuple[int, ...]]]:
    """A library-shaped carrier whose guarded view has no static root.

    The machine's exact `(view, prefix, suffix)` runtime parameters feed the
    first authored state. Removing the three entry constants and adding the
    three machine binders leaves every later value ID dense.
    """
    result = copy.deepcopy(tables((70, 71)))
    result["constants"] = []
    result["constant_children"] = []
    result["machine_params"] = [
        (0, 0, 0, 3, 0),
        (1, 0, 1, 2, 1),
        (2, 0, 2, 2, 2),
    ]
    result["machines"][0] = replace(result["machines"][0], 7, 3)

    def value_id(old: int) -> int:
        if old == 15:
            return 0
        if old == 16:
            return 1
        if old == 17:
            return 2
        return old + 3 if old < 15 else old

    result["block_params"] = [replace(row, 4, value_id(row[4]))
                              for row in result["block_params"]]
    operations = []
    for new_id, row in enumerate(result["operations"][3:]):
        operations.append(replace(replace(row, 0, new_id), 6, value_id(row[6])))
    result["operations"] = operations
    result["operands"] = [(value_id(row[0]),) for row in result["operands"]]
    result["blocks"][0] = replace(result["blocks"][0], 8, 0)
    for block_id in range(1, len(result["blocks"])):
        result["blocks"][block_id] = replace(
            result["blocks"][block_id], 7,
            result["blocks"][block_id][7] - 3,
        )
    result["terminators"] = [
        replace(row, 6, value_id(row[6])) if row[6] != NO_ID else row
        for row in result["terminators"]
    ]
    return result


def arithmetic_composition_tables() -> dict[str, list[tuple[int, ...]]]:
    """Retain both generalized edges while executing the full CKIR14 trio."""
    result = copy.deepcopy(tables((70, 71)))
    full_type = len(result["types"])
    result["types"].append(
        (full_type, 2, 1, 0, 0, 0, 0, 0xFFFF_FFFF)
    )

    def shifted_value(old: int) -> int:
        return old + 6 if old >= 21 else old

    # Operation operands precede all terminator operands. Insert the six
    # arithmetic operands after block 3's original constant and before both
    # synthetic blocks' head/tail operands.
    result["operands"] = [
        *( (shifted_value(row[0]),) for row in result["operands"][:2] ),
        (21,), (22,), (23,), (21,), (24,), (25,),
        *( (shifted_value(row[0]),) for row in result["operands"][2:] ),
    ]
    original = result["operations"]
    arithmetic = [
        (6, 0, 3, 1, 1, 0, 21, full_type, 2, 0, 1, 0),
        (7, 0, 3, 1, 1, 0, 22, full_type, 2, 0, 2, 0),
        (8, 0, 3, 8, 1, 0, 23, full_type, 2, 2, 0, 0),
        (9, 0, 3, 26, 1, 0, 24, full_type, 4, 2, 0, 0),
        (10, 0, 3, 1, 1, 0, 25, full_type, 6, 0, 35, 0),
        (11, 0, 3, 27, 1, 0, 26, full_type, 6, 2, 0, 0),
    ]
    shifted_operations = []
    for row in original[6:]:
        changed = replace(row, 0, row[0] + 6)
        changed = replace(changed, 6, shifted_value(row[6]))
        changed = replace(changed, 8, row[8] + 6)
        shifted_operations.append(changed)
    result["operations"] = original[:6] + arithmetic + shifted_operations
    result["blocks"][3] = replace(result["blocks"][3], 8, 7)
    result["blocks"][4] = replace(result["blocks"][4], 7, 12)
    result["blocks"][5] = replace(result["blocks"][5], 7, 14)
    changed_terms = []
    for row in result["terminators"]:
        changed = replace(row, 6, shifted_value(row[6])) if row[6] != NO_ID else row
        changed = replace(changed, 8, changed[8] + 6)
        changed = replace(changed, 11, changed[11] + 6)
        changed_terms.append(changed)
    result["terminators"] = changed_terms
    return result


def emit(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    positives = {
        "two-byte-recurrent": (tables((70, 71)), {}, "70"),
        "one-byte": (tables((70,)), {}, "70"),
        "empty": (tables(()), {}, "70"),
        "runtime-parameter-library": (
            runtime_parameter_tables(), {"entry": NO_ID}, "library",
        ),
        "view-with-arithmetic": (
            arithmetic_composition_tables(), {"values": 31}, "70",
        ),
    }
    for name, (value, options, _) in positives.items():
        (directory / f"{name}.ckir15").write_bytes(encode(value, **options))
    canonical = encode(positives["two-byte-recurrent"][0])
    (directory / "canonical.ckir15").write_bytes(canonical)
    base = positives["two-byte-recurrent"][0]
    runtime = positives["runtime-parameter-library"][0]
    arithmetic = positives["view-with-arithmetic"][0]
    manifest: list[tuple[str, int]] = []

    def mutation(name: str, change, expected_status: int = 251,
                 major: int = 15, *, source=base, values: int = 25,
                 entry: int = 0) -> None:
        changed = copy.deepcopy(source)
        change(changed)
        (directory / f"{name}.ckir15").write_bytes(
            encode(changed, major=major, values=values, entry=entry)
        )
        manifest.append((name, expected_status))

    mutation("retired-schema-14", lambda _: None, major=14)
    mutation("only-one-synthetic", lambda t: t["blocks"].__setitem__(5,
        replace(t["blocks"][5], 3, 0)))
    mutation("unknown-block-flag", lambda t: t["blocks"].__setitem__(5,
        replace(t["blocks"][5], 3, 2)))
    mutation("synthetic-leading-type", lambda t: t["block_params"].__setitem__(9,
        replace(t["block_params"][9], 3, 1)))
    mutation("synthetic-pass-exact-type", lambda t: (
        t["types"].append((4, 1, 0, 0, 0, 0, 0, 127)),
        t["block_params"].__setitem__(10,
            replace(t["block_params"][10], 3, 4)),
    ))
    mutation("false-pass-exact-type", lambda t: (
        t["types"].append((4, 1, 0, 0, 0, 0, 0, 127)),
        t["block_params"].__setitem__(7,
            replace(t["block_params"][7], 3, 4)),
    ))
    mutation("jump-pass-exact-type", lambda t: (
        t["types"].append((4, 1, 0, 0, 0, 0, 0, 127)),
        t["block_params"].__setitem__(3,
            replace(t["block_params"][3], 3, 4)),
    ))
    mutation("synthetic-on-false-edge", lambda t: t["terminators"].__setitem__(1,
        replace(t["terminators"][1], 10, 4)))
    mutation("synthetic-condition", lambda t: t["terminators"].__setitem__(1,
        replace(t["terminators"][1], 6, 0)))
    mutation("wrong-tested-slice", lambda t: t["operands"].__setitem__(9, (5,)))
    mutation("cross-site-partial-binder", lambda t: t["operands"].__setitem__(2,
        (12,)))
    mutation("duplicate-head", lambda t: t["operations"].__setitem__(7,
        replace(t["operations"][7], 3, 24)))
    mutation("reversed-head-tail-operations", lambda t: (
        t["operations"].__setitem__(6,
            replace(replace(t["operations"][6], 3, 25), 7, 3)),
        t["operations"].__setitem__(7,
            replace(replace(t["operations"][7], 3, 24), 7, 2)),
    ))
    mutation("synthetic-extra-operation", lambda t: t["blocks"].__setitem__(4,
        replace(t["blocks"][4], 8, 1)))
    mutation("synthetic-target-synthetic", lambda t: t["terminators"].__setitem__(4,
        replace(t["terminators"][4], 7, 5)))
    mutation("pass-through-reordered", lambda t: (
        t["operands"].__setitem__(19, (11,)),
        t["operands"].__setitem__(22, (10,)),
    ))
    mutation("pass-through-duplicated", lambda t: t["operands"].__setitem__(22, (10,)))
    mutation("duplicate-pass-binder", lambda t: (
        t["operands"].__setitem__(11, (0,)),
        t["operands"].__setitem__(13, (0,)),
    ))
    mutation("computed-pass-expression", lambda t: (
        t["operands"].__setitem__(11, (18,)),
        t["operands"].__setitem__(13, (18,)),
    ))
    mutation("head-result-duplicated", lambda t: t["operands"].__setitem__(22, (21,)))
    mutation("tail-result-omitted", lambda t: t["operands"].__setitem__(21, (21,)))
    mutation("false-edge-partial", lambda t: t["operands"].__setitem__(12, (21,)))
    mutation("literal-child-type", lambda t: t["constants"].__setitem__(0,
        replace(t["constants"][0], 1, 1)))
    mutation("wrong-value-result-id", lambda t: t["operations"].__setitem__(0,
        replace(t["operations"][0], 6, 16)))

    # The runtime-origin carrier must remain valid without opcode 22, while
    # direct-binder and local ownership checks remain just as strict.
    mutation("runtime-computed-pass", lambda t: (
        t["operands"].__setitem__(10, (18,)),
        t["operands"].__setitem__(12, (18,)),
    ), source=runtime, entry=NO_ID)
    mutation("runtime-false-order", lambda t: (
        t["operands"].__setitem__(12, (5,)),
        t["operands"].__setitem__(13, (3,)),
    ), source=runtime, entry=NO_ID)
    mutation("runtime-cross-site-partial", lambda t: t["operands"].__setitem__(2,
        (15,)), source=runtime, entry=NO_ID)

    # Arithmetic is optional, but once selected its CKIR14 carrier and the
    # generalized view invariants are both still mandatory.
    mutation("arithmetic-noncanonical-carrier", lambda t: t["types"].__setitem__(4,
        replace(t["types"][4], 7, 0x7FFF_FFFF)), source=arithmetic, values=31)
    mutation("arithmetic-view-order", lambda t: (
        t["operands"].__setitem__(25, (11,)),
        t["operands"].__setitem__(28, (10,)),
    ), source=arithmetic, values=31)
    mutation("arithmetic-only-one-synthetic", lambda t: t["blocks"].__setitem__(5,
        replace(t["blocks"][5], 3, 0)), source=arithmetic, values=31)

    over = tables(tuple(range(33)))
    (directory / "literal-33-children.ckir15").write_bytes(encode(over))
    manifest.append(("literal-33-children", 252))
    (directory / "constants-over.ckir15").write_bytes(
        mutate_count(canonical, "constants", 8_193)
    )
    manifest.append(("constants-over", 252))

    (directory / "positives.tsv").write_text(
        "".join(f"{name}\t{expected}\n"
                for name, (_, _, expected) in positives.items()), encoding="ascii"
    )
    (directory / "manifest.tsv").write_text(
        "".join(f"{name}\t{status}\n" for name, status in manifest), encoding="ascii"
    )


def check(path: Path) -> None:
    module = ir15.decode(path.read_bytes())
    ir15.v5.require(module.layouts[3] == (16, 8),
                    "shared byte-view private layout")
    counts = ir15.selected_counts(module)
    ir15.v5.require(
        counts[23] == counts[24] == counts[25] == 2
        and counts[22] in (0, 1),
        "recurrent shared-view operation counts",
    )
    ir15.v5.require(sum(block[3] == 1 for block in module.tables["blocks"]) == 2,
                    "recurrent synthetic block count")
    arithmetic = ir15.selected_arithmetic_counts(module)
    if counts[22] == 0:
        ir15.v5.require(module.entry == NO_ID and not module.tables["constants"],
                        "runtime-parameter library has no static view root")
        ir15.v5.require(ir15.interpret(module) is None,
                        "runtime-parameter library observation")
    else:
        ir15.v5.require(ir15.interpret(module) == 70,
                        "recurrent shared-view carrier result")
    if any(arithmetic.values()):
        ir15.v5.require(arithmetic == {8: 1, 26: 1, 27: 1},
                        "optional complete arithmetic composition")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("emit", "check"))
    parser.add_argument("path", type=Path)
    args = parser.parse_args()
    emit(args.path) if args.command == "emit" else check(args.path)


if __name__ == "__main__":
    main()
