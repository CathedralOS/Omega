#!/usr/bin/env python3
"""Backend-local CKIR14 full-width arithmetic carriers and artifact checks."""

from __future__ import annotations

import argparse
import re
import struct
from pathlib import Path

import checked_ir_v5_reference as v5
import checked_ir_v14_reference as ir14
import checked_ir_v14_test_support as support


NO_ID = ir14.NO_ID
FULL_TYPE = 4
LEGACY_TRAPPING_TYPE = 5
FULL_EDGE_WORDS = {0, 1, 0x7FFF_FFFF, 0x8000_0000, 0xFFFF_FFFF}


def blank() -> dict[str, list[tuple[int, ...]]]:
    result = {name: [] for name in ir14.TABLE_ORDER}
    result["types"] = [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 3, 0, 0, 0, 0, 0, 1),
        (2, 2, 0, 0, 0, 0, 0, 0xFFFF_FFFF),
        (3, 1, 0, 0, 0, 0, 0, 255),
        (4, 2, 1, 0, 0, 0, 0, 0xFFFF_FFFF),
        (5, 2, 1, 0, 0, 0, 0, 0x7FFF_FFFF),
    ]
    result["records"] = [(0, 0, 0, 0, 0, 0, 0, 0)]
    return result


def encode(tables: dict[str, list[tuple[int, ...]]], *, values: int,
           places: int = 0, major: int = 14) -> bytes:
    counts = {name: len(tables[name]) for name in ir14.TABLE_ORDER}
    counts.update(values=values, places=places)
    payload = b"".join(
        ir14.ROWS[name].pack(*row)
        for name in ir14.TABLE_ORDER
        for row in tables[name]
    )
    return ir14.HEADER.pack(
        b"OMGCKIR\0", major, 0, 1, 1, 0,
        ir14.HEADER.size + len(payload),
        *(counts[name] for name in ir14.COUNT_NAMES),
    ) + payload


def linear(constants: tuple[int, ...],
           arithmetic: tuple[tuple[int, int, int], ...]) -> bytes:
    tables = blank()
    operations: list[tuple[int, ...]] = []
    operands: list[tuple[int, ...]] = []
    for value in constants:
        operations.append((
            len(operations), 0, 0, 1, 1, 0, len(operations), FULL_TYPE,
            len(operands), 0, value, 0,
        ))
    for opcode, left, right in arithmetic:
        operations.append((
            len(operations), 0, 0, opcode, 1, 0, len(operations), FULL_TYPE,
            len(operands), 2, 0, 0,
        ))
        operands.extend(((left,), (right,)))
    result = len(operations) - 1
    tables["machines"] = [(0, 0, 2, 0, 0, FULL_TYPE, 0, 0, 0, 1, 0)]
    tables["blocks"] = [(0, 0, 2, 0, 0, 0, 0, 0, len(operations), 0)]
    tables["operations"] = operations
    tables["operands"] = operands
    tables["terminators"] = [(
        0, 0, 0, 4, 0, 0, result, NO_ID, len(operands), 0,
        NO_ID, len(operands), 0, 0, 0,
    )]
    return encode(tables, values=len(operations))


def widened_arithmetic(*, legacy_target: bool = False) -> bytes:
    tables = blank()
    target = LEGACY_TRAPPING_TYPE if legacy_target else FULL_TYPE
    tables["machines"] = [(0, 0, 2, 0, 0, FULL_TYPE, 0, 0, 0, 1, 0)]
    tables["blocks"] = [(0, 0, 2, 0, 0, 0, 0, 0, 4, 0)]
    tables["operations"] = [
        (0, 0, 0, 1, 1, 0, 0, 3, 0, 0, 70, 0),
        (1, 0, 0, 21, 1, 0, 1, target, 0, 1, 0, 0),
        (2, 0, 0, 1, 1, 0, 2, FULL_TYPE, 1, 0, 1, 0),
        (3, 0, 0, 8, 1, 0, 3, FULL_TYPE, 1, 2, 0, 0),
    ]
    tables["operands"] = [(0,), (1,), (2,)]
    tables["terminators"] = [(
        0, 0, 0, 4, 0, 0, 3, NO_ID, 3, 0, NO_ID, 3, 0, 0, 0,
    )]
    return encode(tables, values=4)


