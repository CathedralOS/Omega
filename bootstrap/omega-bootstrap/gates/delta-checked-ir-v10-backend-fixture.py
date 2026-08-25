#!/usr/bin/env python3
"""Backend-local CKIR10 IntegerWiden carriers and instruction checks."""

from __future__ import annotations

import argparse
import importlib.util
import re
from pathlib import Path

import checked_ir_v10_reference as ir10


HERE = Path(__file__).resolve().parent


def load(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, HERE / filename)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


fixture = load("delta_checked_ir_v10_fixture", "delta-checked-ir-v10-fixture.py")


def emit(directory: Path) -> None:
    fixture.emit(directory)


def check_ir(path: Path) -> None:
    module = ir10.decode(path.read_bytes())
    ir10.v5.require(ir10.interpret(module) == 70, "canonical result")
    ir10.v5.require(any(op[3] == 21 for op in module.tables["operations"]),
                     "IntegerWiden count")


def check_artifact(path: Path, ir_path: Path) -> None:
    artifact = path.read_bytes()
    module = ir10.decode(ir_path.read_bytes())
    ir10.v5.require(artifact.startswith(b"\x7fELF\x02\x01\x01"), "ELF64 header")
    if any(op[3] == 21 for op in module.tables["operations"]):
        template = re.compile(
            rb"\x8b\x85....\x0f\xb6\xc0\x89\x85....", re.DOTALL,
        )
        ir10.v5.require(template.search(artifact) is not None,
                       "canonical unsigned IntegerWiden template")


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
