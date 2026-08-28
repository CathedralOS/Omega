#!/usr/bin/env python3
"""Bind one verified Delta assembly publication to one Darwin ARM64 artifact.

This verifier is deliberately only an executable-custody join.  It replays the
complete assembly-publication join, validates a narrow unsigned Mach-O image,
and records exact realization-tool inputs.  It does not prove that the artifact
refines Delta source semantics and therefore grants no compiler authority.
"""

from __future__ import annotations

import hashlib
import json
import os
import struct
import sys
from pathlib import Path

import lower_rooted_assembly_publication_v1 as assembly_publication
import publication_support as support


OBSERVATION_SCHEMA = "omega.delta-darwin-arm64-realization-observation.v1"
RECEIPT_SCHEMA = "omega.delta-lower-rooted-artifact-custody.v1"
RECEIPT_DOMAIN = b"omega.delta-lower-rooted-artifact-custody.v1\0"
PUBLICATION_ID = "delta.compiler.darwin-arm64-executable.candidate.v1"
CLAIM = "candidate_lower_rooted_executable_identity_only"
FORMAT_PROFILE = "delta.darwin-arm64-macho.unsigned-no-uuid-v1"
COMMAND_PROFILE = {
    "architecture_arguments": ["-arch", "arm64"],
    "driver": "apple-clang",
    "input_kind": "darwin-arm64-assembly-v1",
    "linker_selection_argument": "-fuse-ld=LINKER",
    "linker_arguments": ["-Wl,-no_uuid", "-Wl,-no_adhoc_codesign"],
    "minimum_macos_argument": "-mmacosx-version-min=11.0",
    "output_kind": "darwin-arm64-macho-executable-v1",
    "sdk_arguments": ["-isysroot", "SDK_ROOT"],
}
OPEN_REFINEMENT = {
    "reason": "authoritative_delta_v1_semantics_subject_not_published",
    "status": "open",
}

MAX_DOCUMENT = 65_536
MAX_ARTIFACT_BYTES = 64 * 1024 * 1024
MAX_TOOLCHAIN_COMPONENT_BYTES = 512 * 1024 * 1024
MAX_SDK_COMPONENT_BYTES = 64 * 1024 * 1024
EMPTY_SHA256 = hashlib.sha256(b"").hexdigest()

MH_MAGIC_64 = 0xFEEDFACF
CPU_TYPE_ARM64 = 0x0100000C
CPU_SUBTYPE_ARM64_ALL = 0
MH_EXECUTE = 2
MH_EXACT_FLAGS = 0x1 | 0x4 | 0x80 | 0x200000  # NOUNDEFS, DYLDLINK, TWOLEVEL, PIE
MH_ALLOW_STACK_EXECUTION = 0x20000

LC_SEGMENT_64 = 0x19
LC_LOAD_DYLIB = 0xC
LC_LOAD_DYLINKER = 0xE
LC_SYMTAB = 0x2
LC_DYSYMTAB = 0xB
LC_UNIXTHREAD = 0x5
LC_UUID = 0x1B
LC_CODE_SIGNATURE = 0x1D
LC_LAZY_LOAD_DYLIB = 0x20
LC_ENCRYPTION_INFO = 0x21
LC_VERSION_MIN_MACOSX = 0x24
LC_ENCRYPTION_INFO_64 = 0x2C
LC_BUILD_VERSION = 0x32
LC_MAIN = 0x80000028
LC_DYLD_INFO_ONLY = 0x80000022
LC_FUNCTION_STARTS = 0x26
LC_DATA_IN_CODE = 0x29
LC_SOURCE_VERSION = 0x2A
LC_LOAD_WEAK_DYLIB = 0x80000018
LC_REEXPORT_DYLIB = 0x8000001F
LC_LOAD_UPWARD_DYLIB = 0x80000023
LC_RPATH = 0x8000001C
PLATFORM_MACOS = 1
MIN_MACOS_11 = 11 << 16

# The written Delta assembly contributes __text, __const, and __bss. The V1
# command profile's Darwin linker synthesizes only the remaining stub/pointer
# sections below. A new section is a target-profile change to review, not
# opaque payload that can inherit this receipt's format claim.
SECTION_FLAGS = {
    ("__TEXT", "__text"): 0x80000400,
    ("__TEXT", "__stubs"): 0x80000408,
    ("__TEXT", "__stub_helper"): 0x80000400,
    ("__TEXT", "__const"): 0,
    ("__DATA_CONST", "__got"): 0x6,
    ("__DATA", "__la_symbol_ptr"): 0x7,
    ("__DATA", "__data"): 0,
    ("__DATA", "__bss"): 0x1,
}


