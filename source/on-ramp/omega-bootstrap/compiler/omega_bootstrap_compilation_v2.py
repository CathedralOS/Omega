#!/usr/bin/env python3
"""Pack, verify, and inspect the OMGCOMP2 target-bearing envelope.

OMGCOMP2 deliberately reuses the bounded version-1 tables.  Its only wire
extensions are schema major 2 and an exact selected-configuration word.  The
source bytes remain opaque to this structural layer.
"""

from __future__ import annotations

import dataclasses
import hashlib
import json
import struct
import sys
from pathlib import Path
from typing import Any

import omega_bootstrap_compilation as v1


MAGIC = v1.MAGIC
SCHEMA_MAJOR = 2
SCHEMA_MINOR = 0
TARGET_LINUX_X86_64 = v1.TARGET_LINUX_X86_64
CONFIG_NATIVE_PROVIDER_SUBSTITUTION = 1

HEADER = v1.HEADER
PACKAGE_ROW = v1.PACKAGE_ROW
SOURCE_ROW = v1.SOURCE_ROW
ALIAS_ROW = v1.ALIAS_ROW
U32 = v1.U32

MAX_ENVELOPE_BYTES = v1.MAX_ENVELOPE_BYTES
CompilationError = v1.CompilationError
Compilation = v1.Compilation


def reject(message: str) -> CompilationError:
    return CompilationError(message, 251)


def _object(value: Any, name: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise reject(f"{name} must be a JSON object")
    return value


def _keys(value: dict[str, Any], required: set[str], name: str) -> None:
    if set(value) != required:
        raise reject(f"{name} must contain exactly: {', '.join(sorted(required))}")


def _v1_bytes(data: bytes) -> bytes:
    patched = bytearray(data)
    struct.pack_into("<H", patched, 8, v1.SCHEMA_MAJOR)
    struct.pack_into("<I", patched, 60, 0)
    return bytes(patched)


def decode(data: bytes) -> Compilation:
    if len(data) < HEADER.size:
        raise reject("truncated compilation-envelope header")
    fields = HEADER.unpack_from(data)
    major, minor, configuration = fields[1], fields[2], fields[-1]
    if major != SCHEMA_MAJOR or minor != SCHEMA_MINOR:
        raise reject(f"unsupported compilation-envelope schema {major}.{minor}")
    if configuration != CONFIG_NATIVE_PROVIDER_SUBSTITUTION:
        raise reject("unsupported compilation configuration")

    decoded = v1.decode(_v1_bytes(data))
    return dataclasses.replace(decoded, envelope_sha256=hashlib.sha256(data).hexdigest())


def encode_manifest(manifest: dict[str, Any], bundle_raw: bytes) -> bytes:
    _keys(
        manifest,
        {"target", "configuration", "packages", "aliases", "root"},
        "manifest",
    )
    configuration = _object(manifest["configuration"], "configuration")
    _keys(configuration, {"native_provider_substitution"}, "configuration")
    if configuration["native_provider_substitution"] is not True:
        raise reject("configuration.native_provider_substitution must be true")

    v1_manifest = {key: value for key, value in manifest.items() if key != "configuration"}
    encoded = bytearray(v1.encode_manifest(v1_manifest, bundle_raw))
    struct.pack_into("<H", encoded, 8, SCHEMA_MAJOR)
    struct.pack_into("<I", encoded, 60, CONFIG_NATIVE_PROVIDER_SUBSTITUTION)
    result = bytes(encoded)
    decode(result)
    return result


def inspect(compilation: Compilation) -> dict[str, Any]:
    result = v1.inspect(compilation)
    result["schema"] = "omega-bootstrap-compilation-envelope-v2"
    result["configuration"] = {"native_provider_substitution": True}
    return result


def read_bytes(argument: str | None) -> bytes:
    return sys.stdin.buffer.read() if argument in (None, "-") else Path(argument).read_bytes()


def usage() -> CompilationError:
    return reject(
        "usage: omega_bootstrap_compilation_v2.py pack MANIFEST.json BUNDLE | "
        "verify [ENVELOPE] | inspect [ENVELOPE]"
    )


def main(arguments: list[str]) -> int:
    if not arguments:
        raise usage()
    command, *rest = arguments
    if command == "pack" and len(rest) == 2:
        try:
            manifest = json.loads(Path(rest[0]).read_text(encoding="utf-8"))
        except (json.JSONDecodeError, UnicodeDecodeError) as error:
            raise reject(f"invalid manifest JSON: {error}") from error
        output = encode_manifest(_object(manifest, "manifest"), Path(rest[1]).read_bytes())
        sys.stdout.buffer.write(output)
        return 0
    if command == "verify" and len(rest) <= 1:
        decode(read_bytes(rest[0] if rest else None))
        return 0
    if command == "inspect" and len(rest) <= 1:
        result = inspect(decode(read_bytes(rest[0] if rest else None)))
        json.dump(result, sys.stdout, indent=2, sort_keys=True)
        sys.stdout.write("\n")
        return 0
    raise usage()


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except CompilationError as error:
        print(f"omega-bootstrap compilation v2: {error}", file=sys.stderr)
        raise SystemExit(error.status)
    except OSError as error:
        print(f"omega-bootstrap compilation v2: {error}", file=sys.stderr)
        raise SystemExit(2)
