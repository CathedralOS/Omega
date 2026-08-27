#!/usr/bin/env python3
"""Independent OMGCOMP3 build-source custody fixture and controls."""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
import sys
from pathlib import Path


HERE = Path(__file__).resolve().parent
COMPILER = HERE.parent / "compiler"
FIXTURE = HERE / "fixtures" / "omgcomp3-console-provider-plan"
sys.path.insert(0, str(COMPILER))

import omega_bootstrap_bundle as source_bundle  # noqa: E402
import omega_bootstrap_compilation as compilation_v1  # noqa: E402
import omega_bootstrap_compilation_v2 as compilation_v2  # noqa: E402
import omega_bootstrap_compilation_v3 as compilation_v3  # noqa: E402


HEADER = struct.Struct("<8sHHHH12I")
PACKAGE = struct.Struct("<I32sIII")
SOURCE = struct.Struct("<IIIII")
ALIAS = struct.Struct("<IIII")
U32 = struct.Struct("<I")

STD_KEY = bytes.fromhex("11" * 32)
APP_KEY = bytes.fromhex("22" * 32)
EXPECTED_SHA256 = "89f9e825c7e0c4def676042d5b61eab135e422512fa751f5aae456aa0f08b5c0"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def build_bundle() -> bytes:
    return source_bundle.encode(
        [
            source_bundle.Entry("app/build.omg", (FIXTURE / "build.omg").read_bytes()),
            source_bundle.Entry("app/main.omg", (FIXTURE / "app-main.omg").read_bytes()),
            source_bundle.Entry("omega/std/console.omg", (FIXTURE / "console.omg").read_bytes()),
            source_bundle.Entry(
                "omega/std/targets/linux_x64/console_impl.omg",
                (FIXTURE / "console-impl-linux-x64.omg").read_bytes(),
            ),
        ]
    )


def reference_encode(bundle: bytes) -> bytes:
    strings = [b"Main", b"app", b"console", b"main", b"omega_std"]
    string_table = b"".join(U32.pack(len(value)) + value for value in strings)
    fixed = b"".join(
        [
            PACKAGE.pack(0, STD_KEY, 0, 2, 0),
            PACKAGE.pack(1, APP_KEY, 2, 2, 0),
            SOURCE.pack(0, 0, 2, 2, 0),
            SOURCE.pack(1, 0, 3, 2, 0),
            SOURCE.pack(2, 1, 0, 1, 1),
            SOURCE.pack(3, 1, 1, 1, 0),
            ALIAS.pack(1, 4, 0, 0),
        ]
    )
    total = HEADER.size + len(fixed) + len(string_table) + len(bundle)
    return HEADER.pack(
        b"OMGCOMP\0", 3, 0, 1, 0, total, len(bundle), len(string_table),
        len(strings), 2, 4, 1, 1, 3, 0, 3, 1,
    ) + fixed + string_table + bundle


def replace_u16(data: bytes, offset: int, value: int) -> bytes:
    result = bytearray(data)
    struct.pack_into("<H", result, offset, value)
    return bytes(result)


def replace_u32(data: bytes, offset: int, value: int) -> bytes:
    result = bytearray(data)
    struct.pack_into("<I", result, offset, value)
    return bytes(result)


def source_flag_offset(source_id: int) -> int:
    return HEADER.size + 2 * PACKAGE.size + source_id * SOURCE.size + 16


def expected_status(name: str, status: int, action) -> None:
    try:
        action()
    except compilation_v3.CompilationError as error:
        require(error.status == status, f"{name}: expected {status}, got {error.status}")
    else:
        require(status == 0, f"{name}: unexpectedly accepted")


