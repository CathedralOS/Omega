#!/usr/bin/env python3
"""Backend-local CKIR11 canonical u32 Trapping Add instruction checks."""

from __future__ import annotations

import argparse
import importlib.util
import re
from pathlib import Path

import checked_ir_v11_reference as ir11


HERE = Path(__file__).resolve().parent


def load(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, HERE / filename)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


fixture = load("delta_checked_ir_v11_fixture_for_backend", "delta-checked-ir-v11-fixture.py")


def emit(directory: Path) -> None:
    fixture.emit(directory)


def check_ir(path: Path) -> None:
    module = ir11.decode(path.read_bytes())
    ir11.v5.require(ir11.interpret(module) == 70, "canonical result")
    ir11.v5.require(ir11.selected_add_count(module) == 1,
                     "canonical u32 Trapping Add count")


def check_artifact(path: Path, ir_path: Path) -> None:
    artifact = path.read_bytes()
    module = ir11.decode(ir_path.read_bytes())
    ir11.v5.require(artifact.startswith(b"\x7fELF\x02\x01\x01"), "ELF64 header")
    if ir11.selected_add_count(module):
        template = re.compile(
            rb"\x8b\x85....\x03\x85....\x0f\x82...."
            rb"\x3d\x00\x00\x00\x00\x0f\x82...."
            rb"\x3d\xff\xff\xff\x7f\x0f\x87....\x89\x85....",
            re.DOTALL,
        )
        ir11.v5.require(template.search(artifact) is not None,
                        "canonical Add/carry/range/store template")


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
