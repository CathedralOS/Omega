#!/usr/bin/env python3
"""Handcrafted CKIR17 checked-adapter library and mutation corpus."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import struct
from pathlib import Path

import checked_ir_v17_reference as ir17


NO_ID = ir17.NO_ID


def replace(row: tuple[int, ...], index: int, value: int) -> tuple[int, ...]:
    return row[:index] + (value,) + row[index + 1:]


def tables() -> dict[str, list[tuple[int, ...]]]:
    result = {name: [] for name in ir17.TABLE_ORDER}
    result["types"] = [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 3, 0, 0, 0, 0, 0, 1),
        (2, 1, 0, 0, 0, 0, 0, 255),
        (3, 7, 0, 0, 2, 0, 0, 0),
        (4, 9, 0, 0, 0, 0, 0x8000_0000, 0x7FFF_FFFF),
        (5, 10, 0, 0, 0, 0, 0, 0),
    ]
    result["records"] = [(0, 0, 0, 0, 0, 0, 0, 0)]
    result["machines"] = [
        (0, NO_ID, 0, ir17.FREE, 0, NO_ID, 0, 3, 0, 7, 0),
        (1, 0, 0, ir17.STATIC_ATTACHED, 0, NO_ID, 3, 2, 7, 1, 7),
        (2, 0, 0, ir17.STATIC_ATTACHED, 0, NO_ID, 5, 2, 8, 1, 8),
    ]
    result["machine_params"] = [
        (0, 0, 0, 5, 0), (1, 0, 1, 3, 1), (2, 0, 2, 1, 2),
        (3, 1, 0, 5, 3), (4, 1, 1, 3, 4),
        (5, 2, 0, 5, 5), (6, 2, 1, 3, 6),
    ]
    result["blocks"] = [
        (0, 0, 0, 0, 0, 0, 0, 0, 1, 0),
        (1, 0, 0, 1, 0, 0, 3, 1, 2, 1),
        (2, 0, 0, 0, 0, 3, 4, 3, 3, 2),
        (3, 0, 0, 1, 0, 7, 3, 6, 2, 3),
        (4, 0, 0, 0, 0, 10, 2, 8, 1, 4),
        (5, 0, 0, 0, 0, 12, 2, 9, 2, 5),
        (6, 0, 0, 0, 0, 14, 0, 11, 0, 6),
        (7, 1, 0, 0, 0, 14, 0, 11, 2, 7),
        (8, 2, 0, 0, 0, 14, 0, 13, 2, 8),
    ]
    result["block_params"] = [
        (0, 1, 0, 5, 7), (1, 1, 1, 3, 8), (2, 1, 2, 1, 9),
        (3, 2, 0, 5, 10), (4, 2, 1, 2, 11),
        (5, 2, 2, 3, 12), (6, 2, 3, 1, 13),
        (7, 3, 0, 5, 14), (8, 3, 1, 3, 15),
        (9, 3, 2, 1, 16),
        (10, 4, 0, 5, 17), (11, 4, 1, 1, 18),
        (12, 5, 0, 5, 19), (13, 5, 1, 2, 20),
    ]
    result["operations"] = [
        (0, 0, 0, 23, 1, 0, 21, 1, 0, 1, 0, 0),
        (1, 0, 1, 24, 1, 0, 22, 2, 1, 1, 0, 0),
        (2, 0, 1, 25, 1, 0, 23, 3, 2, 1, 0, 0),
        (3, 0, 2, 30, 1, 0, 24, 4, 3, 1, 0, 0),
        (4, 0, 2, 29, 0, 0, NO_ID, NO_ID, 4, 2, 0, 0),
        (5, 0, 2, 23, 1, 0, 25, 1, 6, 1, 0, 0),
        (6, 0, 3, 24, 1, 0, 26, 2, 7, 1, 0, 0),
        (7, 0, 3, 25, 1, 0, 27, 3, 8, 1, 0, 0),
        (8, 0, 4, 1, 1, 0, 28, 2, 9, 0, 10, 0),
        (9, 0, 5, 30, 1, 0, 29, 4, 9, 1, 0, 0),
        (10, 0, 5, 29, 0, 0, NO_ID, NO_ID, 10, 2, 0, 0),
        (11, 1, 7, 1, 1, 0, 30, 1, 12, 0, 0, 0),
        (12, 1, 7, 28, 0, 0, NO_ID, NO_ID, 12, 3, 0, 0),
        (13, 2, 8, 1, 1, 0, 31, 1, 15, 0, 1, 0),
        (14, 2, 8, 28, 0, 0, NO_ID, NO_ID, 15, 3, 0, 0),
    ]
    result["operands"] = [(value,) for value in (
        1, 8, 8, 11, 10, 24, 12, 15, 15, 20, 19, 29,
        3, 4, 30, 5, 6, 31,
        0, 1, 2, 0, 2,
        7, 22, 23, 9,
        10, 12, 13, 10, 13,
        14, 26, 27, 16,
        17, 28,
    )]
    result["terminators"] = [
        (0, 0, 0, 2, 0, 0, 21, 1, 18, 3, 4, 21, 2, 0, 0),
        (1, 0, 1, 1, 0, 0, NO_ID, 2, 23, 4, NO_ID, 27, 0, 0, 0),
        (2, 0, 2, 2, 0, 0, 25, 3, 27, 3, 4, 30, 2, 0, 0),
        (3, 0, 3, 1, 0, 0, NO_ID, 2, 32, 4, NO_ID, 36, 0, 0, 0),
        (4, 0, 4, 2, 0, 0, 18, 5, 36, 2, 6, 38, 0, 0, 0),
        (5, 0, 5, 1, 0, 0, NO_ID, 6, 38, 0, NO_ID, 38, 0, 0, 0),
        (6, 0, 6, 3, 0, 0, NO_ID, NO_ID, 38, 0, NO_ID, 38, 0, 0, 0),
        (7, 1, 7, 3, 0, 0, NO_ID, NO_ID, 38, 0, NO_ID, 38, 0, 0, 0),
        (8, 2, 8, 3, 0, 0, NO_ID, NO_ID, 38, 0, NO_ID, 38, 0, 0, 0),
    ]
    result["services"] = [(0, 0, 0, 0, 1, 0)]
    result["machine_reaches"] = [(0, 0, 0), (1, 1, 0), (2, 2, 0)]
    result["rankings"] = [(0, 0, 1, 1, 1)]
    result["boundary_targets"] = [(0, 0, 4, 4, 4, 0, 4, NO_ID, 2)]
    return result


def encode(raw: dict[str, list[tuple[int, ...]]], *, major: int = 17,
           flags: int = 0, entry: int = NO_ID,
           values: int = 32, places: int = 0) -> bytes:
    counts = {name: len(raw[name]) for name in ir17.TABLE_ORDER}
    counts.update(values=values, places=places)
    payload = b"".join(
        ir17.ROWS[name].pack(*row)
        for name in ir17.TABLE_ORDER
        for row in raw[name]
    )
    return ir17.HEADER.pack(
        b"OMGCKIR\0", major, 0, 1, flags, entry,
        ir17.HEADER.size + len(payload),
        *(counts[name] for name in ir17.COUNT_NAMES),
    ) + payload


def mutate_count(contents: bytes, name: str, value: int) -> bytes:
    changed = bytearray(contents)
    struct.pack_into("<I", changed, 24 + 4 * ir17.COUNT_NAMES.index(name), value)
    return bytes(changed)


def emit(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    base = tables()
    canonical = encode(base)
    (directory / "canonical.ckir17").write_bytes(canonical)
    observations = [
        ("write-empty", "write", "", []),
        ("write-one", "write", "46", [70]),
        ("write-line-two", "write_line", "4647", [70, 71, 10]),
        ("write-line-empty", "write_line", "", [10]),
    ]
    (directory / "observations.json").write_text(
        json.dumps(observations, separators=(",", ":")) + "\n",
        encoding="utf-8",
    )
    (directory / "overlong.bin").write_bytes(bytes(65_537))
    manifest: list[tuple[str, int]] = []

    def mutation(name: str, change, status: int = 251, **options: int) -> None:
        changed = copy.deepcopy(base)
        change(changed)
        (directory / f"{name}.ckir17").write_bytes(encode(changed, **options))
        manifest.append((name, status))

    mutation("old-major", lambda _: None, major=16)
    mutation("entry-bearing", lambda _: None, flags=1, entry=1)
    mutation("free-becomes-attached", lambda t: t["machines"].__setitem__(
        0, replace(replace(replace(t["machines"][0], 1, 0), 2, 1), 3, 0)))
    mutation("static-adapter-invents-receiver", lambda t: t["machines"].__setitem__(
        1, replace(t["machines"][1], 2, 1)))
    mutation("service-provider", lambda t: t["services"].__setitem__(
        0, replace(t["services"][0], 2, 1)))
    mutation("missing-reach", lambda t: t["machine_reaches"].pop())
    mutation("duplicate-reach", lambda t: t["machine_reaches"].__setitem__(
        2, (2, 1, 0)))
    mutation("padded-reach", lambda t: t["machine_reaches"].append((3, 2, 0)))
    mutation("ranking-parameter", lambda t: t["rankings"].__setitem__(
        0, replace(t["rankings"][0], 2, 0)))
    mutation("ranking-measure", lambda t: t["rankings"].__setitem__(
        0, replace(t["rankings"][0], 3, 2)))
    mutation("missing-ranking", lambda t: t["rankings"].clear())
    mutation("non-tail-recurrence", lambda t: t["operands"].__setitem__(34, (15,)))
    mutation("synthetic-flag", lambda t: t["blocks"].__setitem__(
        3, replace(t["blocks"][3], 3, 0)))
    mutation("head-tail-swapped", lambda t: (
        t["operations"].__setitem__(6, replace(t["operations"][6], 3, 25)),
        t["operations"].__setitem__(7, replace(t["operations"][7], 3, 24))))
    mutation("boundary-requirement", lambda t: t["boundary_targets"].__setitem__(
        0, replace(t["boundary_targets"][0], 2, 5)))
    mutation("boundary-plan-row", lambda t: t["boundary_targets"].__setitem__(
        0, replace(t["boundary_targets"][0], 3, 3)))
    mutation("boundary-candidate", lambda t: t["boundary_targets"].__setitem__(
        0, replace(t["boundary_targets"][0], 4, 3)))
    mutation("boundary-provider", lambda t: t["boundary_targets"].__setitem__(
        0, replace(t["boundary_targets"][0], 5, 1)))
    mutation("boundary-binding", lambda t: t["boundary_targets"].__setitem__(
        0, replace(t["boundary_targets"][0], 8, 1)))
    mutation("boundary-service-operand", lambda t: t["operands"].__setitem__(4, (12,)))
    mutation("widen-result-unsigned", lambda t: t["operations"].__setitem__(
        3, replace(t["operations"][3], 7, 2)))
    mutation("widen-source-bool", lambda t: t["operands"].__setitem__(3, (13,)))
    mutation("missing-explicit-widen", lambda t: t["operations"].__setitem__(
        3, replace(t["operations"][3], 3, 21)))
    mutation("receiverless-call-target", lambda t: t["operations"].__setitem__(
        12, replace(t["operations"][12], 10, 1)))
    mutation("service-const", lambda t: t["operations"].__setitem__(
        11, replace(t["operations"][11], 7, 5)))

    trailing = canonical + b"\0"
    (directory / "trailing-byte.ckir17").write_bytes(trailing)
    manifest.append(("trailing-byte", 251))
    over = mutate_count(canonical, "services", ir17.CEILINGS["services"] + 1)
    (directory / "service-exhaustion.ckir17").write_bytes(over)
    manifest.append(("service-exhaustion", 252))
    (directory / "manifest.tsv").write_text(
        "".join(f"{name}\t{status}\n" for name, status in manifest),
        encoding="utf-8",
    )
    identity = {
        "bytes": len(canonical),
        "sha256": hashlib.sha256(canonical).hexdigest(),
        "mutations": len(manifest),
    }
    (directory / "identity.json").write_text(
        json.dumps(identity, sort_keys=True, separators=(",", ":")) + "\n",
        encoding="utf-8",
    )


def check(path: Path) -> None:
    module = ir17.decode(path.read_bytes())
    expected = {
        ("write", b""): (),
        ("write", b"F"): (70,),
        ("write_line", b"FG"): (70, 71, 10),
        ("write_line", b""): (10,),
    }
    for arguments, wanted in expected.items():
        actual = ir17.invoke(module, arguments[0], arguments[1])
        if actual != wanted:
            raise ValueError(f"CKIR17 observation {arguments}: {actual} != {wanted}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("emit", "check", "inspect"))
    parser.add_argument("path", type=Path)
    args = parser.parse_args()
    if args.command == "emit":
        emit(args.path)
    elif args.command == "check":
        check(args.path)
    else:
        raw = args.path.read_bytes()
        module = ir17.decode(raw)
        print(json.dumps({
            "bytes": len(raw),
            "sha256": hashlib.sha256(raw).hexdigest(),
            "tables": {name: len(module.tables[name]) for name in ir17.TABLE_ORDER},
            "values": len(module.value_types),
            "places": 0,
        }, sort_keys=True, separators=(",", ":")))


if __name__ == "__main__":
    main()