def block_parameter_arithmetic() -> bytes:
    tables = blank()
    tables["machines"] = [(0, 0, 2, 0, 0, FULL_TYPE, 0, 0, 0, 2, 0)]
    tables["block_params"] = [(0, 1, 0, FULL_TYPE, 0)]
    tables["blocks"] = [
        (0, 0, 2, 0, 0, 0, 0, 0, 1, 0),
        (1, 0, 2, 0, 0, 0, 1, 1, 2, 1),
    ]
    tables["operations"] = [
        (0, 0, 0, 1, 1, 0, 1, FULL_TYPE, 0, 0, 69, 0),
        (1, 0, 1, 1, 1, 0, 2, FULL_TYPE, 0, 0, 1, 0),
        (2, 0, 1, 8, 1, 0, 3, FULL_TYPE, 0, 2, 0, 0),
    ]
    tables["operands"] = [(0,), (2,), (1,)]
    tables["terminators"] = [
        (0, 0, 0, 1, 0, 0, NO_ID, 1, 2, 1, NO_ID, 3, 0, 0, 0),
        (1, 0, 1, 4, 0, 0, 3, NO_ID, 3, 0, NO_ID, 3, 0, 0, 0),
    ]
    return encode(tables, values=4)


def machine_parameter_arithmetic() -> bytes:
    tables = blank()
    tables["machines"] = [
        (0, 0, 2, 0, 0, FULL_TYPE, 0, 0, 0, 1, 0),
        (1, 0, 2, 0, 0, FULL_TYPE, 0, 1, 1, 1, 1),
    ]
    tables["machine_params"] = [(0, 1, 0, FULL_TYPE, 0)]
    tables["blocks"] = [
        (0, 0, 2, 0, 0, 0, 0, 0, 3, 0),
        (1, 1, 2, 0, 0, 0, 0, 3, 2, 1),
    ]
    tables["operations"] = [
        (0, 0, 0, 2, 2, 0, 0, 0, 0, 0, 0, 0),
        (1, 0, 0, 1, 1, 0, 1, FULL_TYPE, 0, 0, 69, 0),
        (2, 0, 0, 10, 1, 0, 2, FULL_TYPE, 0, 2, 1, 0),
        (3, 1, 1, 1, 1, 0, 3, FULL_TYPE, 2, 0, 1, 0),
        (4, 1, 1, 8, 1, 0, 4, FULL_TYPE, 2, 2, 0, 0),
    ]
    tables["operands"] = [(0,), (1,), (0,), (3,)]
    tables["terminators"] = [
        (0, 0, 0, 4, 0, 0, 2, NO_ID, 4, 0, NO_ID, 4, 0, 0, 0),
        (1, 1, 1, 4, 0, 0, 4, NO_ID, 4, 0, NO_ID, 4, 0, 0, 0),
    ]
    return encode(tables, values=5, places=1)


def call_result_custody_failure() -> bytes:
    """A visible full-u32 call result is not an admitted arithmetic leaf."""
    tables = blank()
    tables["machines"] = [
        (0, 0, 2, 0, 0, FULL_TYPE, 0, 0, 0, 1, 0),
        (1, 0, 2, 0, 0, FULL_TYPE, 0, 0, 1, 1, 1),
    ]
    tables["blocks"] = [
        (0, 0, 2, 0, 0, 0, 0, 0, 4, 0),
        (1, 1, 2, 0, 0, 0, 0, 4, 1, 1),
    ]
    tables["operations"] = [
        (0, 0, 0, 2, 2, 0, 0, 0, 0, 0, 0, 0),
        (1, 0, 0, 10, 1, 0, 0, FULL_TYPE, 0, 1, 1, 0),
        (2, 0, 0, 1, 1, 0, 1, FULL_TYPE, 1, 0, 1, 0),
        (3, 0, 0, 8, 1, 0, 2, FULL_TYPE, 1, 2, 0, 0),
        (4, 1, 1, 1, 1, 0, 3, FULL_TYPE, 3, 0, 70, 0),
    ]
    tables["operands"] = [(0,), (0,), (1,)]
    tables["terminators"] = [
        (0, 0, 0, 4, 0, 0, 2, NO_ID, 3, 0, NO_ID, 3, 0, 0, 0),
        (1, 1, 1, 4, 0, 0, 3, NO_ID, 3, 0, NO_ID, 3, 0, 0, 0),
    ]
    return encode(tables, values=4, places=1)