class CustodyError(Exception):
    status = 251


class CustodyResourceError(CustodyError):
    status = 252


def fail(message: str) -> None:
    raise CustodyError(message)


def resource(message: str) -> None:
    raise CustodyResourceError(message)


def canonical_json(value: object, *, pretty: bool) -> bytes:
    options = {"ensure_ascii": False, "sort_keys": True}
    if pretty:
        return (json.dumps(value, indent=2, **options) + "\n").encode()
    return json.dumps(value, separators=(",", ":"), **options).encode()


def bounded_read(path: Path, context: str, limit: int) -> bytes:
    """Capture one limited path without separating extent, bytes, or identity."""

    with path.open("rb") as stream:
        before = os.fstat(stream.fileno())
        if before.st_size > limit:
            resource(f"{context} byte ceiling")
        raw = stream.read(min(before.st_size + 1, limit + 1))
        after = os.fstat(stream.fileno())
    if len(raw) > limit or after.st_size > limit:
        resource(f"{context} byte ceiling")
    if (
        before.st_dev != after.st_dev
        or before.st_ino != after.st_ino
        or before.st_size != after.st_size
        or len(raw) != after.st_size
    ):
        fail(f"{context} changed while reading")
    current = path.stat()
    if current.st_size > limit:
        resource(f"{context} byte ceiling")
    if (
        current.st_dev != after.st_dev
        or current.st_ino != after.st_ino
        or current.st_size != after.st_size
    ):
        fail(f"{context} path changed while reading")
    return raw


def load_json(path: Path, context: str) -> dict:
    raw = bounded_read(path, context, MAX_DOCUMENT)
    try:
        value = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail(f"{context} JSON: {error}")
    if not isinstance(value, dict) or raw != canonical_json(value, pretty=True):
        fail(f"{context} canonical JSON")
    return value


def strict(value: object, keys: set[str], context: str) -> dict:
    if not isinstance(value, dict) or set(value) != keys:
        fail(f"{context} fields")
    return value


def bytes_identity(raw: bytes, role: str) -> dict:
    return {
        "byte_length": len(raw),
        "role": role,
        "sha256": hashlib.sha256(raw).hexdigest(),
    }


def file_identity(path: Path, role: str, limit: int) -> dict:
    return bytes_identity(bounded_read(path, role, limit), role)


def empty_file(path: Path, role: str) -> dict:
    identity = file_identity(path, role, MAX_DOCUMENT)
    if identity["byte_length"] != 0 or identity["sha256"] != EMPTY_SHA256:
        fail("realization diagnostics")
    return identity


def _name(raw: bytes) -> str:
    try:
        return raw.split(b"\0", 1)[0].decode("ascii")
    except UnicodeDecodeError as error:
        fail(f"Mach-O name: {error}")


def _command_string(raw: bytes, offset: int, command_size: int, context: str) -> str:
    if offset < 8 or offset >= command_size:
        fail(f"Mach-O {context} string offset")
    end = raw.find(b"\0", offset, command_size)
    if end < 0:
        fail(f"Mach-O {context} string terminator")
    try:
        return raw[offset:end].decode("ascii")
    except UnicodeDecodeError as error:
        fail(f"Mach-O {context} string: {error}")


