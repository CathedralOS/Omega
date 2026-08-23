#!/usr/bin/env python3
"""Canonical pack/verify/get tool for the Omega0 source bundle format."""

from __future__ import annotations

import hashlib
import struct
import sys
from dataclasses import dataclass
from pathlib import Path


MAGIC = b"OMG0BNDL"
VERSION = 1
MAX_I32 = (1 << 31) - 1
HEADER = struct.Struct("<8sII")
ENTRY_HEADER = struct.Struct("<II")


class BundleError(ValueError):
    pass


@dataclass(frozen=True)
class Entry:
    label: str
    content: bytes


def validate_label(label: str) -> bytes:
    try:
        raw = label.encode("ascii")
    except UnicodeEncodeError as error:
        raise BundleError(f"label is not ASCII: {label!r}") from error
    if not raw or len(raw) > MAX_I32:
        raise BundleError("label must contain 1..2^31-1 bytes")
    if label.startswith("/") or label.endswith("/") or "\\" in label:
        raise BundleError(f"label is not a canonical relative POSIX path: {label!r}")
    parts = label.split("/")
    if any(part in ("", ".", "..") for part in parts):
        raise BundleError(f"label is not a canonical relative POSIX path: {label!r}")
    allowed = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._-/"
    if any(byte not in allowed for byte in raw):
        raise BundleError(f"label contains a non-portable byte: {label!r}")
    return raw


def encode(entries: list[Entry]) -> bytes:
    if not entries:
        raise BundleError("a bundle must contain at least one source")
    canonical: list[tuple[bytes, bytes]] = []
    labels: set[bytes] = set()
    for entry in entries:
        label = validate_label(entry.label)
        if label in labels:
            raise BundleError(f"duplicate label: {entry.label}")
        if len(entry.content) > MAX_I32:
            raise BundleError(f"source exceeds the signed 32-bit bootstrap extent: {entry.label}")
        labels.add(label)
        canonical.append((label, entry.content))
    canonical.sort(key=lambda item: item[0])
    if len(canonical) > MAX_I32:
        raise BundleError("too many sources")

    output = bytearray(HEADER.pack(MAGIC, VERSION, len(canonical)))
    for label, content in canonical:
        output.extend(ENTRY_HEADER.pack(len(label), len(content)))
        output.extend(label)
        output.extend(content)
    return bytes(output)


def decode(data: bytes) -> list[Entry]:
    if len(data) < HEADER.size:
        raise BundleError("truncated bundle header")
    magic, version, count = HEADER.unpack_from(data)
    if magic != MAGIC:
        raise BundleError("wrong bundle magic")
    if version != VERSION:
        raise BundleError(f"unsupported bundle version: {version}")
    if count == 0 or count > MAX_I32:
        raise BundleError("source count is outside the bootstrap profile")

    cursor = HEADER.size
    entries: list[Entry] = []
    previous: bytes | None = None
    for _ in range(count):
        if len(data) - cursor < ENTRY_HEADER.size:
            raise BundleError("truncated entry header")
        label_len, content_len = ENTRY_HEADER.unpack_from(data, cursor)
        cursor += ENTRY_HEADER.size
        if label_len == 0 or label_len > MAX_I32 or content_len > MAX_I32:
            raise BundleError("entry length is outside the bootstrap profile")
        end = cursor + label_len + content_len
        if end > len(data):
            raise BundleError("truncated entry payload")
        label_raw = data[cursor : cursor + label_len]
        cursor += label_len
        content = data[cursor : cursor + content_len]
        cursor += content_len
        try:
            label = label_raw.decode("ascii")
        except UnicodeDecodeError as error:
            raise BundleError("entry label is not ASCII") from error
        if validate_label(label) != label_raw:
            raise BundleError("entry label is not canonical")
        if previous is not None and label_raw <= previous:
            raise BundleError("entry labels are not strictly increasing")
        previous = label_raw
        entries.append(Entry(label, content))
    if cursor != len(data):
        raise BundleError("trailing bytes after the final source")
    return entries


def read_bundle(argument: str | None) -> bytes:
    return sys.stdin.buffer.read() if argument in (None, "-") else Path(argument).read_bytes()


def usage() -> BundleError:
    return BundleError(
        "usage: omega0_bundle.py pack LABEL=FILE... | verify [BUNDLE] | "
        "manifest [BUNDLE] | get BUNDLE LABEL"
    )


def main(arguments: list[str]) -> int:
    if not arguments:
        raise usage()
    command, *rest = arguments
    if command == "pack":
        entries: list[Entry] = []
        for specification in rest:
            if "=" not in specification:
                raise usage()
            label, filename = specification.split("=", 1)
            entries.append(Entry(label, Path(filename).read_bytes()))
        sys.stdout.buffer.write(encode(entries))
        return 0
    if command == "verify" and len(rest) <= 1:
        decode(read_bundle(rest[0] if rest else None))
        return 0
    if command == "manifest" and len(rest) <= 1:
        for entry in decode(read_bundle(rest[0] if rest else None)):
            digest = hashlib.sha256(entry.content).hexdigest()
            print(f"{entry.label}\t{len(entry.content)}\t{digest}")
        return 0
    if command == "get" and len(rest) == 2:
        bundle, wanted = rest
        validate_label(wanted)
        for entry in decode(read_bundle(bundle)):
            if entry.label == wanted:
                sys.stdout.buffer.write(entry.content)
                return 0
        raise BundleError(f"source not found: {wanted}")
    raise usage()


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except (BundleError, OSError) as error:
        print(f"omega0 bundle: {error}", file=sys.stderr)
        raise SystemExit(2)
