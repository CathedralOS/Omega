#!/usr/bin/env python3
"""Focused CKIR18 backend artifact, parity, and mutation checks."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import re
import subprocess
from pathlib import Path

import checked_ir_v18_reference as ir18


HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location(
    "ckir18_fixture", HERE / "delta-checked-ir-v18-fixture.py")
assert spec is not None and spec.loader is not None
fixture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixture)

CANONICAL_CKIR_SHA = "fd468683d3429eebccd700723f5f554ae586b245b7b6a9570caa5b57ed84a9bb"
CANONICAL_ELF_SHA = "83d5c09e1da6543a59514d0b1cff13e087032e3caafba39452268993a92ad0ce"

CONST64 = re.compile(rb"\x48\xb8.{8}\x48\x89\x85.{4}", re.DOTALL)
LOAD64 = re.compile(rb"\x48\x8b\x85.{4}\x48\x8b\x00\x48\x89\x85.{4}", re.DOTALL)
INDEX64 = re.compile(
    rb"\x48\x8b\x85.{4}\x49\x89\xc2\x48\x8b\x85.{4}"
    rb"\x49\xb9.{8}\x4c\x39\xc8\x0f\x83.{4}"
    rb"\x49\x01\xc2\x4c\x89\xd0\x48\x89\x85.{4}", re.DOTALL)
ADD64 = re.compile(
    rb"\x48\x8b\x85.{4}\x48\x03\x85.{4}\x0f\x82.{4}"
    rb"\x49\xb9.{8}\x4c\x39\xc8\x0f\x82.{4}"
    rb"\x49\xb9.{8}\x4c\x39\xc8\x0f\x87.{4}"
    rb"\x48\x89\x85.{4}", re.DOTALL)
LESS64 = re.compile(
    rb"\x48\x8b\x85.{4}\x48\x3b\x85.{4}"
    rb"\x0f\x92\xc0\x0f\xb6\xc0\x89\x85.{4}", re.DOTALL)
RANGE64 = re.compile(
    rb"\x49\xb9.{8}\x4c\x39\xc8\x0f\x82.{4}"
    rb"\x49\xb9.{8}\x4c\x39\xc8\x0f\x87.{4}", re.DOTALL)
QWORD_FRAME = re.compile(rb"\x48\x8b\x85.{4}|\x48\x89\x85.{4}", re.DOTALL)
PATTERNS = {"index": INDEX64, "add": ADD64, "less": LESS64, "range": RANGE64}


def require(condition: bool, message: str) -> None:
    ir18.v5.require(condition, message)


def run_filter(executable: Path, source: Path, destination: Path,
               expected: int, policy: str) -> None:
    with source.open("rb") as inp, destination.open("wb") as out:
        try:
            completed = subprocess.run(
                [str(executable)], stdin=inp, stdout=out,
                timeout=30, check=False,
            )
        except subprocess.TimeoutExpired as error:
            raise ir18.Ckir18Error(f"timed out: {executable.name}") from error
    require(completed.returncode == expected,
            f"{executable.name} status {completed.returncode}")
    require((destination.stat().st_size > 0) == (policy == "nonempty"),
            "backend publication policy")


def check_artifact(path: Path, ir_path: Path) -> None:
    artifact = path.read_bytes()
    source = ir_path.read_bytes()
    module = ir18.decode(source)
    selected = ir18.selected_operations(module)
    operations = module.tables["operations"]
    kinds = module.tables["types"]
    const64_count = sum(
        row[3] == 1 and kinds[row[7]][1] == 8 for row in operations
    )
    load64_count = sum(
        row[3] == 5 and kinds[row[7]][1] == 8 for row in operations
    )
    require(len(artifact) == 8192 and artifact[:4] == b"\x7fELF",
            "focused ELF envelope")
    require(len(CONST64.findall(artifact)) == const64_count,
            "exact qword Const templates")
    require(len(LOAD64.findall(artifact)) == load64_count,
            "exact qword Load templates")
    require(len(INDEX64.findall(artifact)) == len(selected["index"]),
            "exact qword IndexPlace templates")
    require(len(ADD64.findall(artifact)) == len(selected["add"]),
            "exact qword Add templates")
    require(len(LESS64.findall(artifact)) == len(selected["less"]),
            "exact qword unsigned Less templates")
    require(len(RANGE64.findall(artifact)) >= 5,
            "qword destination range custody")
    require(len(QWORD_FRAME.findall(artifact)) >= 20,
            "qword frame/call/edge traffic")
    if hashlib.sha256(source).hexdigest() == CANONICAL_CKIR_SHA:
        require(hashlib.sha256(artifact).hexdigest() == CANONICAL_ELF_SHA,
                "canonical focused ELF identity")


def mutate_template(source: Path, destination: Path, family: str) -> None:
    raw = bytearray(source.read_bytes())
    match = PATTERNS[family].search(raw)
    require(match is not None, f"source {family} template")
    raw[match.start()] ^= 1
    destination.write_bytes(raw)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "command", choices=("emit", "run-filter", "check-artifact", "mutate-template"))
    parser.add_argument("path", type=Path)
    parser.add_argument("arg1", nargs="?")
    parser.add_argument("arg2", nargs="?")
    parser.add_argument("arg3", nargs="?")
    parser.add_argument("arg4", nargs="?")
    args = parser.parse_args()
    if args.command == "emit":
        fixture.emit(args.path)
    elif args.command == "run-filter":
        run_filter(args.path, Path(args.arg1), Path(args.arg2),
                   int(args.arg3), str(args.arg4))
    elif args.command == "check-artifact":
        check_artifact(args.path, Path(args.arg1))
    else:
        mutate_template(args.path, Path(args.arg1), str(args.arg2))


if __name__ == "__main__":
    main()