def validate_macho(raw: bytes) -> dict:
    """Validate the exact bounded executable container needed by Delta V1."""

    if len(raw) > MAX_ARTIFACT_BYTES:
        resource("Mach-O artifact byte ceiling")
    if len(raw) < 32:
        fail("Mach-O header extent")
    magic, cpu, subtype, filetype, count, command_bytes, flags, reserved = struct.unpack_from(
        "<IIIIIIII", raw, 0
    )
    if (
        magic != MH_MAGIC_64
        or cpu != CPU_TYPE_ARM64
        or subtype != CPU_SUBTYPE_ARM64_ALL
        or filetype != MH_EXECUTE
        or reserved != 0
        or flags != MH_EXACT_FLAGS
    ):
        fail("Mach-O target/header profile")
    if count == 0 or count > 128 or command_bytes > 64 * 1024:
        resource("Mach-O load-command ceiling")
    commands_end = 32 + command_bytes
    if commands_end > len(raw):
        fail("Mach-O load-command extent")

    offset = 32
    segments: dict[str, dict] = {}
    sections: dict[tuple[str, str], dict] = {}
    segment_sections: dict[str, list[dict]] = {}
    dylibs: list[str] = []
    dylinker: str | None = None
    entry_offset: int | None = None
    minimum_os: int | None = None
    sdk: int | None = None
    dyld_info: tuple[int, ...] | None = None
    symbol_table: tuple[int, int, int, int] | None = None
    dynamic_symbol_table: tuple[int, ...] | None = None
    linkedit_data: dict[int, tuple[int, int]] = {}
    seen_unique: set[int] = set()
    for load_index in range(count):
        if offset + 8 > commands_end:
            fail("Mach-O load-command header extent")
        command, size = struct.unpack_from("<II", raw, offset)
        if size < 8 or size % 8 != 0 or offset + size > commands_end:
            fail("Mach-O load-command size")
        payload = raw[offset : offset + size]
        if command in (LC_UUID, LC_CODE_SIGNATURE):
            fail("Mach-O nondeterministic identity metadata")
        if command in (LC_ENCRYPTION_INFO, LC_ENCRYPTION_INFO_64):
            fail("Mach-O encrypted text")
        if command in (
            LC_UNIXTHREAD, LC_VERSION_MIN_MACOSX, LC_LAZY_LOAD_DYLIB,
            LC_LOAD_WEAK_DYLIB, LC_REEXPORT_DYLIB, LC_LOAD_UPWARD_DYLIB,
            LC_RPATH,
        ):
            fail("Mach-O alternate entry/dependency profile")
        if command == LC_SEGMENT_64:
            if size < 72:
                fail("Mach-O segment extent")
            _, _, segment_raw, vmaddr, vmsize, fileoff, filesize, maxprot, initprot, section_count, segment_flags = struct.unpack_from(
                "<II16sQQQQiiII", payload, 0
            )
            if size != 72 + section_count * 80 or section_count > 64:
                fail("Mach-O section extent")
            segment = _name(segment_raw)
            if not segment or segment in segments:
                fail("Mach-O segment identity")
            if fileoff > len(raw) or filesize > len(raw) - fileoff:
                fail("Mach-O segment file range")
            segments[segment] = {
                "file_offset": fileoff,
                "file_size": filesize,
                "initial_protection": initprot,
                "maximum_protection": maxprot,
                "virtual_address": vmaddr,
                "virtual_size": vmsize,
                "flags": segment_flags,
                "load_index": load_index,
                "section_count": section_count,
            }
            segment_sections[segment] = []
            section_offset = 72
            for _ in range(section_count):
                section = struct.unpack_from("<16s16sQQIIIIIIII", payload, section_offset)
                section_name, section_segment = _name(section[0]), _name(section[1])
                address, extent, file_offset = section[2], section[3], section[4]
                key = (section_segment, section_name)
                if section_segment != segment or not section_name or key in sections:
                    fail("Mach-O section identity")
                section_type = section[8] & 0xFF
                if section_type != 1 and (file_offset > len(raw) or extent > len(raw) - file_offset):
                    fail("Mach-O section file range")
                if section_type != 1 and (
                    file_offset < fileoff
                    or extent > filesize
                    or file_offset - fileoff > filesize - extent
                ):
                    fail("Mach-O section/segment file relation")
                if address < vmaddr or extent > vmsize or address - vmaddr > vmsize - extent:
                    fail("Mach-O section virtual range")
                sections[key] = {
                    "address": address,
                    "byte_length": extent,
                    "file_offset": file_offset,
                    "flags": section[8],
                    "relocation_count": section[7],
                    "relocation_offset": section[6],
                    "reserved": section[9:12],
                }
                segment_sections[segment].append(sections[key])
                section_offset += 80
        elif command == LC_MAIN:
            if command in seen_unique or size != 24:
                fail("Mach-O main command")
            _, _, entry_offset, stack_size = struct.unpack_from("<IIQQ", payload, 0)
            if stack_size != 0:
                fail("Mach-O implicit stack request")
            seen_unique.add(command)
        elif command == LC_BUILD_VERSION:
            if command in seen_unique or size < 24:
                fail("Mach-O build-version command")
            _, _, platform, minimum_os, sdk, tool_count = struct.unpack_from("<IIIIII", payload, 0)
            if platform != PLATFORM_MACOS or minimum_os != MIN_MACOS_11 or size != 24 + tool_count * 8:
                fail("Mach-O build-version target")
            seen_unique.add(command)
        elif command == LC_LOAD_DYLINKER:
            if command in seen_unique or size < 16:
                fail("Mach-O dynamic-linker command")
            dylinker = _command_string(payload, struct.unpack_from("<I", payload, 8)[0], size, "dylinker")
            seen_unique.add(command)
        elif command == LC_LOAD_DYLIB:
            if size < 24:
                fail("Mach-O dylib command")
            dylibs.append(_command_string(payload, struct.unpack_from("<I", payload, 8)[0], size, "dylib"))
        elif command == LC_DYLD_INFO_ONLY:
            if command in seen_unique or size != 48:
                fail("Mach-O dyld-info command")
            dyld_info = struct.unpack_from("<10I", payload, 8)
            seen_unique.add(command)
        elif command == LC_SYMTAB:
            if command in seen_unique or size != 24:
                fail("Mach-O symbol-table command")
            symbol_table = struct.unpack_from("<4I", payload, 8)
            seen_unique.add(command)
        elif command == LC_DYSYMTAB:
            if command in seen_unique or size != 80:
                fail("Mach-O dynamic-symbol-table command")
            dynamic_symbol_table = struct.unpack_from("<18I", payload, 8)
            seen_unique.add(command)
        elif command == LC_SOURCE_VERSION:
            if command in seen_unique or size != 16 or struct.unpack_from("<Q", payload, 8)[0] != 0:
                fail("Mach-O source-version command")
            seen_unique.add(command)
        elif command in (LC_FUNCTION_STARTS, LC_DATA_IN_CODE):
            if command in seen_unique or size != 16:
                fail("Mach-O link-edit data command")
            linkedit_data[command] = struct.unpack_from("<II", payload, 8)
            seen_unique.add(command)
        else:
            # V1 is a closed container profile. Silently accepting a new load
            # command would let an unmodeled dependency, fixup mechanism, or
            # identity-bearing payload enter an otherwise valid receipt.
            fail("Mach-O load-command profile")
        offset += size

    if offset != commands_end:
        fail("Mach-O load-command coverage")
    required_segments = {"__PAGEZERO", "__TEXT", "__DATA", "__LINKEDIT"}
    if not required_segments.issubset(segments) or not set(segments).issubset(
        required_segments | {"__DATA_CONST"}
    ):
        fail("Mach-O required segments")
    text = sections.get(("__TEXT", "__text"))
    bss = sections.get(("__DATA", "__bss"))
    if text is None or bss is None or text["byte_length"] == 0:
        fail("Mach-O required sections")
    if (
        segments["__PAGEZERO"]["file_size"] != 0
        or segments["__PAGEZERO"]["virtual_address"] != 0
        or segments["__PAGEZERO"]["initial_protection"] != 0
        or segments["__PAGEZERO"]["maximum_protection"] != 0
        or segments["__TEXT"]["file_offset"] != 0
        or segments["__TEXT"]["initial_protection"] != 5
        or segments["__TEXT"]["maximum_protection"] != 5
        or segments["__DATA"]["initial_protection"] != 3
        or segments["__DATA"]["maximum_protection"] != 3
        or segments["__LINKEDIT"]["initial_protection"] != 1
        or segments["__LINKEDIT"]["maximum_protection"] != 1
        or "__DATA_CONST" in segments
        and (
            segments["__DATA_CONST"]["initial_protection"] != 3
            or segments["__DATA_CONST"]["maximum_protection"] != 3
        )
    ):
        fail("Mach-O page-zero profile")
    ordered_segment_names = ["__PAGEZERO", "__TEXT"]
    if "__DATA_CONST" in segments:
        ordered_segment_names.append("__DATA_CONST")
    ordered_segment_names.extend(("__DATA", "__LINKEDIT"))
    ordered_segments = [segments[name] for name in ordered_segment_names]
    if [row["load_index"] for row in ordered_segments] != sorted(
        row["load_index"] for row in ordered_segments
    ) or segments["__LINKEDIT"]["load_index"] != max(
        row["load_index"] for row in segments.values()
    ):
        fail("Mach-O segment command order")
    previous_virtual_end = 0
    previous_file_end = 0
    for name, segment in zip(ordered_segment_names, ordered_segments):
        if segment["virtual_size"] == 0 or segment["virtual_address"] < previous_virtual_end:
            fail("Mach-O segment virtual topology")
        previous_virtual_end = segment["virtual_address"] + segment["virtual_size"]
        if segment["file_size"]:
            if segment["file_offset"] < previous_file_end:
                fail("Mach-O segment file overlap")
            previous_file_end = segment["file_offset"] + segment["file_size"]
    if commands_end > segments["__TEXT"]["file_size"]:
        fail("Mach-O load commands outside text segment")
    if segments["__LINKEDIT"]["file_offset"] + segments["__LINKEDIT"]["file_size"] != len(raw):
        fail("Mach-O complete file coverage")
    if segments["__LINKEDIT"]["section_count"] != 0:
        fail("Mach-O link-edit sections")
    if bss["flags"] & 0xFF != 1 or bss["file_offset"] != 0:
        fail("Mach-O BSS profile")

    for key, section in sections.items():
        if key not in SECTION_FLAGS or section["flags"] != SECTION_FLAGS[key]:
            fail("Mach-O section profile")
        if section["relocation_offset"] != 0 or section["relocation_count"] != 0:
            fail("Mach-O final-image section relocations")
        _, section_name = key
        reserved1, reserved2, reserved3 = section["reserved"]
        if reserved3 != 0:
            fail("Mach-O section reserved fields")
        if section_name == "__stubs":
            if reserved2 != 12:
                fail("Mach-O ARM64 stub width")
        elif section_name in ("__got", "__la_symbol_ptr"):
            if reserved2 != 0:
                fail("Mach-O pointer-section reserved fields")
        elif reserved1 != 0 or reserved2 != 0:
            fail("Mach-O section reserved fields")

    for segment_name, retained in segment_sections.items():
        virtual_ranges = sorted(
            (row["address"], row["address"] + row["byte_length"])
            for row in retained if row["byte_length"]
        )
        if any(right_start < left_end for (_, left_end), (right_start, _) in zip(
            virtual_ranges, virtual_ranges[1:]
        )):
            fail("Mach-O section virtual overlap")
        file_ranges = sorted(
            (row["file_offset"], row["file_offset"] + row["byte_length"])
            for row in retained if row["byte_length"] and row["flags"] & 0xFF != 1
        )
        if any(right_start < left_end for (_, left_end), (right_start, _) in zip(
            file_ranges, file_ranges[1:]
        )):
            fail("Mach-O section file overlap")
        if segment_name == "__TEXT" and file_ranges and file_ranges[0][0] < commands_end:
            fail("Mach-O text section/load-command overlap")

    linkedit_start = segments["__LINKEDIT"]["file_offset"]
    linkedit_end = linkedit_start + segments["__LINKEDIT"]["file_size"]

    def linkedit_range(file_offset: int, extent: int, context: str) -> None:
        if extent == 0:
            if file_offset != 0 and not linkedit_start <= file_offset <= linkedit_end:
                fail(f"Mach-O {context} empty range")
            return
        if file_offset < linkedit_start or extent > linkedit_end - file_offset:
            fail(f"Mach-O {context} range")

    if dyld_info is not None:
        for index in range(0, len(dyld_info), 2):
            linkedit_range(dyld_info[index], dyld_info[index + 1], "dyld info")
    if symbol_table is not None:
        symbol_offset, symbol_count, string_offset, string_extent = symbol_table
        linkedit_range(symbol_offset, symbol_count * 16, "symbol table")
        linkedit_range(string_offset, string_extent, "string table")
    if dynamic_symbol_table is not None:
        if symbol_table is None:
            fail("Mach-O dynamic symbol table without symbol table")
        symbol_count = symbol_table[1]
        local_index, local_count, external_index, external_count, undefined_index, undefined_count = dynamic_symbol_table[:6]
        if (
            local_index != 0
            or external_index != local_count
            or undefined_index != external_index + external_count
            or undefined_index + undefined_count != symbol_count
        ):
            fail("Mach-O dynamic symbol partition")
        table_shapes = (
            (dynamic_symbol_table[6], dynamic_symbol_table[7], 8, "table of contents"),
            (dynamic_symbol_table[8], dynamic_symbol_table[9], 56, "module table"),
            (dynamic_symbol_table[10], dynamic_symbol_table[11], 4, "external references"),
            (dynamic_symbol_table[12], dynamic_symbol_table[13], 4, "indirect symbols"),
            (dynamic_symbol_table[14], dynamic_symbol_table[15], 8, "external relocations"),
            (dynamic_symbol_table[16], dynamic_symbol_table[17], 8, "local relocations"),
        )
        for file_offset, row_count, row_size, context in table_shapes:
            linkedit_range(file_offset, row_count * row_size, context)
    for command, (file_offset, extent) in linkedit_data.items():
        context = "function starts" if command == LC_FUNCTION_STARTS else "data in code"
        linkedit_range(file_offset, extent, context)
    if minimum_os != MIN_MACOS_11 or sdk is None:
        fail("Mach-O missing build-version target")
    if entry_offset is None or not (
        text["file_offset"] <= entry_offset < text["file_offset"] + text["byte_length"]
    ):
        fail("Mach-O entry relation")
    if dylinker != "/usr/lib/dyld" or dylibs != ["/usr/lib/libSystem.B.dylib"]:
        fail("Mach-O dynamic dependency closure")

    return {
        "dynamic_linker": dylinker,
        "dynamic_libraries": dylibs,
        "entry_file_offset": entry_offset,
        "format": "mach-o-64",
        "minimum_macos": "11.0.0",
        "sdk_version_raw": sdk,
        "target": "macos_arm64",
        "text": {
            "byte_length": text["byte_length"],
            "file_offset": text["file_offset"],
        },
    }


