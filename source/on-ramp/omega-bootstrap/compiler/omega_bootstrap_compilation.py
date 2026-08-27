#!/usr/bin/env python3
"""Pack, verify, and inspect the private omega-bootstrap compilation envelope."""

from __future__ import annotations

import hashlib
import json
import struct
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import omega_bootstrap_bundle as source_bundle


MAGIC = b"OMGCOMP\0"
SCHEMA_MAJOR = 1
SCHEMA_MINOR = 0
TARGET_LINUX_X86_64 = 1
MAX_I32 = (1 << 31) - 1

MAX_SOURCES = 16
MAX_SOURCE_BYTES = 131_072
MAX_TOTAL_SOURCE_BYTES = 262_144
MAX_LABEL_BYTES = 64
MAX_TOTAL_LABEL_BYTES = 1_024
MAX_PACKAGES = 16
MAX_ALIASES = 32
MAX_STRINGS = 64
MAX_STRING_PAYLOAD_BYTES = 2_048
MAX_PATH_COMPONENTS = 8
MAX_IDENTIFIER_BYTES = 64
MAX_BUNDLE_BYTES = 263_312
MAX_ENVELOPE_BYTES = 267_280

HEADER = struct.Struct("<8sHHHH12I")
PACKAGE_ROW = struct.Struct("<I32sIII")
SOURCE_ROW = struct.Struct("<IIIII")
ALIAS_ROW = struct.Struct("<IIII")
U32 = struct.Struct("<I")


class CompilationError(ValueError):
    def __init__(self, message: str, status: int = 251):
        super().__init__(message)
        self.status = status


def reject(message: str) -> CompilationError:
    return CompilationError(message, 251)


def exhaust(message: str) -> CompilationError:
    return CompilationError(message, 252)


@dataclass(frozen=True)
class Package:
    package_id: int
    key: bytes
    source_start: int
    source_count: int


@dataclass(frozen=True)
class Source:
    source_id: int
    owner_package_id: int
    bundle_entry_id: int
    module_string_id: int


@dataclass(frozen=True)
class Alias:
    requester_package_id: int
    alias_string_id: int
    target_package_id: int


@dataclass(frozen=True)
class Compilation:
    encoded_length: int
    envelope_sha256: str
    bundle_length: int
    packages: tuple[Package, ...]
    sources: tuple[Source, ...]
    aliases: tuple[Alias, ...]
    strings: tuple[str, ...]
    root_package_id: int
    root_source_id: int
    root_owner_string_id: int
    root_machine_string_id: int
    bundle_entries: tuple[source_bundle.Entry, ...]


def _resource(value: int, maximum: int, name: str, minimum: int = 0) -> None:
    if value < minimum:
        raise reject(f"{name} is below its required minimum")
    if value > maximum:
        raise exhaust(f"{name} exceeds {maximum}")


def _i32(value: int, name: str) -> None:
    if value < 0 or value > MAX_I32:
        raise reject(f"{name} is outside the signed 32-bit bootstrap extent")


def _checked_extent(parts: list[tuple[int, int, str]]) -> int:
    total = HEADER.size
    for count, width, name in parts:
        _i32(count, name)
        product = count * width
        if product > MAX_I32 or total > MAX_I32 - product:
            raise reject("compilation envelope extent overflows signed 32-bit arithmetic")
        total += product
    return total


def _ascii(raw: bytes, name: str) -> str:
    try:
        return raw.decode("ascii")
    except UnicodeDecodeError as error:
        raise reject(f"{name} is not ASCII") from error


def _identifier(raw: bytes, name: str) -> None:
    if not raw:
        raise reject(f"{name} must be one identifier")
    if len(raw) > MAX_IDENTIFIER_BYTES:
        raise exhaust(f"{name} component exceeds {MAX_IDENTIFIER_BYTES} bytes")
    first = raw[0]
    if not (first == 95 or 65 <= first <= 90 or 97 <= first <= 122):
        raise reject(f"{name} is not a canonical identifier")
    for byte in raw[1:]:
        if not (byte == 95 or 48 <= byte <= 57 or 65 <= byte <= 90 or 97 <= byte <= 122):
            raise reject(f"{name} is not a canonical identifier")


