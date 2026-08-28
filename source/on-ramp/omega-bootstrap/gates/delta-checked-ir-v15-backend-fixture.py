#!/usr/bin/env python3
"""Backend-local CKIR15 recurrent-view artifact and timeout checks."""

from __future__ import annotations

import argparse
import importlib.util
import re
import struct
import subprocess
from pathlib import Path

import checked_ir_v15_reference as ir15


HERE = Path(__file__).resolve().parent


def load(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, HERE / filename)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


fixture = load("delta_checked_ir_v15_fixture_for_backend",
               "delta-checked-ir-v15-fixture.py")


STATIC = re.compile(
    rb"\x4c\x8d\x9d.{4}\x48\x8d\x05.{4}\x49\x89\x03\xb8(....)"
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
TRUE_EDGE_BRANCH = re.compile(
    rb"\x8b\x85.{4}\x85\xc0\x0f\x84.{4}", re.DOTALL,
)
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


def emit(directory: Path) -> None:
    fixture.emit(directory)


def literal_bytes(module: ir15.Module) -> bytes:
    roots = [row[10] for row in module.tables["operations"] if row[3] == 22]
    ir15.v5.require(len(roots) == 1, "focused StaticByteView root")
    root = module.tables["constants"][roots[0]]
    children = module.tables["constant_children"]
    nodes = module.tables["constants"]
    return bytes(nodes[children[index][0]][4]
                 for index in range(root[2], root[2] + root[3]))


def readonly_segment(artifact: bytes) -> bytes:
    ir15.v5.require(artifact.startswith(b"\x7fELF\x02\x01\x01"), "ELF64 header")
    phoff = struct.unpack_from("<Q", artifact, 32)[0]
    phentsize, phnum = struct.unpack_from("<HH", artifact, 54)
    ir15.v5.require(phentsize == 56, "ELF64 program-header size")
    for index in range(phnum):
        row = struct.unpack_from("<IIQQQQQQ", artifact,
                                 phoff + index * phentsize)
        kind, flags, offset, _, _, file_size, _, alignment = row
        if kind == 1 and flags == 4:
            ir15.v5.require(alignment == 4096 and file_size >= 1,
                           "private read-only literal segment")
            return artifact[offset:offset + file_size]
    raise ir15.Ckir15Error("missing private read-only literal segment")


def check_ir(path: Path, expected: str) -> None:
    fixture.check(path)
    result = ir15.interpret(ir15.decode(path.read_bytes()))
    if expected == "library":
        ir15.v5.require(result is None, "runtime-parameter library observation")
    else:
        ir15.v5.require(result == int(expected), "recurrent runtime result")


def check_produced_ir(path: Path) -> None:
    module = ir15.decode(path.read_bytes())
    ir15.v5.require(ir15.interpret(module) == 70, "produced recurrent result")
    ir15.v5.require(ir15.selected_counts(module) ==
                    {22: 1, 23: 2, 24: 2, 25: 2},
                    "produced recurrent operation counts")
    synthetic = [row for row in module.tables["blocks"] if row[3] == 1]
    ir15.v5.require(len(synthetic) == 2 and all(row[6] == 4 for row in synthetic),
                    "produced four-parameter synthetic edges")
    canonical = module.tables["types"][
        len(module.tables["records"]) + len(module.tables["sums"]) + 1
    ]
    ir15.v5.require(canonical[1:] == (2, 0, 0, 0, 0, 0, 0x7FFF_FFFF),
                    "produced inherited bounded-u32 canonical row")


def check_artifact(path: Path, ir_path: Path) -> None:
    artifact = path.read_bytes()
    module = ir15.decode(ir_path.read_bytes())
    counts = ir15.selected_counts(module)
    arithmetic = ir15.selected_arithmetic_counts(module)
    ir15.v5.require(len(STATIC.findall(artifact)) == counts[22],
                    "exact StaticByteView template count")
    ir15.v5.require(len(NONEMPTY.findall(artifact)) == counts[23],
                    "exact recurrent SliceNonEmpty template count")
    ir15.v5.require(len(HEAD.findall(artifact)) == counts[24] == 2,
                    "exact recurrent SliceHead template count")
    ir15.v5.require(len(TAIL.findall(artifact)) == counts[25] == 2,
                    "exact recurrent SliceTailOne template count")
    ir15.v5.require(len(TRUE_EDGE_BRANCH.findall(artifact)) >= 2,
                    "both false-bypassing true-edge branches")
    ir15.v5.require(len(ADD.findall(artifact)) == arithmetic[8],
                    "optional exact Add templates")
    ir15.v5.require(len(SUBTRACT.findall(artifact)) == arithmetic[26],
                    "optional exact Subtract templates")
    ir15.v5.require(len(MULTIPLY.findall(artifact)) == arithmetic[27],
                    "optional exact Multiply templates")
    expected = literal_bytes(module)
    materialized = expected if expected else b"\0"
    ir15.v5.require(readonly_segment(artifact).startswith(materialized),
                    "program-static literal bytes")


def mutate_second_head(source: Path, destination: Path) -> None:
    raw = bytearray(source.read_bytes())
    matches = list(HEAD.finditer(raw))
    ir15.v5.require(len(matches) == 2, "two source head templates")
    raw[matches[1].start()] ^= 1
    destination.write_bytes(raw)


def run_filter(executable: Path, source: Path, destination: Path,
               expected: int, output_policy: str) -> None:
    with source.open("rb") as input_file, destination.open("wb") as output_file:
        try:
            completed = subprocess.run(
                [str(executable)], stdin=input_file, stdout=output_file,
                timeout=20, check=False,
            )
        except subprocess.TimeoutExpired as error:
            raise ir15.Ckir15Error(
                f"timed out after 20 seconds: {executable.name}"
            ) from error
    ir15.v5.require(
        completed.returncode == expected,
        f"{executable.name} status {completed.returncode}, expected {expected}",
    )
    size = destination.stat().st_size
    if output_policy == "empty":
        ir15.v5.require(size == 0, f"{executable.name} failure publication")
    elif output_policy == "nonempty":
        ir15.v5.require(size > 0, f"{executable.name} expected output")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "command",
        choices=("emit", "check-ir", "check-produced-ir",
                 "check-artifact", "mutate-second-head", "run-filter"),
    )
    parser.add_argument("path", type=Path)
    parser.add_argument("arg1", nargs="?")
    parser.add_argument("arg2", nargs="?")
    parser.add_argument("arg3", nargs="?")
    parser.add_argument("arg4", nargs="?")
    args = parser.parse_args()
    if args.command == "emit":
        emit(args.path)
    elif args.command == "check-ir":
        if args.arg1 is None:
            parser.error("check-ir requires expected result")
        check_ir(args.path, args.arg1)
    elif args.command == "check-produced-ir":
        check_produced_ir(args.path)
    elif args.command == "check-artifact":
        if args.arg1 is None:
            parser.error("check-artifact requires CKIR path")
        check_artifact(args.path, Path(args.arg1))
    elif args.command == "mutate-second-head":
        if args.arg1 is None:
            parser.error("mutate-second-head requires destination")
        mutate_second_head(args.path, Path(args.arg1))
    else:
        if None in (args.arg1, args.arg2, args.arg3, args.arg4):
            parser.error("run-filter requires input, output, status, policy")
        run_filter(args.path, Path(args.arg1), Path(args.arg2), int(args.arg3),
                   args.arg4)


if __name__ == "__main__":
    main()
