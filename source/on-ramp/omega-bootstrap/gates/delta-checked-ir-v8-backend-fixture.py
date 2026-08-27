#!/usr/bin/env python3
"""Backend-local CKIR8 ScalarEqual carriers and instruction checks."""

from __future__ import annotations

import argparse
import importlib.util
import re
from pathlib import Path

import checked_ir_v8_reference as ir8


HERE = Path(__file__).resolve().parent


def load(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, HERE / filename)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


fixture = load("delta_checked_ir_v8_fixture", "delta-checked-ir-v8-fixture.py")


def emit(directory: Path) -> None:
    fixture.emit(directory)


def check_ir(path: Path) -> None:
    module = ir8.decode(path.read_bytes())
    ir8.v5.require(ir8.interpret(module) == 70, "canonical result")
    ir8.v5.require(sum(op[3] == 18 for op in module.tables["operations"]) == 1,
                   "ScalarEqual count")


def check_artifact(path: Path) -> None:
    artifact = path.read_bytes()
    ir8.v5.require(artifact.startswith(b"\x7fELF\x02\x01\x01"), "ELF64 header")
    template = re.compile(
        rb"\x8b\x85....\x3b\x85....\x0f\x94\xc0\x0f\xb6\xc0\x89\x85....",
        re.DOTALL,
    )
    ir8.v5.require(template.search(artifact) is not None,
                   "canonical load/cmp/sete/movzx/store template")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("emit", "check-ir", "check-artifact"))
    parser.add_argument("path", type=Path)
    args = parser.parse_args()
    if args.command == "emit":
        emit(args.path)
    elif args.command == "check-ir":
        check_ir(args.path)
    else:
        check_artifact(args.path)


if __name__ == "__main__":
    main()
