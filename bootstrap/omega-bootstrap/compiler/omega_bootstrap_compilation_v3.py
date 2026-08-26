#!/usr/bin/env python3
"""Pack, verify, and inspect the OMGCOMP3 build-source-bearing envelope."""

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
SCHEMA_MAJOR = 3
SCHEMA_MINOR = 0
TARGET_LINUX_X86_64 = v1.TARGET_LINUX_X86_64
CONFIG_NATIVE_PROVIDER_SUBSTITUTION = 1
SOURCE_FLAG_BUILD = 1

HEADER = v1.HEADER
PACKAGE_ROW = v1.PACKAGE_ROW
SOURCE_ROW = v1.SOURCE_ROW

MAX_ENVELOPE_BYTES = v1.MAX_ENVELOPE_BYTES
CompilationError = v1.CompilationError


@dataclasses.dataclass(frozen=True)
class Compilation(v1.Compilation):
    build_source_id: int


def reject(message: str) -> CompilationError:
    return CompilationError(message, 251)


def _object(value: Any, name: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise reject(f"{name} must be a JSON object")
    return value


def _keys(value: dict[str, Any], required: set[str], name: str) -> None:
    if set(value) != required:
        raise reject(f"{name} must contain exactly: {', '.join(sorted(required))}")


def _v1_bytes(data: bytes) -> tuple[bytes, int]:
    if len(data) < HEADER.size:
        raise reject("truncated compilation-envelope header")
    fields = HEADER.unpack_from(data)
    if fields[0] != MAGIC:
        raise reject("wrong compilation-envelope magic")
    if fields[1:5] != (SCHEMA_MAJOR, SCHEMA_MINOR, TARGET_LINUX_X86_64, 0):
        raise reject("unsupported OMGCOMP3 identity or target")
    if fields[16] != CONFIG_NATIVE_PROVIDER_SUBSTITUTION:
        raise reject("unsupported compilation configuration")

    package_count = fields[9]
    source_count = fields[10]
    if package_count > v1.MAX_PACKAGES:
        raise v1.exhaust(f"package count exceeds {v1.MAX_PACKAGES}")
    if source_count > v1.MAX_SOURCES:
        raise v1.exhaust(f"source count exceeds {v1.MAX_SOURCES}")
    source_table = HEADER.size + package_count * PACKAGE_ROW.size
    if source_table + source_count * SOURCE_ROW.size > len(data):
        raise reject("truncated compilation-envelope source table")

    patched = bytearray(data)
    build_sources: list[int] = []
    for source_id in range(source_count):
        flags_offset = source_table + source_id * SOURCE_ROW.size + 16
        flags = struct.unpack_from("<I", data, flags_offset)[0]
        if flags not in (0, SOURCE_FLAG_BUILD):
            raise reject("unsupported OMGCOMP3 source-role flags")
        if flags == SOURCE_FLAG_BUILD:
            build_sources.append(source_id)
            struct.pack_into("<I", patched, flags_offset, 0)
    if len(build_sources) != 1:
        raise reject("OMGCOMP3 requires exactly one authoritative build source")

    struct.pack_into("<H", patched, 8, v1.SCHEMA_MAJOR)
    struct.pack_into("<I", patched, 60, 0)
    return bytes(patched), build_sources[0]


def decode(data: bytes) -> Compilation:
    patched, build_source_id = _v1_bytes(data)
    decoded = v1.decode(patched)
    if decoded.sources[build_source_id].owner_package_id != decoded.root_package_id:
        raise reject("authoritative build source is not owned by the root package")
    values = {field.name: getattr(decoded, field.name) for field in dataclasses.fields(v1.Compilation)}
    values["envelope_sha256"] = hashlib.sha256(data).hexdigest()
    return Compilation(**values, build_source_id=build_source_id)


def encode_manifest(manifest: dict[str, Any], bundle_raw: bytes) -> bytes:
    _keys(
        manifest,
        {"target", "configuration", "packages", "aliases", "root", "build"},
        "manifest",
    )
    configuration = _object(manifest["configuration"], "configuration")
    _keys(configuration, {"native_provider_substitution"}, "configuration")
    if configuration["native_provider_substitution"] is not True:
        raise reject("configuration.native_provider_substitution must be true")

    build = _object(manifest["build"], "build")
    _keys(build, {"package", "source"}, "build")
    build_key = v1._key(build["package"], "build package")
    build_label = v1._text(build["source"], "build source")

    v1_manifest = {
        key: value
        for key, value in manifest.items()
        if key not in {"configuration", "build"}
    }
    encoded = bytearray(v1.encode_manifest(v1_manifest, bundle_raw))
    base = v1.decode(bytes(encoded))
    package_ids = {package.key: package.package_id for package in base.packages}
    if build_key not in package_ids:
        raise reject("build package is absent from the manifest")
    build_package_id = package_ids[build_key]
    matches = [
        source.source_id
        for source in base.sources
        if source.owner_package_id == build_package_id
        and base.bundle_entries[source.bundle_entry_id].label == build_label
    ]
    if len(matches) != 1:
        raise reject("build source is absent from its named package")

    source_table = HEADER.size + len(base.packages) * PACKAGE_ROW.size
    struct.pack_into("<I", encoded, source_table + matches[0] * SOURCE_ROW.size + 16, SOURCE_FLAG_BUILD)
    struct.pack_into("<H", encoded, 8, SCHEMA_MAJOR)
    struct.pack_into("<I", encoded, 60, CONFIG_NATIVE_PROVIDER_SUBSTITUTION)
    result = bytes(encoded)
    decode(result)
    return result


def inspect(compilation: Compilation) -> dict[str, Any]:
    result = v1.inspect(compilation)
    source = compilation.sources[compilation.build_source_id]
    entry = compilation.bundle_entries[source.bundle_entry_id]
    result["schema"] = "omega-bootstrap-compilation-envelope-v3"
    result["configuration"] = {"native_provider_substitution": True}
    result["build"] = {
        "package": source.owner_package_id,
        "source": source.source_id,
        "label": entry.label,
        "module": compilation.strings[source.module_string_id],
    }
    return result


def read_bytes(argument: str | None) -> bytes:
    return sys.stdin.buffer.read() if argument in (None, "-") else Path(argument).read_bytes()


def usage() -> CompilationError:
    return reject(
        "usage: omega_bootstrap_compilation_v3.py pack MANIFEST.json BUNDLE | "
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
        sys.stdout.buffer.write(
            encode_manifest(_object(manifest, "manifest"), Path(rest[1]).read_bytes())
        )
        return 0
    if command == "verify" and len(rest) <= 1:
        decode(read_bytes(rest[0] if rest else None))
        return 0
    if command == "inspect" and len(rest) <= 1:
        json.dump(inspect(decode(read_bytes(rest[0] if rest else None))), sys.stdout, indent=2, sort_keys=True)
        sys.stdout.write("\n")
        return 0
    raise usage()


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except CompilationError as error:
        print(f"omega-bootstrap compilation v3: {error}", file=sys.stderr)
        raise SystemExit(error.status)
    except OSError as error:
        print(f"omega-bootstrap compilation v3: {error}", file=sys.stderr)
        raise SystemExit(2)