def check_fixture(envelope: bytes, bundle: bytes) -> None:
    manifest = json.loads((FIXTURE / "manifest.json").read_text(encoding="utf-8"))
    packed = compilation_v3.encode_manifest(manifest, bundle)
    require(packed == envelope, "V3 packer differs from independent reference bytes")
    require(hashlib.sha256(envelope).hexdigest() == EXPECTED_SHA256, "canonical OMGCOMP3 digest drift")
    decoded = compilation_v3.decode(envelope)
    view = compilation_v3.inspect(decoded)
    require(view["schema"] == "omega-bootstrap-compilation-envelope-v3", "inspection schema")
    require(view["configuration"] == {"native_provider_substitution": True}, "inspection configuration")
    require(view["root"] == {"package": 1, "source": 3, "owner": "Main", "machine": "main"}, "selected root")
    require(view["build"] == {"package": 1, "source": 2, "label": "app/build.omg", "module": "app"}, "build role")

    missing = replace_u32(envelope, source_flag_offset(2), 0)
    duplicate = replace_u32(envelope, source_flag_offset(3), 1)
    wrong_owner = replace_u32(replace_u32(envelope, source_flag_offset(2), 0), source_flag_offset(0), 1)
    expected_status("missing build role", 251, lambda: compilation_v3.decode(missing))
    expected_status("duplicate build role", 251, lambda: compilation_v3.decode(duplicate))
    expected_status("wrong-owner build role", 251, lambda: compilation_v3.decode(wrong_owner))
    expected_status("unknown source role", 251, lambda: compilation_v3.decode(replace_u32(envelope, source_flag_offset(2), 2)))

    v2_pair = replace_u16(missing, 8, 2)
    compilation_v2.decode(v2_pair)
    expected_status("V3 decoder rejects V2", 251, lambda: compilation_v3.decode(v2_pair))
    v1_pair = replace_u32(replace_u16(missing, 8, 1), 60, 0)
    compilation_v1.decode(v1_pair)


def build(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    bundle = build_bundle()
    envelope = reference_encode(bundle)
    check_fixture(envelope, bundle)
    missing = replace_u32(envelope, source_flag_offset(2), 0)
    cases = [
        ("canonical-v3", 0, envelope),
        ("canonical-v2-structural", 0, replace_u16(missing, 8, 2)),
        ("canonical-v1-structural", 0, replace_u32(replace_u16(missing, 8, 1), 60, 0)),
        ("reject-v3-no-build", 251, missing),
        ("reject-v3-two-builds", 251, replace_u32(envelope, source_flag_offset(3), 1)),
        ("reject-v3-build-wrong-owner", 251, replace_u32(replace_u32(envelope, source_flag_offset(2), 0), source_flag_offset(0), 1)),
        ("reject-v3-source-role-bits", 251, replace_u32(envelope, source_flag_offset(2), 2)),
        ("reject-v2-with-build-role", 251, replace_u16(envelope, 8, 2)),
        ("reject-v3-config0", 251, replace_u32(envelope, 60, 0)),
        ("reject-v3-target", 251, replace_u16(envelope, 12, 2)),
        ("reject-v3-trailing", 251, envelope + b"x"),
        ("exhaust-v3-package-count", 252, replace_u32(envelope, 32, 17)),
        ("exhaust-v3-input-adjacent", 252, bytes(267_281)),
    ]
    opaque = bytearray(envelope)
    marker = opaque.find(b"select_provider")
    require(marker >= 0, "opaque build spelling absent")
    opaque[marker] = ord("S")
    cases.insert(1, ("canonical-v3-opaque-source-mutation", 0, bytes(opaque)))

    (output / "source.bundle").write_bytes(bundle)
    (output / "reference.omgc").write_bytes(envelope)
    (output / "reference.sha256").write_text(hashlib.sha256(envelope).hexdigest() + "\n", encoding="ascii")
    (output / "inspection.json").write_text(
        json.dumps(compilation_v3.inspect(compilation_v3.decode(envelope)), indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    with (output / "cases.tsv").open("w", encoding="utf-8") as rows:
        for name, status, data in cases:
            path = output / f"{name}.omgc"
            path.write_bytes(data)
            rows.write(f"{name}\t{status}\t{path}\n")
    print(hashlib.sha256(envelope).hexdigest())


def check(output: Path) -> None:
    bundle = (output / "source.bundle").read_bytes()
    envelope = (output / "reference.omgc").read_bytes()
    require(envelope == reference_encode(bundle), "saved envelope is not the independent reference")
    check_fixture(envelope, bundle)
    print("OMGCOMP3 fixture/reference: PASS")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("build", "check"))
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    build(args.output) if args.command == "build" else check(args.output)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, source_bundle.BundleError, compilation_v3.CompilationError) as error:
        raise SystemExit(f"OMGCOMP3 build-source fixture: {error}")