def validate_status_elapsed(status: int, elapsed_ms: int) -> None:
    if isinstance(status, bool) or not isinstance(status, int) or status != 0:
        fail("realization status")
    if (
        isinstance(elapsed_ms, bool)
        or not isinstance(elapsed_ms, int)
        or elapsed_ms < 0
        or elapsed_ms > (1 << 63) - 1
    ):
        fail("realization elapsed milliseconds")


def make_observation(
    status: int,
    elapsed_ms: int,
    assembly: Path,
    artifact: Path,
    stdout: Path,
    stderr: Path,
    clang: Path,
    linker: Path,
    sdk_settings: Path,
    libsystem: Path,
    compiler_runtime: Path,
) -> dict:
    validate_status_elapsed(status, elapsed_ms)
    assembly_role = "darwin_arm64_assembly_stdout"
    assembly_raw = bounded_read(
        assembly, assembly_role, support.MAX_ASSEMBLY_BYTES
    )
    assembly_identity = bytes_identity(assembly_raw, assembly_role)
    try:
        support.validate_darwin_arm64_assembly(assembly_raw)
    except support.PublicationSupportResourceError as error:
        raise CustodyResourceError(str(error)) from error
    except support.PublicationSupportError as error:
        fail(str(error))
    artifact_role = "unsigned_darwin_arm64_macho_executable"
    artifact_raw = bounded_read(artifact, artifact_role, MAX_ARTIFACT_BYTES)
    artifact_identity = bytes_identity(artifact_raw, artifact_role)
    target = validate_macho(artifact_raw)
    return {
        "artifact": artifact_identity,
        "assembly": assembly_identity,
        "command_profile": COMMAND_PROFILE,
        "elapsed_milliseconds": elapsed_ms,
        "schema": OBSERVATION_SCHEMA,
        "status": status,
        "stderr": empty_file(stderr, "apple_clang_diagnostic_stderr"),
        "stdout": empty_file(stdout, "apple_clang_diagnostic_stdout"),
        "target": target,
        "toolchain": {
            "clang_driver": file_identity(
                clang, "ambient_apple_clang_driver", MAX_TOOLCHAIN_COMPONENT_BYTES
            ),
            "compiler_runtime": file_identity(
                compiler_runtime, "ambient_clang_runtime_archive", MAX_TOOLCHAIN_COMPONENT_BYTES
            ),
            "libsystem_stub": file_identity(
                libsystem, "ambient_macos_sdk_libsystem_stub", MAX_SDK_COMPONENT_BYTES
            ),
            "linker": file_identity(
                linker, "ambient_apple_linker", MAX_TOOLCHAIN_COMPONENT_BYTES
            ),
            "sdk_settings": file_identity(
                sdk_settings, "ambient_macos_sdk_settings", MAX_SDK_COMPONENT_BYTES
            ),
        },
    }


