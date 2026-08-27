#!/usr/bin/env python3
"""Backend-local CKIR12 shared-byte-view artifact checks."""

from __future__ import annotations

import argparse
import importlib.util
import re
import struct
from pathlib import Path

import checked_ir_v12_reference as ir12


HERE = Path(__file__).resolve().parent


def load(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, HERE / filename)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


fixture = load("delta_checked_ir_v12_fixture_for_backend",
               "delta-checked-ir-v12-fixture.py")


def emit(directory: Path) -> None:
    fixture.emit(directory)


def literal_bytes(module: ir12.Module) -> bytes:
    operations = module.tables["operations"]
    roots = [operation[10] for operation in operations if operation[3] == 22]
    ir12.v5.require(len(roots) == 1, "focused StaticByteView root")
    root = module.tables["constants"][roots[0]]
    children = module.tables["constant_children"]
    nodes = module.tables["constants"]
    return bytes(nodes[children[index][0]][4]
                 for index in range(root[2], root[2] + root[3]))


def readonly_segment(artifact: bytes) -> bytes:
    ir12.v5.require(artifact.startswith(b"\x7fELF\x02\x01\x01"), "ELF64 header")
    phoff = struct.unpack_from("<Q", artifact, 32)[0]
    phentsize, phnum = struct.unpack_from("<HH", artifact, 54)
    ir12.v5.require(phentsize == 56, "ELF64 program-header size")
    for index in range(phnum):
        row = struct.unpack_from("<IIQQQQQQ", artifact, phoff + index * phentsize)
        kind, flags, offset, _, _, file_size, _, alignment = row
        if kind == 1 and flags == 4:
            ir12.v5.require(alignment == 4096 and file_size >= 1,
                           "private read-only literal segment")
            return artifact[offset:offset + file_size]
    raise ir12.Ckir12Error("missing private read-only literal segment")


STATIC_PREFIX = re.compile(
    rb"\x4c\x8d\x9d....\x48\x8d\x05....\x49\x89\x03\xb8(....)"
    rb"\x49\x89\x43\x08\x4c\x89\x9d....",
    re.DOTALL,
)
NONEMPTY = re.compile(
    rb"\x4c\x8b\x9d....\x49\x83\x7b\x08\x00\x0f\x95\xc0"
    rb"\x0f\xb6\xc0\x89\x85....",
    re.DOTALL,
)
HEAD = re.compile(
    rb"\x4c\x8b\x9d....\x49\x83\x7b\x08\x00\x0f\x84...."
    rb"\x49\x8b\x03\x0f\xb6\x00\x89\x85....",
    re.DOTALL,
)
TAIL = re.compile(
    rb"\x4c\x8b\x9d....\x4c\x8d\x95....\x49\x83\x7b\x08\x00"
    rb"\x0f\x84....\x49\x8b\x03\x48\x83\xc0\x01\x49\x89\x02"
    rb"\x49\x8b\x43\x08\x48\x83\xe8\x01\x49\x89\x42\x08"
    rb"\x4c\x89\x95....",
    re.DOTALL,
)
TRUE_EDGE_BRANCH = re.compile(rb"\x8b\x85....\x85\xc0\x0f\x84....", re.DOTALL)


def check_ir(path: Path) -> None:
    module = ir12.decode(path.read_bytes())
    ir12.v5.require(ir12.interpret(module) == 70, "shared-byte-view result")
    ir12.v5.require(ir12.selected_counts(module) == {22: 1, 23: 2, 24: 1, 25: 1},
                    "focused operation counts")


def check_artifact(path: Path, ir_path: Path) -> None:
    artifact = path.read_bytes()
    module = ir12.decode(ir_path.read_bytes())
    expected = literal_bytes(module)
    static = STATIC_PREFIX.search(artifact)
    ir12.v5.require(static is not None, "StaticByteView descriptor template")
    assert static is not None
    ir12.v5.require(struct.unpack("<I", static.group(1))[0] == len(expected),
                    "StaticByteView exact length")
    ir12.v5.require(NONEMPTY.search(artifact) is not None,
                    "SliceNonEmpty descriptor-length template")
    ir12.v5.require(HEAD.search(artifact) is not None,
                    "SliceHead nonempty-check/load template")
    ir12.v5.require(TAIL.search(artifact) is not None,
                    "SliceTailOne nonempty-check/descriptor template")
    ir12.v5.require(TRUE_EDGE_BRANCH.search(artifact) is not None,
                    "false-bypassing true-edge branch template")
    ro = readonly_segment(artifact)
    materialized = expected if expected else b"\0"
    ir12.v5.require(ro.startswith(materialized), "program-static literal bytes")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("emit", "check-ir", "check-artifact"))
    parser.add_argument("path", type=Path)
    parser.add_argument("ir", type=Path, nargs="?")
    args = parser.parse_args()
    if args.command == "emit":
        emit(args.path)
    elif args.command == "check-ir":
        check_ir(args.path)
    else:
        if args.ir is None:
            parser.error("check-artifact requires CKIR path")
        check_artifact(args.path, args.ir)


if __name__ == "__main__":
    main()