def carriers() -> dict[str, tuple[bytes, str]]:
    result = {
        "add-zero-one": (linear((0, 1), ((8, 0, 1),)), "ok:1"),
        "add-high-boundary": (
            linear((0x7FFF_FFFF, 1), ((8, 0, 1),)), "ok:2147483648",
        ),
        "add-full-success": (
            linear((0xFFFF_FFFE, 1), ((8, 0, 1),)), "ok:4294967295",
        ),
        "subtract-high-boundary": (
            linear((0xFFFF_FFFF, 0x7FFF_FFFF), ((26, 0, 1),)),
            "ok:2147483648",
        ),
        "subtract-high-word": (
            linear((0x8000_0000, 0), ((26, 0, 1),)), "ok:2147483648",
        ),
        "subtract-to-zero": (linear((1, 1), ((26, 0, 1),)), "ok:0"),
        "multiply-zero-full": (
            linear((0, 0xFFFF_FFFF), ((27, 0, 1),)), "ok:0",
        ),
        "multiply-one-full": (
            linear((1, 0xFFFF_FFFF), ((27, 0, 1),)), "ok:4294967295",
        ),
        "multiply-full-success": (
            linear((65_535, 65_537), ((27, 0, 1),)), "ok:4294967295",
        ),
        "recursive-mixed": (
            linear(
                (0xFFFF_FFFF, 0xFFFF_FFFA, 13, 5),
                ((26, 0, 1), (27, 2, 3), (8, 4, 5)),
            ),
            "ok:70",
        ),
        "widen-into-arithmetic": (widened_arithmetic(), "ok:71"),
        "block-parameter-arithmetic": (block_parameter_arithmetic(), "ok:70"),
        "machine-parameter-arithmetic": (machine_parameter_arithmetic(), "ok:70"),
        "composed-view-arithmetic": (
            support.composed_view_and_arithmetic(), "ok:70",
        ),
        "add-overflow": (
            linear((0xFFFF_FFFF, 1), ((8, 0, 1),)), "trap:add",
        ),
        "subtract-underflow": (linear((0, 1), ((26, 0, 1),)), "trap:subtract"),
        "multiply-overflow": (
            linear((65_536, 65_536), ((27, 0, 1),)), "trap:multiply",
        ),
    }
    observed = set()
    for contents, _ in result.values():
        module = ir14.decode(contents)
        for operation in module.tables["operations"]:
            if operation[3] == 1 and module.tables["types"][operation[7]][1] == 2:
                observed.add(operation[10])
    v5.require(FULL_EDGE_WORDS <= observed, "full-u32 edge fixture coverage")
    return result


def invalid_carriers() -> dict[str, bytes]:
    canonical = carriers()["recursive-mixed"][0]
    retired = bytearray(canonical)
    struct.pack_into("<H", retired, 8, 13)
    return {
        "retired-major-13": bytes(retired),
        "missing-arithmetic": support.view_only(),
        "widen-legacy-target": widened_arithmetic(legacy_target=True),
        "call-result-custody": call_result_custody_failure(),
    }