def _package_alias(raw: bytes) -> None:
    if not raw:
        raise reject("local alias must be a package snake_case identifier")
    if len(raw) > MAX_IDENTIFIER_BYTES:
        raise exhaust(f"local alias component exceeds {MAX_IDENTIFIER_BYTES} bytes")
    if not 97 <= raw[0] <= 122:
        raise reject("local alias must begin with a lowercase ASCII letter")
    if raw[-1] == 95 or b"__" in raw:
        raise reject("local alias may not end in '_' or contain '__'")
    for byte in raw[1:]:
        if not (byte == 95 or 48 <= byte <= 57 or 97 <= byte <= 122):
            raise reject("local alias is not canonical package snake_case")


def _path(raw: bytes, name: str, allow_empty: bool) -> None:
    if not raw:
        if allow_empty:
            return
        raise reject(f"{name} may not be empty")
    components = raw.split(b"::")
    if len(components) > MAX_PATH_COMPONENTS:
        raise exhaust(f"{name} exceeds {MAX_PATH_COMPONENTS} path components")
    for component in components:
        _identifier(component, name)


def _parse_bundle(raw: bytes, expected_count: int) -> tuple[source_bundle.Entry, ...]:
    _resource(len(raw), MAX_BUNDLE_BYTES, "nested source-bundle byte length")
    try:
        entries = tuple(source_bundle.decode(raw))
    except source_bundle.BundleError as error:
        raise reject(f"nested source bundle: {error}") from error
    _resource(len(entries), MAX_SOURCES, "source count", 1)
    if len(entries) != expected_count:
        raise reject("nested source-bundle count does not equal envelope source count")
    total_labels = 0
    total_content = 0
    for entry in entries:
        label_length = len(entry.label.encode("ascii"))
        content_length = len(entry.content)
        _resource(label_length, MAX_LABEL_BYTES, "source label bytes", 1)
        _resource(content_length, MAX_SOURCE_BYTES, "source content bytes")
        total_labels += label_length
        total_content += content_length
    _resource(total_labels, MAX_TOTAL_LABEL_BYTES, "aggregate source label bytes")
    _resource(total_content, MAX_TOTAL_SOURCE_BYTES, "aggregate source content bytes")
    return entries


