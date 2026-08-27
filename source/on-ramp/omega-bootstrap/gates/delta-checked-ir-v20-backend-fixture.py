#!/usr/bin/env python3
"""Independent CKIR20 backend artifact and mutation checks."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import re
import subprocess
from pathlib import Path

import checked_ir_v20_reference as ir20


HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location(
    "ckir20_fixture", HERE / "delta-checked-ir-v20-fixture.py")
assert spec is not None and spec.loader is not None
fixture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixture)

CANONICAL_CKIR_SHA = "ee418b04a4c661d329fe55198ae2b1063c86f5ed421711b9b0ab88f5eff6351a"
CANONICAL_ELF_SHA = "194bfd8efcc1bb8f48d1347de64f53331967247a6eba69c63a52163fa779935a"

CONST64 = re.compile(rb"\x48\xb8.{8}\x48\x89\x85.{4}", re.DOTALL)
INDEX64 = re.compile(
    rb"\x48\x8b\x85.{4}\x49\x89\xc2\x48\x8b\x85.{4}"
    rb"\x49\xb9.{8}\x4c\x39\xc8\x0f\x83.{4}"
    rb"\x48\x69\xc0.{4}\x0f\x80.{4}"
    rb"\x49\x01\xc2\x0f\x82.{4}"
    rb"\x4c\x89\xd0\x48\x89\x85.{4}", re.DOTALL)
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
SUM_COPY = re.compile(
    rb"\x41\x8b\x0b\x89\xca\x89\xc8\x3d.{4}\x0f\x83.{4}", re.DOTALL)
DISPATCH = re.compile(rb"\x41\x8b\x03\x3d\x09\x00\x00\x00\x0f\x83.{4}", re.DOTALL)
CASE_CONSTRUCTOR = re.compile(rb"\x41\xc7\x02\x02\x00\x00\x00", re.DOTALL)
PATTERNS = {
    "index": INDEX64, "add": ADD64, "less": LESS64,
    "range": RANGE64, "sum-copy": SUM_COPY, "dispatch": DISPATCH,
}


def require(condition: bool, message: str) -> None:
    ir20.v5.require(condition, message)


def run_filter(executable: Path, source: Path, destination: Path,
               expected: int, policy: str) -> None:
    with source.open("rb") as inp, destination.open("wb") as out:
        try:
            completed = subprocess.run(
                [str(executable)], stdin=inp, stdout=out,
                timeout=30, check=False)
        except subprocess.TimeoutExpired as error:
            raise ir20.Ckir20Error(f"timed out: {executable.name}") from error
    require(completed.returncode == expected,
            f"{executable.name} status {completed.returncode}, expected {expected}")
    require((destination.stat().st_size > 0) == (policy == "nonempty"),
            "backend publication policy")


def check_sum_control(artifact: bytes) -> None:
    """Follow every nonmatching sum-copy edge without trusting disassembly.

    Each JNE must land exactly on the next ordinal comparison; the last lands
    on the tag commit. This catches shared recursive-emitter state corruption,
    including targets in the middle of unrelated instructions. The canonical
    Float matching body must also copy all three active Boolean leaves before
    installing its sentinel and committing the original tag.
    """
    found_token_kind = False
    for header in SUM_COPY.finditer(artifact):
        case_count = int.from_bytes(header.group(0)[8:12], "little")
        cursor = header.end()
        float_body = b""
        for ordinal in range(case_count):
            require(artifact[cursor:cursor + 2] == b"\x81\xf9",
                    "sum-copy ordinal comparison boundary")
            require(int.from_bytes(artifact[cursor + 2:cursor + 6], "little") == ordinal,
                    "sum-copy complete ordinal progression")
            require(artifact[cursor + 6:cursor + 8] == b"\x0f\x85",
                    "sum-copy JNE template")
            displacement = int.from_bytes(
                artifact[cursor + 8:cursor + 12], "little", signed=True)
            target = cursor + 12 + displacement
            require(cursor + 12 <= target <= len(artifact),
                    "sum-copy forward branch extent")
            require(artifact[target - 5:target] == b"\xb9" + case_count.to_bytes(4, "little"),
                    "sum-copy matched-case sentinel")
            if case_count == 9 and ordinal == 2:
                float_body = artifact[cursor + 12:target - 5]
            cursor = target
        require(artifact[cursor:cursor + 3] == b"\x41\x89\x92",
                "sum-copy original-tag commit boundary")
        if case_count == 9:
            found_token_kind = True
            for offset in (4, 5, 6):
                leaf = (b"\x41\x0f\xb6\x83" + offset.to_bytes(4, "little")
                        + b"\x41\x88\x82" + offset.to_bytes(4, "little"))
                require(leaf in float_body,
                        "TokenKind::Float active Boolean payload copy")
    require(found_token_kind, "TokenKind semantic sum-copy control")


def check_artifact(path: Path, ir_path: Path) -> None:
    artifact = path.read_bytes()
    source = ir_path.read_bytes()
    module = ir20.decode(source)
    selected = ir20.profile(module)
    operations = module.tables["operations"]
    kinds = module.tables["types"]
    const64_count = sum(op[3] == 1 and kinds[op[7]][1] == 8
                        for op in operations)
    require(len(artifact) % 4096 == 0 and artifact[:4] == b"\x7fELF",
            "focused ELF envelope")
    require(int.from_bytes(artifact[96:104], "little") == len(artifact),
            "RX publication extent")
    require(int.from_bytes(artifact[160:168], "little") == 1_642_496,
            "exact rounded TokenStream BSS")
    require(len(CONST64.findall(artifact)) == const64_count,
            "exact qword Const templates")
    require(len(INDEX64.findall(artifact)) == len(selected["indexes"]),
            "exact guarded qword record-index templates")
    require(len(ADD64.findall(artifact)) == len(selected["adds"]),
            "exact qword Add templates")
    require(len(LESS64.findall(artifact)) == len(selected["lesses"]),
            "exact unsigned qword Less templates")
    require(len(RANGE64.findall(artifact)) >= 9,
            "qword parameter/store/call range custody")
    require(len(SUM_COPY.findall(artifact)) >= len(selected["copies"]),
            "semantic sum-copy tag guards")
    check_sum_control(artifact)
    require(len(DISPATCH.findall(artifact)) == 1,
            "indexed TokenKind dispatch tag guard")
    require(len(CASE_CONSTRUCTOR.findall(artifact)) >= 1,
            "Float constructor tag")
    if hashlib.sha256(source).hexdigest() == CANONICAL_CKIR_SHA:
        require(len(artifact) == 16_384, "canonical ELF length")
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
    parser.add_argument("command", choices=(
        "emit", "run-filter", "check-artifact", "mutate-template"))
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