OBSERVATION_KEYS = {
    "artifact", "assembly", "command_profile", "elapsed_milliseconds", "schema",
    "status", "stderr", "stdout", "target", "toolchain",
}


def receipt_digest(receipt: dict) -> str:
    projection = {key: value for key, value in receipt.items() if key != "receipt_sha256"}
    compact = canonical_json(projection, pretty=False)
    return hashlib.sha256(
        RECEIPT_DOMAIN + len(compact).to_bytes(8, "little") + compact
    ).hexdigest()


def rederive_assembly_receipt(receipt_path: Path, join_arguments: list[str]) -> dict:
    candidate = load_json(receipt_path, "assembly receipt")
    expected = assembly_publication.make_receipt(
        *assembly_publication.parse_join(join_arguments)
    )
    if candidate != expected or candidate.get("receipt_sha256") != assembly_publication.receipt_digest(candidate):
        fail("assembly receipt custody")
    return candidate


def make_receipt(
    assembly_receipt_path: Path,
    observation_path: Path,
    assembly: Path,
    artifact: Path,
    stdout: Path,
    stderr: Path,
    clang: Path,
    linker: Path,
    sdk_settings: Path,
    libsystem: Path,
    compiler_runtime: Path,
    assembly_join_arguments: list[str],
) -> dict:
    parent = rederive_assembly_receipt(assembly_receipt_path, assembly_join_arguments)
    observation = load_json(observation_path, "realization observation")
    strict(observation, OBSERVATION_KEYS, "realization observation")
    expected_observation = make_observation(
        observation.get("status"), observation.get("elapsed_milliseconds"),
        assembly, artifact, stdout, stderr, clang, linker, sdk_settings,
        libsystem, compiler_runtime,
    )
    if observation != expected_observation:
        fail("realization observation custody")
    if observation["assembly"] != parent["assembly"]:
        fail("assembly/artifact cross-pair")

    receipt = {
        "artifact": observation["artifact"],
        "assembly": observation["assembly"],
        "assembly_publication": {
            "publication_id": parent["publication_id"],
            "receipt_sha256": parent["receipt_sha256"],
            "source_image": parent["source_image"],
            "source_snapshot": parent["source_snapshot"],
        },
        "claim": CLAIM,
        "format_profile": FORMAT_PROFILE,
        "open_refinement": OPEN_REFINEMENT,
        "publication_id": PUBLICATION_ID,
        "realization": observation,
        "receipt_sha256": "0" * 64,
        "schema": RECEIPT_SCHEMA,
        "target": {
            "abi": "delta.sealed-stream-compiler-v1",
            "configuration": "conservative-unsigned-content-image",
            "target": "macos_arm64",
        },
    }
    receipt["receipt_sha256"] = receipt_digest(receipt)
    if len(canonical_json(receipt, pretty=True)) > MAX_DOCUMENT:
        resource("artifact custody receipt byte ceiling")
    return receipt


