#!/usr/bin/env python3
"""Independent fixed-shape OMGCOMP2 Console exit custody fixture."""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
import sys
from pathlib import Path


HERE = Path(__file__).resolve().parent
COMPILER = HERE.parent / "compiler"
FIXTURE = HERE / "fixtures" / "omgcomp2-console-exit-provider"
sys.path.insert(0, str(COMPILER))

import omega_bootstrap_bundle as source_bundle  # noqa: E402
import omega_bootstrap_compilation as compilation_v1  # noqa: E402
import omega_bootstrap_compilation_v2 as compilation_v2  # noqa: E402


HEADER = struct.Struct("<8sHHHH12I")
PACKAGE = struct.Struct("<I32sIII")
SOURCE = struct.Struct("<IIIII")
ALIAS = struct.Struct("<IIII")
U32 = struct.Struct("<I")

STD_KEY = bytes.fromhex("11" * 32)
APP_KEY = bytes.fromhex("22" * 32)
EXPECTED_SHA256 = "9f4db7352836deae6648c34301d2b3b63b4233fa679de40591bd3b2d605167de"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def build_bundle() -> bytes:
    return source_bundle.encode(
        [
            source_bundle.Entry("app/main.omg", (FIXTURE / "app-main.omg").read_bytes()),
            source_bundle.Entry("omega/std/console.omg", (FIXTURE / "console.omg").read_bytes()),
            source_bundle.Entry(
                "omega/std/targets/linux_x64/console_impl.omg",
                (FIXTURE / "console-impl-linux-x64.omg").read_bytes(),
            ),
        ]
    )


def reference_encode(bundle: bytes) -> bytes:
    # Canonical strings by raw byte order: selected owner, app module, shared
    # console module, selected machine, and requester-local package alias.
    strings = [b"Main", b"app", b"console", b"main", b"omega_std"]
    string_table = b"".join(U32.pack(len(value)) + value for value in strings)

    fixed = b"".join(
        [
            PACKAGE.pack(0, STD_KEY, 0, 2, 0),
            PACKAGE.pack(1, APP_KEY, 2, 1, 0),
            SOURCE.pack(0, 0, 1, 2, 0),
            SOURCE.pack(1, 0, 2, 2, 0),
            SOURCE.pack(2, 1, 0, 1, 0),
            ALIAS.pack(1, 4, 0, 0),
        ]
    )
    total = HEADER.size + len(fixed) + len(string_table) + len(bundle)
    header = HEADER.pack(
        b"OMGCOMP\0",
        2,
        0,
        1,
        0,
        total,
        len(bundle),
        len(string_table),
        len(strings),
        2,
        3,
        1,
        1,
        2,
        0,
        3,
        1,
    )
    return header + fixed + string_table + bundle


def replace_u16(data: bytes, offset: int, value: int) -> bytes:
    result = bytearray(data)
    struct.pack_into("<H", result, offset, value)
    return bytes(result)


def replace_u32(data: bytes, offset: int, value: int) -> bytes:
    result = bytearray(data)
    struct.pack_into("<I", result, offset, value)
    return bytes(result)


def expected_status(name: str, status: int, action) -> None:
    try:
        action()
    except compilation_v2.CompilationError as error:
        require(error.status == status, f"{name}: expected {status}, got {error.status}")
    else:
        require(status == 0, f"{name}: unexpectedly accepted")