def emit(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    valid = carriers()
    invalid = invalid_carriers()
    for name, (contents, _) in valid.items():
        (directory / f"{name}.ckir14").write_bytes(contents)
    for name, contents in invalid.items():
        (directory / f"{name}.ckir14").write_bytes(contents)
    (directory / "positives.tsv").write_text("".join(
        f"{name}\t{outcome}\n" for name, (_, outcome) in valid.items()
    ))
    (directory / "invalid.tsv").write_text("".join(
        f"{name}\t251\n" for name in invalid
    ))


def check_meaning(path: Path, outcome: str) -> None:
    module = ir14.decode(path.read_bytes())
    kind, expected = outcome.split(":", 1)
    if kind == "ok":
        v5.require(ir14.interpret(module) == int(expected), "CKIR14 result")
        return
    try:
        ir14.interpret(module)
    except v5.Ckir5Error as error:
        v5.require(expected in str(error), "expected arithmetic trap family")
        return
    raise v5.Ckir5Error("expected arithmetic trap")


ADD = re.compile(
    rb"\x8b\x85.{4}\x03\x85.{4}\x0f\x82.{4}"
    rb"\x3d\x00\x00\x00\x00\x0f\x82.{4}"
    rb"\x3d\xff\xff\xff\xff\x0f\x87.{4}\x89\x85.{4}", re.DOTALL,
)
SUBTRACT = re.compile(
    rb"\x8b\x85.{4}\x2b\x85.{4}\x0f\x82.{4}"
    rb"\x3d\x00\x00\x00\x00\x0f\x82.{4}"
    rb"\x3d\xff\xff\xff\xff\x0f\x87.{4}\x89\x85.{4}", re.DOTALL,
)
MULTIPLY = re.compile(
    rb"\x8b\x85.{4}\xf7\xa5.{4}\x85\xd2\x0f\x85.{4}"
    rb"\x3d\x00\x00\x00\x00\x0f\x82.{4}"
    rb"\x3d\xff\xff\xff\xff\x0f\x87.{4}\x89\x85.{4}", re.DOTALL,
)
WIDEN = re.compile(rb"\x8b\x85.{4}\x0f\xb6\xc0\x89\x85.{4}", re.DOTALL)
STATIC = re.compile(
    rb"\x4c\x8d\x9d.{4}\x48\x8d\x05.{4}\x49\x89\x03\xb8.{4}"
    rb"\x49\x89\x43\x08\x4c\x89\x9d.{4}", re.DOTALL,
)
NONEMPTY = re.compile(
    rb"\x4c\x8b\x9d.{4}\x49\x83\x7b\x08\x00\x0f\x95\xc0"
    rb"\x0f\xb6\xc0\x89\x85.{4}", re.DOTALL,
)
HEAD = re.compile(
    rb"\x4c\x8b\x9d.{4}\x49\x83\x7b\x08\x00\x0f\x84.{4}"
    rb"\x49\x8b\x03\x0f\xb6\x00\x89\x85.{4}", re.DOTALL,
)
TAIL = re.compile(
    rb"\x4c\x8b\x9d.{4}\x4c\x8d\x95.{4}\x49\x83\x7b\x08\x00"
    rb"\x0f\x84.{4}\x49\x8b\x03\x48\x83\xc0\x01\x49\x89\x02"
    rb"\x49\x8b\x43\x08\x48\x83\xe8\x01\x49\x89\x42\x08"
    rb"\x4c\x89\x95.{4}", re.DOTALL,
)


def segments(artifact: bytes) -> tuple[bytes, bool]:
    v5.require(artifact.startswith(b"\x7fELF\x02\x01\x01"), "ELF64 header")
    phoff = struct.unpack_from("<Q", artifact, 32)[0]
    phentsize, phnum = struct.unpack_from("<HH", artifact, 54)
    v5.require(phentsize == 56, "ELF64 program-header size")
    executable = None
    readonly = False
    for index in range(phnum):
        row = struct.unpack_from("<IIQQQQQQ", artifact, phoff + index * phentsize)
        kind, flags, offset, _, _, file_size, _, alignment = row
        if kind == 1 and flags == 5:
            v5.require(executable is None and alignment == 4096, "executable segment")
            executable = artifact[offset:offset + file_size]
        elif kind == 1 and flags == 4:
            readonly = True
    v5.require(executable is not None, "missing executable segment")
    return executable, readonly


def branches_reach_trap(text: bytes, match: re.Match[bytes]) -> None:
    found = 0
    for index in range(match.start(), match.end() - 5):
        if text[index] == 0x0F and text[index + 1] in (0x82, 0x85, 0x87):
            displacement = struct.unpack_from("<i", text, index + 2)[0]
            target = index + 6 + displacement
            v5.require(text[target:target + 2] == b"\x0f\x0b", "branch to shared ud2")
            found += 1
    v5.require(found == 3, "carry/borrow/high-half plus range trap branches")


def check_artifact(path: Path, ir_path: Path) -> None:
    artifact = path.read_bytes()
    module = ir14.decode(ir_path.read_bytes())
    text, readonly = segments(artifact)
    patterns = {8: ADD, 21: WIDEN, 22: STATIC, 23: NONEMPTY,
                24: HEAD, 25: TAIL, 26: SUBTRACT, 27: MULTIPLY}
    expected_counts = {opcode: 0 for opcode in patterns}
    cursor = 0
    arithmetic_matches: list[re.Match[bytes]] = []
    for operation in module.tables["operations"]:
        opcode = operation[3]
        if opcode == 1:
            immediate = struct.pack("<I", operation[10])
            pattern = re.compile(b"\xb8" + re.escape(immediate) + rb"\x89\x85.{4}", re.DOTALL)
        elif opcode in patterns:
            pattern = patterns[opcode]
            expected_counts[opcode] += 1
        else:
            continue
        match = pattern.search(text, cursor)
        v5.require(match is not None, f"opcode {opcode} exact emitted template")
        assert match is not None
        cursor = match.end()
        if opcode in (8, 26, 27):
            branches_reach_trap(text, match)
            arithmetic_matches.append(match)
    for opcode, pattern in patterns.items():
        v5.require(len(pattern.findall(text)) == expected_counts[opcode],
                   f"opcode {opcode} exact artifact reconstruction")
    has_view = any(expected_counts[opcode] for opcode in range(22, 26))
    v5.require(readonly == has_view, "optional view read-only segment")
    v5.require(bool(arithmetic_matches), "selected arithmetic artifact")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("emit", "check-ir", "check-artifact"))
    parser.add_argument("path", type=Path)
    parser.add_argument("extra", nargs="?")
    args = parser.parse_args()
    if args.command == "emit":
        emit(args.path)
    elif args.command == "check-ir":
        if args.extra is None:
            parser.error("check-ir requires an outcome")
        check_meaning(args.path, args.extra)
    else:
        if args.extra is None:
            parser.error("check-artifact requires a CKIR path")
        check_artifact(args.path, Path(args.extra))


if __name__ == "__main__":
    main()