def parse_join(arguments: list[str]) -> tuple:
    if len(arguments) < 29:
        fail("join arguments")
    fixed = list(map(Path, arguments[:11]))
    return (*fixed, arguments[11:])


def main(arguments: list[str]) -> int:
    if not arguments:
        fail("command")
    command, *rest = arguments
    if command == "observe" and len(rest) == 11:
        status, elapsed = map(int, rest[:2])
        paths = list(map(Path, rest[2:]))
        value = make_observation(status, elapsed, *paths)
        sys.stdout.buffer.write(canonical_json(value, pretty=True))
        return 0
    if command == "generate":
        value = make_receipt(*parse_join(rest))
        sys.stdout.buffer.write(canonical_json(value, pretty=True))
        return 0
    if command == "verify" and rest:
        candidate = load_json(Path(rest[0]), "artifact custody receipt")
        expected = make_receipt(*parse_join(rest[1:]))
        if candidate != expected or candidate.get("receipt_sha256") != receipt_digest(candidate):
            fail("artifact custody receipt")
        return 0
    fail("command")


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except (
        CustodyError,
        assembly_publication.ReceiptError,
        support.PublicationSupportError,
        OSError,
        ValueError,
    ) as error:
        status = getattr(error, "status", 251)
        print(f"Delta artifact custody V1: {error}", file=sys.stderr)
        raise SystemExit(status)