def check_fixture(envelope: bytes, bundle: bytes) -> None:
    manifest = json.loads((FIXTURE / "manifest.json").read_text(encoding="utf-8"))
    packed = compilation_v2.encode_manifest(manifest, bundle)
    require(packed == envelope, "V2 packer differs from independent reference bytes")
    digest = hashlib.sha256(envelope).hexdigest()
    require(digest == EXPECTED_SHA256, "canonical OMGCOMP2 digest drift")

    decoded = compilation_v2.decode(envelope)
    view = compilation_v2.inspect(decoded)
    require(view["schema"] == "omega-bootstrap-compilation-envelope-v2", "inspection schema")
    require(view["target"] == "linux_x86_64", "inspection target")
    require(view["configuration"] == {"native_provider_substitution": True}, "inspection configuration")
    require([row["id"] for row in view["packages"]] == [0, 1], "package order")
    require([row["label"] for row in view["sources"]] == [
        "omega/std/console.omg",
        "omega/std/targets/linux_x64/console_impl.omg",
        "app/main.omg",
    ], "source custody order")
    require([row["module"] for row in view["sources"]] == ["console", "console", "app"], "logical placements")
    require(view["aliases"] == [{"requester": 1, "alias": "omega_std", "target": 0}], "direct alias")
    require(view["root"] == {"package": 1, "source": 2, "owner": "Main", "machine": "main"}, "selected root")

    v1_pair = replace_u32(replace_u16(envelope, 8, 1), 60, 0)
    compilation_v1.decode(v1_pair)
    expected_status("V2 decoder rejects V1", 251, lambda: compilation_v2.decode(v1_pair))
    expected_status("V2/config0 cross-pair", 251, lambda: compilation_v2.decode(replace_u32(envelope, 60, 0)))
    expected_status("V2/config2", 251, lambda: compilation_v2.decode(replace_u32(envelope, 60, 2)))
    expected_status("V3", 251, lambda: compilation_v2.decode(replace_u16(envelope, 8, 3)))
    expected_status("wrong target", 251, lambda: compilation_v2.decode(replace_u16(envelope, 12, 2)))
    expected_status("package ceiling", 252, lambda: compilation_v2.decode(replace_u32(envelope, 32, 17)))


def build(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    bundle = build_bundle()
    envelope = reference_encode(bundle)
    check_fixture(envelope, bundle)

    cases: list[tuple[str, int, bytes]] = []
    cases.append(("canonical-v2", 0, envelope))
    cases.append(("canonical-v1-structural", 0, replace_u32(replace_u16(envelope, 8, 1), 60, 0)))
    opaque = bytearray(envelope)
    marker = opaque.find(b"exit_process(70)")
    require(marker >= 0, "opaque body marker absent")
    opaque[marker + len(b"exit_process(")] = ord("7")
    opaque[marker + len(b"exit_process(") + 1] = ord("1")
    cases.append(("canonical-opaque-body-mutation", 0, bytes(opaque)))
    cases.extend(
        [
            ("reject-v1-config1-cross-pair", 251, replace_u16(envelope, 8, 1)),
            ("reject-v2-config0-cross-pair", 251, replace_u32(envelope, 60, 0)),
            ("reject-v2-config2", 251, replace_u32(envelope, 60, 2)),
            ("reject-v3", 251, replace_u16(envelope, 8, 3)),
            ("reject-target", 251, replace_u16(envelope, 12, 2)),
            ("reject-flags", 251, replace_u16(envelope, 14, 1)),
            ("reject-source-owner", 251, replace_u32(envelope, 64 + 2 * 48 + 4, 1)),
            ("reject-trailing-eof", 251, envelope + b"x"),
            ("exhaust-package-count-17", 252, replace_u32(envelope, 32, 17)),
            ("exhaust-input-adjacent", 252, bytes(267_281)),
        ]
    )

    (output / "source.bundle").write_bytes(bundle)
    (output / "reference.omgc").write_bytes(envelope)
    (output / "reference.sha256").write_text(hashlib.sha256(envelope).hexdigest() + "\n", encoding="ascii")
    (output / "inspection.json").write_text(
        json.dumps(compilation_v2.inspect(compilation_v2.decode(envelope)), indent=2, sort_keys=True) + "\n",
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
    print("OMGCOMP2 fixture/reference: PASS")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("build", "check"))
    parser.add_argument("output", type=Path)
    arguments = parser.parse_args()
    if arguments.command == "build":
        build(arguments.output)
    else:
        check(arguments.output)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, source_bundle.BundleError, compilation_v2.CompilationError) as error:
        raise SystemExit(f"OMGCOMP2 Console exit fixture: {error}")
