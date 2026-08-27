#!/usr/bin/env python3
"""Backend-local CKIR7 truth carriers and logical instruction checks."""

from __future__ import annotations

import argparse
import importlib.util
import re
from pathlib import Path

import checked_ir_v7_reference as ir7


HERE = Path(__file__).resolve().parent


def load(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, HERE / filename)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


fixture = load("delta_checked_ir_v7_fixture", "delta-checked-ir-v7-fixture.py")


def emit(directory: Path) -> None:
    fixture.emit(directory)


def check_ir(path: Path) -> None:
    module = ir7.decode(path.read_bytes())
    ir7.v5.require(ir7.interpret(module) == 70, "canonical result")
    ir7.v5.require(sum(op[3] in (16, 17) for op in module.tables["operations"]) == 1,
                   "logical binary count")


def check_artifact(path: Path, opcode: str) -> None:
    artifact = path.read_bytes()
    ir7.v5.require(artifact.startswith(b"\x7fELF\x02\x01\x01"), "ELF64 header")
    instruction = rb"\x23\x85" if opcode == "and" else rb"\x0b\x85"
    template = re.compile(
        rb"\x8b\x85...." + instruction + rb"....\x89\x85....",
        re.DOTALL,
    )
    ir7.v5.require(template.search(artifact) is not None,
                   f"canonical load/{opcode}/store template")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "command",
        choices=("emit", "check-ir", "check-artifact-and", "check-artifact-or"),
    )
    parser.add_argument("path", type=Path)
    args = parser.parse_args()
    if args.command == "emit":
        emit(args.path)
    elif args.command == "check-ir":
        check_ir(args.path)
    else:
        check_artifact(args.path, args.command.rsplit("-", 1)[1])


if __name__ == "__main__":
    main()
