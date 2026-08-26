#!/usr/bin/env python3
"""Backend-local CKIR16 artifact and timeout checks."""

from __future__ import annotations

import argparse
import re
import subprocess
from pathlib import Path

import checked_ir_v16_reference as ir16
import importlib.util

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location(
    "ckir16_fixture", HERE / "delta-checked-ir-v16-fixture.py")
assert spec is not None and spec.loader is not None
fixture = importlib.util.module_from_spec(spec); spec.loader.exec_module(fixture)

CONST64 = re.compile(rb"\x48\xb8.{8}\x48\x89\x85.{4}", re.DOTALL)
LOAD64 = re.compile(rb"\x48\x8b\x85.{4}\x48\x8b\x00\x48\x89\x85.{4}", re.DOTALL)
LESS64 = re.compile(rb"\x48\x8b\x85.{4}\x48\x3b\x85.{4}\x0f\x92\xc0\x0f\xb6\xc0\x89\x85.{4}", re.DOTALL)
RANGE64 = re.compile(rb"\x49\xb9.{8}\x4c\x39\xc8\x0f\x82.{4}\x49\xb9.{8}\x4c\x39\xc8\x0f\x87.{4}", re.DOTALL)
QWORD_FRAME = re.compile(rb"\x48\x8b\x85.{4}|\x48\x89\x85.{4}", re.DOTALL)


def run_filter(executable: Path, source: Path, destination: Path,
               expected: int, policy: str) -> None:
    with source.open("rb") as inp, destination.open("wb") as out:
        try:
            completed = subprocess.run([str(executable)], stdin=inp, stdout=out,
                                       timeout=20, check=False)
        except subprocess.TimeoutExpired as error:
            raise ir16.Ckir16Error(f"timed out: {executable.name}") from error
    ir16.v5.require(completed.returncode == expected,
                    f"{executable.name} status {completed.returncode}")
    ir16.v5.require((destination.stat().st_size > 0) == (policy == "nonempty"),
                    "backend publication policy")


def check_artifact(path: Path, ir_path: Path) -> None:
    artifact = path.read_bytes(); module = ir16.decode(ir_path.read_bytes())
    ir16.v5.require(len(CONST64.findall(artifact)) >= 2, "u64 Const templates")
    ir16.v5.require(len(LOAD64.findall(artifact)) >= 2, "u64 Load templates")
    ir16.v5.require(len(LESS64.findall(artifact)) == ir16.selected_count(module),
                    "exact qword unsigned Less template")
    ir16.v5.require(len(RANGE64.findall(artifact)) >= 6,
                    "store/call/edge/return/constructor range custody")
    ir16.v5.require(len(QWORD_FRAME.findall(artifact)) >= 12,
                    "qword frame/call/edge traffic")


def mutate_less(source: Path, destination: Path) -> None:
    raw = bytearray(source.read_bytes()); match = LESS64.search(raw)
    ir16.v5.require(match is not None, "source qword Less template")
    raw[match.start()] ^= 1; destination.write_bytes(raw)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("emit", "run-filter", "check-artifact", "mutate-less"))
    parser.add_argument("path", type=Path)
    parser.add_argument("arg1", nargs="?"); parser.add_argument("arg2", nargs="?")
    parser.add_argument("arg3", nargs="?"); parser.add_argument("arg4", nargs="?")
    args = parser.parse_args()
    if args.command == "emit": fixture.emit(args.path)
    elif args.command == "run-filter":
        run_filter(args.path, Path(args.arg1), Path(args.arg2), int(args.arg3), str(args.arg4))
    elif args.command == "check-artifact": check_artifact(args.path, Path(args.arg1))
    else: mutate_less(args.path, Path(args.arg1))


if __name__ == "__main__": main()
