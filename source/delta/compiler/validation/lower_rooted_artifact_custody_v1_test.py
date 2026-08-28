#!/usr/bin/env python3
"""Focused tests for the non-authoritative Delta executable-custody join."""

from __future__ import annotations

import hashlib
import json
import platform
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
from pathlib import Path

import lower_rooted_artifact_custody_v1 as custody
import lower_rooted_assembly_publication_v1 as assembly_publication
from lower_rooted_assembly_publication_v1_test import Evidence as AssemblyEvidence
from publication_support_test import POSITIVE_ASSEMBLY


HERE = Path(__file__).resolve().parent


class ObservedPath:
    def __init__(self, path: Path) -> None:
        self.path = path
        self.opens = 0

    def open(self, mode: str):
        self.opens += 1
        return self.path.open(mode)

    def stat(self):
        return self.path.stat()


class MutatingStream:
    def __init__(self, path: Path, mutate) -> None:
        self.stream = path.open("rb")
        self.mutate = mutate
        self.mutated = False

    def __enter__(self):
        return self

    def __exit__(self, kind, value, traceback) -> None:
        self.stream.close()

    def fileno(self) -> int:
        return self.stream.fileno()

    def read(self, extent: int) -> bytes:
        raw = self.stream.read(extent)
        if not self.mutated:
            self.mutated = True
            self.mutate()
        return raw


class MutatingPath:
    def __init__(self, path: Path, mutate) -> None:
        self.path = path
        self.mutate = mutate

    def open(self, mode: str):
        if mode != "rb":
            raise AssertionError("bounded snapshot mode")
        return MutatingStream(self.path, self.mutate)

    def stat(self):
        return self.path.stat()


def padded_command(command: int, prefix: bytes, text: bytes) -> bytes:
    size = (8 + len(prefix) + len(text) + 1 + 7) & ~7
    return struct.pack("<II", command, size) + prefix + text + b"\0" + b"\0" * (
        size - 8 - len(prefix) - len(text) - 1
    )


def segment(
    name: bytes,
    vmaddr: int,
    vmsize: int,
    fileoff: int,
    filesize: int,
    protection: int,
    sections: bytes = b"",
) -> bytes:
    if len(sections) % 80:
        raise AssertionError("test section table extent")
    count = len(sections) // 80
    size = 72 + len(sections)
    return struct.pack(
        "<II16sQQQQiiII", custody.LC_SEGMENT_64, size,
        name.ljust(16, b"\0"), vmaddr, vmsize, fileoff, filesize,
        protection, protection, count, 0,
    ) + sections


def section(name: bytes, segment_name: bytes, address: int, extent: int, offset: int, flags: int) -> bytes:
    return struct.pack(
        "<16s16sQQIIIIIIII", name.ljust(16, b"\0"),
        segment_name.ljust(16, b"\0"), address, extent, offset,
        2, 0, 0, flags, 0, 0, 0,
    )


def minimal_macho(
    extra_commands: tuple[bytes, ...] = (), *, extra_executable_section: bool = False
) -> bytes:
    text_offset = 1024
    text_bytes = b"\xc0\x03\x5f\xd6"  # ret
    constant_bytes = b"ABC"
    extra_text_bytes = b"X" if extra_executable_section else b""
    extra_text_section = b""
    if extra_executable_section:
        extra_offset = text_offset + len(text_bytes) + len(constant_bytes)
        extra_text_section = section(
            b"__extra_exec", b"__TEXT", 0x100000000 + extra_offset,
            len(extra_text_bytes), extra_offset, 0x80000400,
        )
    linkedit = b"LINK"
    linkedit_offset = text_offset + len(text_bytes) + len(constant_bytes) + len(extra_text_bytes)
    commands = [
        segment(b"__PAGEZERO", 0, 0x100000000, 0, 0, 0),
        segment(
            b"__TEXT", 0x100000000, 0x1000, 0, linkedit_offset, 5,
            section(b"__text", b"__TEXT", 0x100000000 + text_offset, len(text_bytes), text_offset, 0x80000400)
            + section(b"__const", b"__TEXT", 0x100000000 + text_offset + len(text_bytes), len(constant_bytes), text_offset + len(text_bytes), 0)
            + extra_text_section,
        ),
        segment(
            b"__DATA", 0x100001000, 0x1000, 0, 0, 3,
            section(b"__bss", b"__DATA", 0x100001000, 4096, 0, 1),
        ),
        segment(b"__LINKEDIT", 0x100002000, 0x1000, linkedit_offset, len(linkedit), 1),
        padded_command(custody.LC_LOAD_DYLINKER, struct.pack("<I", 12), b"/usr/lib/dyld"),
        struct.pack("<IIIIII", custody.LC_BUILD_VERSION, 24, custody.PLATFORM_MACOS, custody.MIN_MACOS_11, 0x000F0500, 0),
        struct.pack("<IIQQ", custody.LC_MAIN, 24, text_offset, 0),
        padded_command(
            custody.LC_LOAD_DYLIB,
            struct.pack("<IIII", 24, 0, 0x00010000, 0x00010000),
            b"/usr/lib/libSystem.B.dylib",
        ),
        *extra_commands,
    ]
    load_commands = b"".join(commands)
    header = struct.pack(
        "<IIIIIIII", custody.MH_MAGIC_64, custody.CPU_TYPE_ARM64,
        custody.CPU_SUBTYPE_ARM64_ALL, custody.MH_EXECUTE, len(commands),
        len(load_commands), custody.MH_EXACT_FLAGS, 0,
    )
    prefix = header + load_commands
    if len(prefix) > text_offset:
        raise AssertionError("test Mach-O load commands exceed text offset")
    return (
        prefix + b"\0" * (text_offset - len(prefix)) + text_bytes
        + constant_bytes + extra_text_bytes + linkedit
    )


