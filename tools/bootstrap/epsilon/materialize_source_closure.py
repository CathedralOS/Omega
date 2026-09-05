#!/usr/bin/env python3

import hashlib
import os
import re
import sys
import tempfile
from pathlib import Path, PurePosixPath


HEADER = "EpsilonSourceClosureV1"
IDENTITY = re.compile(r"[0-9a-f]{64}")
DECIMAL = re.compile(r"0|[1-9][0-9]*")


def fail(message: str) -> None:
    raise SystemExit(f"epsilon source closure: {message}")


def source_path(root: Path, spelling: str) -> Path:
    relative = PurePosixPath(spelling)
    if relative.is_absolute() or not relative.parts or any(
        part in ("", ".", "..") for part in relative.parts
    ):
        fail(f"noncanonical member path: {spelling}")
    if "\\" in spelling:
        fail(f"noncanonical member path: {spelling}")
    candidate = root
    for part in relative.parts:
        candidate = candidate / part
        if candidate.is_symlink():
            fail(f"member path traverses a symbolic link: {spelling}")
    if not candidate.is_file():
        fail(f"member is not a regular file: {spelling}")
    return candidate


def source_bytes(path: Path, spelling: str) -> bytes:
    data = path.read_bytes()
    for offset, byte in enumerate(data):
        if byte not in (9, 10, 13) and not 32 <= byte <= 126:
            fail(f"forbidden byte in {spelling} at offset {offset}")
    return data


def materialize(manifest_path: Path) -> bytes:
    manifest_bytes = manifest_path.read_bytes()
    try:
        lines = manifest_bytes.decode("ascii").splitlines()
    except UnicodeDecodeError:
        fail("manifest is not ASCII")
    if not lines or lines[0] != HEADER:
        fail("manifest header is not EpsilonSourceClosureV1")
    if len(lines) == 1:
        fail("manifest has no members")

    result = bytearray()
    prior_identity = None
    paths = set()
    for line_number, line in enumerate(lines[1:], 2):
        fields = line.split(" ")
        if len(fields) != 5 or fields[0] != "member" or any(not field for field in fields):
            fail(f"malformed member row at line {line_number}")
        _, identity, length_text, expected_digest, spelling = fields
        if IDENTITY.fullmatch(identity) is None:
            fail(f"invalid member identity at line {line_number}")
        if prior_identity is not None and identity <= prior_identity:
            fail(f"member identities are not in strict order at line {line_number}")
        prior_identity = identity
        if DECIMAL.fullmatch(length_text) is None:
            fail(f"noncanonical member length at line {line_number}")
        if IDENTITY.fullmatch(expected_digest) is None:
            fail(f"invalid member digest at line {line_number}")
        if spelling in paths:
            fail(f"duplicate member path at line {line_number}")
        paths.add(spelling)

        path = source_path(manifest_path.parent, spelling)
        data = source_bytes(path, spelling)
        expected_length = int(length_text)
        if len(data) != expected_length:
            fail(
                f"member length changed for {spelling}: "
                f"expected {expected_length}, found {len(data)}"
            )
        digest = hashlib.sha256(data).hexdigest()
        if digest != expected_digest:
            fail(
                f"member digest changed for {spelling}: "
                f"expected {expected_digest}, found {digest}"
            )
        result.extend(data)
    return bytes(result)


def write_atomic(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(data)
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def main() -> None:
    if len(sys.argv) != 3:
        fail("usage: materialize_source_closure.py MANIFEST OUTPUT")
    manifest_path = Path(sys.argv[1])
    if manifest_path.is_symlink() or not manifest_path.is_file():
        fail("manifest is not a regular file")
    write_atomic(Path(sys.argv[2]), materialize(manifest_path))


if __name__ == "__main__":
    main()