def decode(data: bytes) -> Compilation:
    if len(data) < HEADER.size:
        raise reject("truncated compilation-envelope header")
    fields = HEADER.unpack_from(data)
    (
        magic,
        major,
        minor,
        target,
        flags,
        encoded_length,
        bundle_length,
        string_table_length,
        string_count,
        package_count,
        source_count,
        alias_count,
        root_package_id,
        root_source_id,
        root_owner_string_id,
        root_machine_string_id,
        reserved,
    ) = fields

    if magic != MAGIC:
        raise reject("wrong compilation-envelope magic")
    if major != SCHEMA_MAJOR or minor != SCHEMA_MINOR:
        raise reject(f"unsupported compilation-envelope schema {major}.{minor}")
    if target != TARGET_LINUX_X86_64:
        raise reject(f"unsupported compilation target {target}")
    if flags != 0 or reserved != 0:
        raise reject("nonzero reserved header field")

    for value, name in (
        (encoded_length, "total envelope byte length"),
        (bundle_length, "nested source-bundle byte length"),
        (string_table_length, "canonical-string-table byte length"),
        (root_package_id, "selected root package ID"),
        (root_source_id, "selected root source ID"),
        (root_owner_string_id, "selected root owner string ID"),
        (root_machine_string_id, "selected root machine string ID"),
    ):
        _i32(value, name)
    for value, name in (
        (package_count, "package count"),
        (source_count, "source count"),
        (alias_count, "alias count"),
        (string_count, "canonical string count"),
    ):
        _i32(value, name)
    _resource(package_count, MAX_PACKAGES, "package count", 1)
    _resource(source_count, MAX_SOURCES, "source count", 1)
    _resource(alias_count, MAX_ALIASES, "alias count")
    _resource(string_count, MAX_STRINGS, "canonical string count")
    _resource(bundle_length, MAX_BUNDLE_BYTES, "nested source-bundle byte length")
    _resource(encoded_length, MAX_ENVELOPE_BYTES, "total envelope byte length")
    _resource(len(data), MAX_ENVELOPE_BYTES, "actual envelope byte length")

    expected_length = _checked_extent(
        [
            (package_count, PACKAGE_ROW.size, "package count"),
            (source_count, SOURCE_ROW.size, "source count"),
            (alias_count, ALIAS_ROW.size, "alias count"),
            (string_table_length, 1, "canonical-string-table byte length"),
            (bundle_length, 1, "nested source-bundle byte length"),
        ]
    )
    if encoded_length != expected_length or len(data) != expected_length:
        raise reject("computed, declared, and actual envelope lengths do not agree")

    cursor = HEADER.size
    packages: list[Package] = []
    expected_source_start = 0
    keys: set[bytes] = set()
    previous_key: bytes | None = None
    for package_id in range(package_count):
        row_id, key, source_start, count, row_reserved = PACKAGE_ROW.unpack_from(data, cursor)
        cursor += PACKAGE_ROW.size
        if row_id != package_id:
            raise reject("package IDs are not dense in row order")
        if not any(key):
            raise reject("package commitment is zero")
        if key in keys:
            raise reject("duplicate package commitment")
        if previous_key is not None and key <= previous_key:
            raise reject("package rows are not ordered by raw PackageKey bytes")
        keys.add(key)
        previous_key = key
        if row_reserved != 0:
            raise reject("nonzero reserved package field")
        _i32(source_start, "package source-row start")
        _i32(count, "package source-row count")
        if count == 0:
            raise reject("every retained package must own at least one source")
        if source_start != expected_source_start or count > source_count - source_start:
            raise reject("package source spans do not partition the source table")
        expected_source_start += count
        packages.append(Package(package_id, key, source_start, count))
    if expected_source_start != source_count:
        raise reject("package source spans do not cover the source table")

    sources: list[Source] = []
    bundle_ids: set[int] = set()
    for source_id in range(source_count):
        row_id, owner, bundle_id, module_id, row_flags = SOURCE_ROW.unpack_from(data, cursor)
        cursor += SOURCE_ROW.size
        if row_id != source_id:
            raise reject("source IDs are not dense in row order")
        if owner >= package_count:
            raise reject("source owner package ID is out of range")
        package = packages[owner]
        if not (package.source_start <= source_id < package.source_start + package.source_count):
            raise reject("source owner disagrees with its package span")
        if bundle_id >= source_count or bundle_id in bundle_ids:
            raise reject("bundle-entry IDs are not an exact permutation")
        bundle_ids.add(bundle_id)
        if module_id >= string_count:
            raise reject("source module-path string ID is out of range")
        if row_flags != 0:
            raise reject("nonzero source flags")
        sources.append(Source(source_id, owner, bundle_id, module_id))
    if bundle_ids != set(range(source_count)):
        raise reject("bundle-entry IDs are not an exact permutation")

    raw_aliases: list[tuple[int, int, int]] = []
    for _ in range(alias_count):
        requester, alias_id, target_id, row_reserved = ALIAS_ROW.unpack_from(data, cursor)
        cursor += ALIAS_ROW.size
        if requester >= package_count or target_id >= package_count:
            raise reject("alias package ID is out of range")
        if alias_id >= string_count:
            raise reject("alias string ID is out of range")
        if requester == target_id:
            raise reject("an alias may not target its requester")
        if row_reserved != 0:
            raise reject("nonzero reserved alias field")
        raw_aliases.append((requester, alias_id, target_id))

    string_end = cursor + string_table_length
    if string_end > len(data):
        raise reject("truncated canonical string table")
    strings: list[str] = []
    string_bytes: list[bytes] = []
    payload_bytes = 0
    previous: bytes | None = None
    while cursor < string_end:
        if string_end - cursor < U32.size:
            raise reject("truncated canonical string length")
        (length,) = U32.unpack_from(data, cursor)
        cursor += U32.size
        _i32(length, "canonical string byte length")
        if length > string_end - cursor:
            raise reject("truncated canonical string payload")
        raw = data[cursor : cursor + length]
        cursor += length
        if previous is not None and raw <= previous:
            raise reject("canonical strings are not unique and strictly increasing")
        previous = raw
        strings.append(_ascii(raw, "canonical string"))
        string_bytes.append(raw)
        payload_bytes += length
    if cursor != string_end or len(strings) != string_count:
        raise reject("canonical string count or table extent does not agree")
    _resource(payload_bytes, MAX_STRING_PAYLOAD_BYTES, "aggregate canonical string payload bytes")

    if root_package_id >= package_count or root_source_id >= source_count:
        raise reject("selected root ID is out of range")
    if root_owner_string_id >= string_count or root_machine_string_id >= string_count:
        raise reject("selected root string ID is out of range")
    if sources[root_source_id].owner_package_id != root_package_id:
        raise reject("selected root source does not belong to selected root package")

    referenced: set[int] = set()
    for source in sources:
        raw = string_bytes[source.module_string_id]
        _path(raw, "module path", allow_empty=True)
        referenced.add(source.module_string_id)
    aliases: list[Alias] = []
    previous_alias: tuple[int, bytes] | None = None
    for requester, alias_id, target_id in raw_aliases:
        raw = string_bytes[alias_id]
        _package_alias(raw)
        order_key = (requester, raw)
        if previous_alias is not None and order_key <= previous_alias:
            raise reject("alias rows are not ordered and requester-locally unique")
        previous_alias = order_key
        referenced.add(alias_id)
        aliases.append(Alias(requester, alias_id, target_id))
    _identifier(string_bytes[root_owner_string_id], "selected root owner")
    _identifier(string_bytes[root_machine_string_id], "selected root machine")
    referenced.add(root_owner_string_id)
    referenced.add(root_machine_string_id)
    if referenced != set(range(string_count)):
        raise reject("canonical string table contains an unused entry")

    adjacency: list[list[int]] = [[] for _ in packages]
    for alias in aliases:
        adjacency[alias.requester_package_id].append(alias.target_package_id)
    colors = [0] * package_count

    def visit(node: int) -> None:
        if colors[node] == 1:
            raise reject("alias package graph contains a cycle")
        if colors[node] == 2:
            return
        colors[node] = 1
        for target_id in adjacency[node]:
            visit(target_id)
        colors[node] = 2

    for package_id in range(package_count):
        visit(package_id)
    reachable: set[int] = set()
    pending = [root_package_id]
    while pending:
        package_id = pending.pop()
        if package_id in reachable:
            continue
        reachable.add(package_id)
        pending.extend(adjacency[package_id])
    if len(reachable) != package_count:
        raise reject("a package is unreachable from the selected root")

    bundle_raw = data[string_end:]
    if len(bundle_raw) != bundle_length:
        raise reject("nested source-bundle extent does not agree")
    bundle_entries = _parse_bundle(bundle_raw, source_count)
    for package in packages:
        previous_label: bytes | None = None
        for source_id in range(package.source_start, package.source_start + package.source_count):
            label = bundle_entries[sources[source_id].bundle_entry_id].label.encode("ascii")
            if previous_label is not None and label <= previous_label:
                raise reject("source rows are not ordered by nested-bundle label within package")
            previous_label = label

    return Compilation(
        encoded_length,
        hashlib.sha256(data).hexdigest(),
        bundle_length,
        tuple(packages),
        tuple(sources),
        tuple(aliases),
        tuple(strings),
        root_package_id,
        root_source_id,
        root_owner_string_id,
        root_machine_string_id,
        bundle_entries,
    )