class CustodyEvidence:
    def __init__(self, root: Path) -> None:
        self.assembly_evidence = AssemblyEvidence(root / "assembly")
        self.assembly_evidence.receipt.write_bytes(
            assembly_publication.canonical_json(
                assembly_publication.make_receipt(
                    *assembly_publication.parse_join(self.assembly_evidence.join_arguments())
                ),
                pretty=True,
            )
        )
        self.assembly = root / "published.s"
        self.artifact = root / "candidate"
        self.stdout = root / "clang.stdout"
        self.stderr = root / "clang.stderr"
        self.clang = root / "clang"
        self.linker = root / "ld"
        self.sdk_settings = root / "SDKSettings.json"
        self.libsystem = root / "libSystem.tbd"
        self.runtime = root / "libclang_rt.osx.a"
        self.observation = root / "realization.json"
        self.receipt = root / "artifact-receipt.json"
        self.assembly.write_bytes(POSITIVE_ASSEMBLY)
        self.artifact.write_bytes(minimal_macho())
        self.stdout.write_bytes(b"")
        self.stderr.write_bytes(b"")
        for path, raw in (
            (self.clang, b"test apple clang\n"),
            (self.linker, b"test apple linker\n"),
            (self.sdk_settings, b'{"Version":"15.5"}\n'),
            (self.libsystem, b"test libSystem stub\n"),
            (self.runtime, b"test compiler runtime\n"),
        ):
            path.write_bytes(raw)
        self.write_observation()

    def write_observation(self) -> None:
        value = custody.make_observation(
            0, 23, self.assembly, self.artifact, self.stdout, self.stderr,
            self.clang, self.linker, self.sdk_settings, self.libsystem, self.runtime,
        )
        self.observation.write_bytes(custody.canonical_json(value, pretty=True))

    def join_arguments(self) -> list[str]:
        return [
            str(self.assembly_evidence.receipt), str(self.observation),
            str(self.assembly), str(self.artifact), str(self.stdout),
            str(self.stderr), str(self.clang), str(self.linker),
            str(self.sdk_settings), str(self.libsystem), str(self.runtime),
            *self.assembly_evidence.join_arguments(),
        ]


class ArtifactCustodyTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        root = Path(self.temporary.name)
        (root / "assembly").mkdir()
        self.evidence = CustodyEvidence(root)

    def run_cli(self, *arguments: str) -> subprocess.CompletedProcess[bytes]:
        return subprocess.run(
            [sys.executable, "-B", str(HERE / "lower_rooted_artifact_custody_v1.py"), *arguments],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
        )

    def assert_rejects(self, status: int, *arguments: str) -> None:
        result = self.run_cli(*arguments)
        self.assertEqual(result.returncode, status, result.stderr)
        self.assertEqual(result.stdout, b"")

    def test_exact_generate_and_verify_retains_open_refinement(self) -> None:
        generated = self.run_cli("generate", *self.evidence.join_arguments())
        self.assertEqual(generated.returncode, 0, generated.stderr)
        receipt = json.loads(generated.stdout)
        self.assertEqual(receipt["claim"], custody.CLAIM)
        self.assertEqual(receipt["open_refinement"], custody.OPEN_REFINEMENT)
        self.assertEqual(
            receipt["assembly_publication"]["receipt_sha256"],
            json.loads(self.evidence.assembly_evidence.receipt.read_bytes())["receipt_sha256"],
        )
        self.assertEqual(receipt["receipt_sha256"], custody.receipt_digest(receipt))
        self.evidence.receipt.write_bytes(generated.stdout)
        verified = self.run_cli(
            "verify", str(self.evidence.receipt), *self.evidence.join_arguments()
        )
        self.assertEqual(verified.returncode, 0, verified.stderr)
        self.assertEqual(verified.stdout, b"")

    def test_observe_reconstructs_exact_realization(self) -> None:
        result = self.run_cli(
            "observe", "0", "23", str(self.evidence.assembly),
            str(self.evidence.artifact), str(self.evidence.stdout),
            str(self.evidence.stderr), str(self.evidence.clang),
            str(self.evidence.linker), str(self.evidence.sdk_settings),
            str(self.evidence.libsystem), str(self.evidence.runtime),
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, self.evidence.observation.read_bytes())

    def test_bounded_identity_ceiling_is_inclusive(self) -> None:
        path = self.evidence.stdout
        with path.open("wb") as stream:
            stream.truncate(custody.MAX_DOCUMENT)
        identity = custody.file_identity(path, "bounded_document", custody.MAX_DOCUMENT)
        self.assertEqual(identity["byte_length"], custody.MAX_DOCUMENT)

        with path.open("r+b") as stream:
            stream.truncate(custody.MAX_DOCUMENT + 1)
        with self.assertRaises(custody.CustodyResourceError):
            custody.file_identity(path, "bounded_document", custody.MAX_DOCUMENT)

    def test_bounded_read_rejects_growth(self) -> None:
        path = self.evidence.stdout
        path.write_bytes(b"1234567")

        def grow() -> None:
            with path.open("ab") as stream:
                stream.write(b"8")

        with self.assertRaisesRegex(custody.CustodyError, "changed while reading"):
            custody.bounded_read(MutatingPath(path, grow), "growing input", 8)

    def test_bounded_read_rejects_path_replacement(self) -> None:
        path = self.evidence.stdout
        path.write_bytes(b"same bytes")
        original_inode = path.stat().st_ino

        def replace() -> None:
            replacement = path.with_name("replacement")
            replacement.write_bytes(b"same bytes")
            replacement.replace(path)

        with self.assertRaisesRegex(custody.CustodyError, "path changed while reading"):
            custody.bounded_read(MutatingPath(path, replace), "replaced input", 32)
        self.assertNotEqual(path.stat().st_ino, original_inode)

    def test_observation_validates_and_identifies_the_same_snapshots(self) -> None:
        assembly = ObservedPath(self.evidence.assembly)
        artifact = ObservedPath(self.evidence.artifact)
        seen_assembly: list[bytes] = []
        seen_artifact: list[bytes] = []
        validate_assembly = custody.support.validate_darwin_arm64_assembly
        validate_artifact = custody.validate_macho

        def capture_assembly(raw: bytes) -> None:
            seen_assembly.append(raw)
            validate_assembly(raw)

        def capture_artifact(raw: bytes) -> dict:
            seen_artifact.append(raw)
            return validate_artifact(raw)

        with (
            mock.patch.object(
                custody.support, "validate_darwin_arm64_assembly",
                side_effect=capture_assembly,
            ),
            mock.patch.object(custody, "validate_macho", side_effect=capture_artifact),
        ):
            observation = custody.make_observation(
                0, 23, assembly, artifact, self.evidence.stdout,
                self.evidence.stderr, self.evidence.clang, self.evidence.linker,
                self.evidence.sdk_settings, self.evidence.libsystem,
                self.evidence.runtime,
            )

        self.assertEqual(assembly.opens, 1)
        self.assertEqual(artifact.opens, 1)
        self.assertEqual(len(seen_assembly), 1)
        self.assertEqual(len(seen_artifact), 1)
        self.assertEqual(
            observation["assembly"]["sha256"],
            hashlib.sha256(seen_assembly[0]).hexdigest(),
        )
        self.assertEqual(
            observation["artifact"]["sha256"],
            hashlib.sha256(seen_artifact[0]).hexdigest(),
        )

    def test_parent_receipt_and_assembly_cross_pairs_reject(self) -> None:
        parent = json.loads(self.evidence.assembly_evidence.receipt.read_bytes())
        parent["receipt_sha256"] = "0" * 64
        self.evidence.assembly_evidence.receipt.write_bytes(
            custody.canonical_json(parent, pretty=True)
        )
        self.assert_rejects(251, "generate", *self.evidence.join_arguments())

        parent = assembly_publication.make_receipt(
            *assembly_publication.parse_join(self.evidence.assembly_evidence.join_arguments())
        )
        self.evidence.assembly_evidence.receipt.write_bytes(
            assembly_publication.canonical_json(parent, pretty=True)
        )
        changed = self.evidence.assembly.parent / "changed.s"
        changed.write_bytes(POSITIVE_ASSEMBLY.replace(b".byte 65,66,67", b".byte 65,66,68"))
        arguments = self.evidence.join_arguments()
        arguments[2] = str(changed)
        observation = custody.make_observation(
            0, 23, changed, self.evidence.artifact, self.evidence.stdout,
            self.evidence.stderr, self.evidence.clang, self.evidence.linker,
            self.evidence.sdk_settings, self.evidence.libsystem, self.evidence.runtime,
        )
        self.evidence.observation.write_bytes(custody.canonical_json(observation, pretty=True))
        self.assert_rejects(251, "generate", *arguments)

    def test_artifact_tool_and_observation_mutations_reject(self) -> None:
        raw = bytearray(self.evidence.artifact.read_bytes())
        raw[-1] ^= 1
        self.evidence.artifact.write_bytes(raw)
        self.assert_rejects(251, "generate", *self.evidence.join_arguments())

        self.evidence.artifact.write_bytes(minimal_macho())
        self.evidence.write_observation()
        arguments = self.evidence.join_arguments()
        arguments[7] = str(self.evidence.clang)
        self.assert_rejects(251, "generate", *arguments)

        arguments = self.evidence.join_arguments()
        observation = json.loads(self.evidence.observation.read_bytes())
        observation["command_profile"]["minimum_macos_argument"] = "-mmacosx-version-min=12.0"
        self.evidence.observation.write_bytes(custody.canonical_json(observation, pretty=True))
        self.assert_rejects(251, "generate", *arguments)

    def test_macho_target_identity_and_metadata_teeth(self) -> None:
        raw = bytearray(minimal_macho())
        struct.pack_into("<I", raw, 4, 0x01000007)
        with self.assertRaises(custody.CustodyError):
            custody.validate_macho(raw)

        raw = bytearray(minimal_macho())
        flags = struct.unpack_from("<I", raw, 24)[0]
        struct.pack_into("<I", raw, 24, flags | custody.MH_ALLOW_STACK_EXECUTION)
        with self.assertRaises(custody.CustodyError):
            custody.validate_macho(raw)

        raw = bytearray(minimal_macho())
        command_offset = 32
        struct.pack_into("<I", raw, command_offset, custody.LC_UUID)
        with self.assertRaises(custody.CustodyError):
            custody.validate_macho(raw)

        raw = bytearray(minimal_macho())
        main = raw.find(struct.pack("<I", custody.LC_MAIN), 32)
        self.assertGreater(main, 0)
        struct.pack_into("<Q", raw, main + 8, 7)
        with self.assertRaises(custody.CustodyError):
            custody.validate_macho(raw)

        raw = bytearray(minimal_macho())
        dylib = raw.find(b"/usr/lib/libSystem.B.dylib")
        self.assertGreater(dylib, 0)
        raw[dylib] = ord("x")
        with self.assertRaises(custody.CustodyError):
            custody.validate_macho(raw)

        raw = minimal_macho(extra_executable_section=True)
        with self.assertRaises(custody.CustodyError):
            custody.validate_macho(raw)

        raw = bytearray(minimal_macho())
        text = raw.find(b"__text")
        constant = raw.find(b"__const")
        self.assertGreaterEqual(text, 32)
        self.assertGreaterEqual(constant, 32)
        struct.pack_into("<Q", raw, constant + 32, struct.unpack_from("<Q", raw, text + 32)[0])
        struct.pack_into("<I", raw, constant + 48, struct.unpack_from("<I", raw, text + 48)[0])
        with self.assertRaises(custody.CustodyError):
            custody.validate_macho(raw)

        # The V1 container profile is closed: a structurally bounded but
        # unmodeled command cannot ride along outside the target summary.
        raw = minimal_macho((struct.pack("<IIQ", 0x12345678, 16, 0),))
        with self.assertRaises(custody.CustodyError):
            custody.validate_macho(raw)

        # Known link-edit commands are not mere spellings; every retained byte
        # range must belong to the terminal __LINKEDIT segment.
        raw = minimal_macho((struct.pack(
            "<IIII", custody.LC_FUNCTION_STARTS, 16, 1, 8
        ),))
        with self.assertRaises(custody.CustodyError):
            custody.validate_macho(raw)

        # A terminal segment that overlaps __TEXT does not provide complete
        # non-cross-paired container custody, even if it still ends at EOF.
        raw = bytearray(minimal_macho())
        linkedit = raw.find(b"__LINKEDIT") - 8
        self.assertGreaterEqual(linkedit, 32)
        struct.pack_into("<QQ", raw, linkedit + 40, 512, len(raw) - 512)
        with self.assertRaises(custody.CustodyError):
            custody.validate_macho(raw)

    def test_nonzero_diagnostics_and_resource_overflow_reject(self) -> None:
        with self.assertRaises(custody.CustodyError):
            custody.make_observation(
                1, 23, self.evidence.assembly, self.evidence.artifact,
                self.evidence.stdout, self.evidence.stderr, self.evidence.clang,
                self.evidence.linker, self.evidence.sdk_settings,
                self.evidence.libsystem, self.evidence.runtime,
            )
        self.evidence.stderr.write_bytes(b"warning\n")
        with self.assertRaises(custody.CustodyError):
            custody.make_observation(
                0, 23, self.evidence.assembly, self.evidence.artifact,
                self.evidence.stdout, self.evidence.stderr, self.evidence.clang,
                self.evidence.linker, self.evidence.sdk_settings,
                self.evidence.libsystem, self.evidence.runtime,
            )
        with self.assertRaises(custody.CustodyResourceError):
            custody.validate_macho(b"x" * (custody.MAX_ARTIFACT_BYTES + 1))
        with self.evidence.artifact.open("r+b") as stream:
            stream.truncate(custody.MAX_ARTIFACT_BYTES + 1)
        self.assert_rejects(
            252, "observe", "0", "23", str(self.evidence.assembly),
            str(self.evidence.artifact), str(self.evidence.stdout),
            str(self.evidence.stderr), str(self.evidence.clang),
            str(self.evidence.linker), str(self.evidence.sdk_settings),
            str(self.evidence.libsystem), str(self.evidence.runtime),
        )

    @unittest.skipUnless(platform.system() == "Darwin" and platform.machine() == "arm64", "Darwin arm64 integration")
    def test_command_profile_builds_a_real_unsigned_macho(self) -> None:
        root = Path(self.temporary.name) / "real"
        root.mkdir()
        assembly = root / "candidate.s"
        artifact = root / "candidate"
        stdout = root / "clang.stdout"
        stderr = root / "clang.stderr"
        assembly.write_bytes(POSITIVE_ASSEMBLY)
        clang = subprocess.check_output(["xcrun", "--find", "clang"], text=True).strip()
        linker = subprocess.check_output(["xcrun", "--find", "ld"], text=True).strip()
        sdk = subprocess.check_output(
            ["xcrun", "--sdk", "macosx", "--show-sdk-path"], text=True
        ).strip()
        result = subprocess.run(
            [clang, "-arch", "arm64", "-isysroot", sdk,
             f"-fuse-ld={linker}",
             "-mmacosx-version-min=11.0",
             "-Wl,-no_uuid", "-Wl,-no_adhoc_codesign", "-o", str(artifact), str(assembly)],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        stdout.write_bytes(result.stdout)
        stderr.write_bytes(result.stderr)
        summary = custody.validate_macho(artifact.read_bytes())
        self.assertEqual(summary["target"], "macos_arm64")
        observation = custody.make_observation(
            0, 0, assembly, artifact, stdout, stderr, Path(clang), Path(linker),
            Path(sdk) / "SDKSettings.json", Path(sdk) / "usr/lib/libSystem.tbd",
            Path(
                subprocess.check_output(
                    [clang, "-print-file-name=libclang_rt.osx.a"], text=True
                ).strip()
            ),
        )
        self.assertEqual(observation["command_profile"], custody.COMMAND_PROFILE)
        self.assertEqual(observation["artifact"]["byte_length"], artifact.stat().st_size)
        execution = subprocess.run([str(artifact)], check=False)
        self.assertEqual(execution.returncode, 0)


if __name__ == "__main__":
    unittest.main()