def _object(value: Any, name: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise reject(f"{name} must be a JSON object")
    return value


def _list(value: Any, name: str) -> list[Any]:
    if not isinstance(value, list):
        raise reject(f"{name} must be a JSON array")
    return value


def _text(value: Any, name: str) -> str:
    if not isinstance(value, str):
        raise reject(f"{name} must be a JSON string")
    return value


def _keys(value: dict[str, Any], required: set[str], name: str) -> None:
    if set(value) != required:
        raise reject(f"{name} must contain exactly: {', '.join(sorted(required))}")


def _key(value: Any, name: str) -> bytes:
    text = _text(value, name)
    if len(text) != 64:
        raise reject(f"{name} must be exactly 64 hexadecimal digits")
    try:
        raw = bytes.fromhex(text)
    except ValueError as error:
        raise reject(f"{name} is not hexadecimal") from error
    if len(raw) != 32:
        raise reject(f"{name} must decode to exactly 32 bytes")
    if not any(raw):
        raise reject(f"{name} is zero")
    return raw


def encode_manifest(manifest: dict[str, Any], bundle_raw: bytes) -> bytes:
    _keys(manifest, {"target", "packages", "aliases", "root"}, "manifest")
    target = _text(manifest["target"], "target")
    if target != "linux_x86_64":
        raise reject(f"unsupported compilation target {target!r}")
    try:
        bundle_entries = tuple(source_bundle.decode(bundle_raw))
    except source_bundle.BundleError as error:
        raise reject(f"nested source bundle: {error}") from error
    _resource(len(bundle_entries), MAX_SOURCES, "source count", 1)
    bundle_by_label = {entry.label: index for index, entry in enumerate(bundle_entries)}

    package_specs: list[tuple[bytes, list[tuple[str, str]]]] = []
    for index, item in enumerate(_list(manifest["packages"], "packages")):
        package = _object(item, f"packages[{index}]")
        _keys(package, {"key", "sources"}, f"packages[{index}]")
        key = _key(package["key"], f"packages[{index}].key")
        source_specs: list[tuple[str, str]] = []
        for source_index, source_item in enumerate(_list(package["sources"], "package sources")):
            source_spec = _object(source_item, f"packages[{index}].sources[{source_index}]")
            _keys(source_spec, {"label", "module"}, "source specification")
            label = _text(source_spec["label"], "source label")
            module = _text(source_spec["module"], "source module")
            source_specs.append((label, module))
        package_specs.append((key, source_specs))
    _resource(len(package_specs), MAX_PACKAGES, "package count", 1)
    package_specs.sort(key=lambda item: item[0])
    keys = [item[0] for item in package_specs]
    if len(set(keys)) != len(keys):
        raise reject("duplicate package commitment")
    package_ids = {key: index for index, key in enumerate(keys)}

    string_set: set[bytes] = set()
    source_specs_by_package: list[list[tuple[int, bytes]]] = []
    assigned_labels: set[str] = set()
    for _, source_specs in package_specs:
        prepared: list[tuple[int, bytes]] = []
        for label, module in source_specs:
            if label not in bundle_by_label:
                raise reject(f"source label is absent from nested bundle: {label!r}")
            if label in assigned_labels:
                raise reject(f"nested bundle entry is assigned more than once: {label!r}")
            assigned_labels.add(label)
            try:
                module_raw = module.encode("ascii")
            except UnicodeEncodeError as error:
                raise reject("source module is not ASCII") from error
            _path(module_raw, "module path", allow_empty=True)
            string_set.add(module_raw)
            prepared.append((bundle_by_label[label], module_raw))
        prepared.sort(key=lambda item: item[0])
        source_specs_by_package.append(prepared)
    if assigned_labels != set(bundle_by_label):
        raise reject("every nested bundle entry must be assigned to one package")

    alias_specs: list[tuple[int, bytes, int]] = []
    for index, item in enumerate(_list(manifest["aliases"], "aliases")):
        alias = _object(item, f"aliases[{index}]")
        _keys(alias, {"requester", "alias", "target"}, f"aliases[{index}]")
        requester_key = _key(alias["requester"], "alias requester")
        target_key = _key(alias["target"], "alias target")
        if requester_key not in package_ids or target_key not in package_ids:
            raise reject("alias names a package absent from the manifest")
        alias_text = _text(alias["alias"], "local alias")
        try:
            alias_raw = alias_text.encode("ascii")
        except UnicodeEncodeError as error:
            raise reject("local alias is not ASCII") from error
        _package_alias(alias_raw)
        string_set.add(alias_raw)
        alias_specs.append((package_ids[requester_key], alias_raw, package_ids[target_key]))
    _resource(len(alias_specs), MAX_ALIASES, "alias count")
    alias_specs.sort(key=lambda item: (item[0], item[1]))

    root = _object(manifest["root"], "root")
    _keys(root, {"package", "source", "owner", "machine"}, "root")
    root_key = _key(root["package"], "root package")
    if root_key not in package_ids:
        raise reject("root package is absent from the manifest")
    root_label = _text(root["source"], "root source")
    try:
        owner_raw = _text(root["owner"], "root owner").encode("ascii")
        machine_raw = _text(root["machine"], "root machine").encode("ascii")
    except UnicodeEncodeError as error:
        raise reject("selected root name is not ASCII") from error
    _identifier(owner_raw, "selected root owner")
    _identifier(machine_raw, "selected root machine")
    string_set.update((owner_raw, machine_raw))

    strings_raw = sorted(string_set)
    _resource(len(strings_raw), MAX_STRINGS, "canonical string count")
    _resource(sum(map(len, strings_raw)), MAX_STRING_PAYLOAD_BYTES, "aggregate canonical string payload bytes")
    string_ids = {raw: index for index, raw in enumerate(strings_raw)}

    packages: list[Package] = []
    sources: list[Source] = []
    root_source_id: int | None = None
    for package_id, ((key, _), prepared) in enumerate(zip(package_specs, source_specs_by_package)):
        if not prepared:
            raise reject("every retained package must own at least one source")
        start = len(sources)
        for bundle_id, module_raw in prepared:
            source_id = len(sources)
            sources.append(Source(source_id, package_id, bundle_id, string_ids[module_raw]))
            if package_id == package_ids[root_key] and bundle_entries[bundle_id].label == root_label:
                root_source_id = source_id
        packages.append(Package(package_id, key, start, len(prepared)))
    if root_source_id is None:
        raise reject("root source is absent from the root package")

    string_table = bytearray()
    for raw in strings_raw:
        string_table.extend(U32.pack(len(raw)))
        string_table.extend(raw)
    aliases = [Alias(requester, string_ids[raw], target) for requester, raw, target in alias_specs]

    total = _checked_extent(
        [
            (len(packages), PACKAGE_ROW.size, "package count"),
            (len(sources), SOURCE_ROW.size, "source count"),
            (len(aliases), ALIAS_ROW.size, "alias count"),
            (len(string_table), 1, "canonical-string-table byte length"),
            (len(bundle_raw), 1, "nested source-bundle byte length"),
        ]
    )
    _resource(len(bundle_raw), MAX_BUNDLE_BYTES, "nested source-bundle byte length")
    _resource(total, MAX_ENVELOPE_BYTES, "total envelope byte length")
    output = bytearray(
        HEADER.pack(
            MAGIC,
            SCHEMA_MAJOR,
            SCHEMA_MINOR,
            TARGET_LINUX_X86_64,
            0,
            total,
            len(bundle_raw),
            len(string_table),
            len(strings_raw),
            len(packages),
            len(sources),
            len(aliases),
            package_ids[root_key],
            root_source_id,
            string_ids[owner_raw],
            string_ids[machine_raw],
            0,
        )
    )
    for package in packages:
        output.extend(PACKAGE_ROW.pack(package.package_id, package.key, package.source_start, package.source_count, 0))
    for source in sources:
        output.extend(
            SOURCE_ROW.pack(
                source.source_id,
                source.owner_package_id,
                source.bundle_entry_id,
                source.module_string_id,
                0,
            )
        )
    for alias in aliases:
        output.extend(ALIAS_ROW.pack(alias.requester_package_id, alias.alias_string_id, alias.target_package_id, 0))
    output.extend(string_table)
    output.extend(bundle_raw)
    encoded = bytes(output)
    decode(encoded)
    return encoded


def inspect(compilation: Compilation) -> dict[str, Any]:
    sources = []
    for source in compilation.sources:
        entry = compilation.bundle_entries[source.bundle_entry_id]
        sources.append(
            {
                "id": source.source_id,
                "package": source.owner_package_id,
                "bundle_entry": source.bundle_entry_id,
                "label": entry.label,
                "module": compilation.strings[source.module_string_id],
                "content_bytes": len(entry.content),
                "sha256": hashlib.sha256(entry.content).hexdigest(),
            }
        )
    return {
        "schema": "omega-bootstrap-compilation-envelope-v1",
        "target": "linux_x86_64",
        "encoded_bytes": compilation.encoded_length,
        "envelope_sha256": compilation.envelope_sha256,
        "bundle_bytes": compilation.bundle_length,
        "packages": [
            {
                "id": package.package_id,
                "key": package.key.hex(),
                "source_start": package.source_start,
                "source_count": package.source_count,
            }
            for package in compilation.packages
        ],
        "sources": sources,
        "aliases": [
            {
                "requester": alias.requester_package_id,
                "alias": compilation.strings[alias.alias_string_id],
                "target": alias.target_package_id,
            }
            for alias in compilation.aliases
        ],
        "root": {
            "package": compilation.root_package_id,
            "source": compilation.root_source_id,
            "owner": compilation.strings[compilation.root_owner_string_id],
            "machine": compilation.strings[compilation.root_machine_string_id],
        },
    }


def read_bytes(argument: str | None) -> bytes:
    return sys.stdin.buffer.read() if argument in (None, "-") else Path(argument).read_bytes()


def usage() -> CompilationError:
    return reject(
        "usage: omega_bootstrap_compilation.py pack MANIFEST.json BUNDLE | "
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
        sys.stdout.buffer.write(encode_manifest(_object(manifest, "manifest"), Path(rest[1]).read_bytes()))
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
        print(f"omega-bootstrap compilation: {error}", file=sys.stderr)
        raise SystemExit(error.status)
    except OSError as error:
        print(f"omega-bootstrap compilation: {error}", file=sys.stderr)
        raise SystemExit(2)
